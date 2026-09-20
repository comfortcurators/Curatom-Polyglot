/*
Intent : Register with an email + a password you chose; the verify link only activates the account.
Pattern: POST /auth/register {email,password}, follow the ZeptoMail link -- hit it twice and it's still fine.
Signed. Claude / 2026-09-18 UTC

Corrected 2026-09-18. The previous shape of this file minted the
password on the verify GET and showed it exactly once, in that
response. That put a real secret behind a race: mail providers' link
scanners (Microsoft Safe Links and equivalents at Google, Yahoo, Zoho)
prefetch every URL in an inbound email before a human ever clicks it,
which is itself a GET -- so the scanner won the race, the account was
minted with a password nobody but the scanner ever saw, and the actual
person got "unknown_or_used_token" on a link already burned out from
under them. Confirmed live against a dedicated test mailbox: this exact failure occurred
on first registration.

The fix removes the secret from that path rather than trying to referee
who gets there first. The password is chosen at registration -- the
same request that claims the email -- hashed immediately, and carried
on the `pending_verifications` row. `handle_verify` no longer creates or
reveals a credential; it only flips a pending claim into a real account,
which is safe to run twice. Whichever request gets there first (scanner
or human) completes the same account creation; the second is a no-op
that reads as success ("already verified, sign in"), not an error --
there is nothing left in that response worth winning a race for.

Additive, still: this does not touch RAJ_TOKEN or
CloudflareAccessIdentityProvider. What it does reach, past this file, is
per-user Durable Object routing in lib.rs -- a verified account's session
now resolves to its own CuratomKernel instance, not the founder's.
*/

use worker::*;

use curatom_crypto::{hash_password, random_id, random_salt_hex, random_token_hex, sha256_hex, verify_password};
use serde::Deserialize;
use serde_json::json;

use crate::json_response;
use crate::mail::{send_password_reset_email, send_verification_email, MailError};

pub const USER_SESSION_COOKIE: &str = "curatom_user_session";
const SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60; // 30 days, same as the RAJ_TOKEN cookie.
const VERIFICATION_TTL_SECONDS: i64 = 30 * 60; // matches the wording in the email itself.
const MAX_EMAIL_LEN: usize = 254;
const MIN_PASSWORD_LEN: usize = 8;

#[derive(Deserialize)]
struct RegisterBody {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    /// Empty string (the default) means "mint one from the email" --
    /// unchanged from before this field existed.
    #[serde(default)]
    username: String,
}

#[derive(Deserialize)]
struct LoginBody {
    /// Either the minted username or the registered email -- whichever
    /// the person has to hand.
    #[serde(default)]
    username_or_email: String,
    #[serde(default)]
    password: String,
    /// Cloudflare Turnstile response token from the sign-in page. Single
    /// use, ~300s TTL; Siteverify is what actually decides. Required --
    /// the design says the sign-in page has a human gate, and login is
    /// the endpoint an unauthenticated attacker actually hammers.
    #[serde(default)]
    turnstile_token: String,
}

fn normalize_email(raw: &str) -> Option<String> {
    let email = raw.trim().to_lowercase();
    if email.is_empty() || email.len() > MAX_EMAIL_LEN {
        return None;
    }
    let mut parts = email.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return None;
    }
    Some(email)
}

fn now_unix() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn base_url(req: &Request) -> Result<String> {
    let url = req.url()?;
    Ok(format!(
        "{}://{}",
        url.scheme(),
        url.host_str().unwrap_or("curatom.rajvansh.dev")
    ))
}

async fn read_json<T: for<'de> Deserialize<'de>>(req: &mut Request) -> Option<T> {
    let body = req.text().await.ok()?;
    serde_json::from_str(&body).ok()
}

async fn email_is_registered(env: &Env, email: &str) -> Result<bool> {
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        #[allow(dead_code)]
        id: String,
    }
    let stmt = db
        .prepare("SELECT id FROM users WHERE email = ?1 LIMIT 1")
        .bind(&[email.into()])?;
    let row: Option<Row> = stmt.first(None).await?;
    Ok(row.is_some())
}

const MIN_USERNAME_LEN: usize = 3;
const MAX_USERNAME_LEN: usize = 32;

/// Lowercase, `[a-z0-9_-]` only, 3-32 characters -- the same shape
/// `mint_username` already produces automatically, just chosen by hand
/// instead. Returns `None` for anything that doesn't fit that shape;
/// the caller turns that into a specific error, this just says yes/no.
fn normalize_chosen_username(raw: &str) -> Option<String> {
    let u = raw.trim().to_lowercase();
    if u.len() < MIN_USERNAME_LEN || u.len() > MAX_USERNAME_LEN {
        return None;
    }
    if !u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    if !u.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
        return None;
    }
    Some(u)
}

async fn username_taken(env: &Env, username: &str) -> Result<bool> {
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        #[allow(dead_code)]
        username: String,
    }
    let row: Option<Row> = db
        .prepare("SELECT username FROM users WHERE username = ?1 LIMIT 1")
        .bind(&[username.into()])?
        .first(None)
        .await?;
    Ok(row.is_some())
}

/// The live check the register form calls as someone types. This is an
/// advisory answer, not the gate -- `users.username` carries its own
/// `UNIQUE` constraint in D1, which is what actually decides at the
/// moment `handle_verify` writes the row, closing the gap between "was
/// free when I checked" and "is free right now" that this endpoint
/// alone could never close.
pub async fn handle_username_available(req: Request, env: &Env) -> Result<Response> {
    let url = req.url()?;
    let raw = url.query_pairs().find(|(k, _)| k == "u").map(|(_, v)| v.into_owned()).unwrap_or_default();
    let Some(username) = normalize_chosen_username(&raw) else {
        return json_response(200, json!({ "available": false, "reason": "invalid_format" }));
    };
    let taken = username_taken(env, &username).await?;
    json_response(200, json!({ "available": !taken, "username": username }))
}

/// Derives a unique username from the email's local part: lowercase,
/// non `[a-z0-9]` characters dropped, then a numeric suffix appended if
/// the plain form is already taken. Falls back to a random handle if the
/// local part strips to nothing (an address like `+1@example.com`).
async fn mint_username(env: &Env, email: &str) -> Result<String> {
    let local = email.split('@').next().unwrap_or("");
    let base: String = local
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let base = if base.is_empty() {
        format!("user{}", &random_token_hex()[..8])
    } else {
        base
    };

    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        #[allow(dead_code)]
        username: String,
    }
    let taken = |candidate: &str| -> D1PreparedStatement {
        db.prepare("SELECT username FROM users WHERE username = ?1 LIMIT 1")
            .bind(&[candidate.into()])
            .expect("bind is infallible for a single text param")
    };

    let existing: Option<Row> = taken(&base).first(None).await?;
    if existing.is_none() {
        return Ok(base);
    }
    for suffix in 1..1000u32 {
        let candidate = format!("{base}{suffix}");
        let existing: Option<Row> = taken(&candidate).first(None).await?;
        if existing.is_none() {
            return Ok(candidate);
        }
    }
    // Practically unreachable (it would mean 1000 accounts share one
    // local-part base), but a real error beats an infinite loop.
    Err(Error::RustError("could not mint a unique username".into()))
}

async fn create_session(env: &Env, user_id: &str) -> Result<String> {
    let token = random_token_hex();
    let hash = sha256_hex(token.as_bytes());
    let now = now_unix();
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT INTO user_sessions (session_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&[
        hash.into(),
        user_id.into(),
        // D1's bind rejects a JS BigInt outright, and `worker`'s
        // `From<i64> for JsValue` produces one -- cast through f64
        // (lossless well under 2^53 for a unix-seconds timestamp),
        // same fix already applied in h_approve_knock.
        (now as f64).into(),
        ((now + SESSION_TTL_SECONDS) as f64).into(),
    ])?
    .run()
    .await?;
    Ok(token)
}

fn set_session_cookie(resp: &mut Response, token: &str) -> Result<()> {
    let cookie = format!(
        "{USER_SESSION_COOKIE}={token}; Path=/; Max-Age={SESSION_TTL_SECONDS}; HttpOnly; Secure; SameSite=Lax"
    );
    resp.headers_mut().append("Set-Cookie", &cookie)
}

/// Mints a session for `user_id`, sets it as the response cookie, and
/// returns the response built from `body` -- the one path both password
/// login and passkey login end at, so a session is created exactly one
/// way in this codebase.
pub async fn create_session_cookie(env: &Env, user_id: &str, body: serde_json::Value) -> Result<Response> {
    let token = create_session(env, user_id).await?;
    let mut resp = json_response(200, body)?;
    set_session_cookie(&mut resp, &token)?;
    Ok(resp)
}

fn clear_session_cookie(resp: &mut Response) -> Result<()> {
    let cookie = format!("{USER_SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax");
    resp.headers_mut().append("Set-Cookie", &cookie)
}

fn cookie_value<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    cookie_header.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k == name).then_some(v)
    })
}

/// The registered user this session belongs to, or `None` for no/invalid/
/// expired session. Everything past this file that wants "which account
/// is making this request" goes through this, not through re-reading the
/// cookie by hand. Takes the raw `Cookie` header rather than a
/// `worker::Request` so it can be called both at the top level (where the
/// request is still a live `Request`) and from inside `CuratomKernel`
/// (where the request has already become an `HttpRequestDto` -- its
/// header map, not a `Request`, since `to_dto` consumes the original by
/// reading its body).
pub async fn resolve_user_session_from_cookie(
    cookie_header: Option<&str>,
    env: &Env,
) -> Result<Option<(String, String)>> {
    let Some(cookie_header) = cookie_header else {
        return Ok(None);
    };
    let Some(token) = cookie_value(cookie_header, USER_SESSION_COOKIE) else {
        return Ok(None);
    };
    let hash = sha256_hex(token.as_bytes());
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct SessionRow {
        user_id: String,
        expires_at: i64,
    }
    let stmt = db
        .prepare("SELECT user_id, expires_at FROM user_sessions WHERE session_hash = ?1 LIMIT 1")
        .bind(&[hash.into()])?;
    let Some(session): Option<SessionRow> = stmt.first(None).await? else {
        return Ok(None);
    };
    if now_unix() >= session.expires_at {
        return Ok(None);
    }
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct UsernameRow {
        username: String,
    }
    let stmt = db
        .prepare("SELECT username FROM users WHERE id = ?1 LIMIT 1")
        .bind(&[session.user_id.clone().into()])?;
    let Some(row): Option<UsernameRow> = stmt.first(None).await? else {
        return Ok(None);
    };
    Ok(Some((session.user_id, row.username)))
}

/// `resolve_user_session_from_cookie`, reading the header off a live
/// `worker::Request` (headers-only -- never touches the body, so the
/// request is still whole for whoever forwards it next).
pub async fn resolve_user_session(req: &Request, env: &Env) -> Result<Option<(String, String)>> {
    let cookie_header = req.headers().get("Cookie")?;
    resolve_user_session_from_cookie(cookie_header.as_deref(), env).await
}

/// Step 1: claim an email address and choose a password. No account
/// exists yet -- only a `pending_verifications` row (password already
/// hashed) and a sent email. The password never appears in any response
/// this file sends; it was typed by the person who is about to read
/// their own email, not minted and handed to whichever request follows
/// the link first.
pub async fn handle_register(mut req: Request, env: &Env) -> Result<Response> {
    let Some(body) = read_json::<RegisterBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    let Some(email) = normalize_email(&body.email) else {
        return json_response(400, json!({ "error": "invalid_email" }));
    };
    if body.password.len() < MIN_PASSWORD_LEN {
        return json_response(400, json!({ "error": "password_too_short", "min_length": MIN_PASSWORD_LEN }));
    }
    // Optional: an empty string means "mint one from the email", same
    // as before this existed. A non-empty one that doesn't fit the
    // shape, or is already taken, is rejected here -- this is only the
    // friendly pre-check, though; `users.username UNIQUE` is what
    // actually decides at the moment `handle_verify` writes the row.
    let chosen_username = if body.username.trim().is_empty() {
        None
    } else {
        let Some(u) = normalize_chosen_username(&body.username) else {
            return json_response(
                400,
                json!({ "error": "invalid_username", "detail": "3-32 characters, letters/numbers/-/_, must start with a letter or number" }),
            );
        };
        if username_taken(env, &u).await? {
            return json_response(409, json!({ "error": "username_taken" }));
        }
        Some(u)
    };

    if email_is_registered(env, &email).await? {
        return json_response(409, json!({ "error": "email_already_registered" }));
    }

    let salt = random_salt_hex();
    let password_hash = hash_password(&body.password, &salt);

    let token = random_token_hex();
    let hash = sha256_hex(token.as_bytes());
    let now = now_unix();
    let db = env.d1("CURATOM_LEDGER")?;

    // `handle_verify` deliberately stops deleting a pending row the
    // moment it's used, so that a second hit of the same link (a mail
    // scanner's prefetch, then the real click) reads back as "already
    // verified" instead of "unknown token". Nothing else clears these
    // rows out, so this is the sweep: on every registration, reap
    // whatever has aged past its own `expires_at`, used or not. A
    // best-effort delete, not a transaction -- worst case a row survives
    // one extra registration cycle, which costs nothing.
    db.prepare("DELETE FROM pending_verifications WHERE expires_at < ?1")
        .bind(&[(now as f64).into()])?
        .run()
        .await?;

    db.prepare(
        "INSERT OR REPLACE INTO pending_verifications (token_hash, email, created_at, expires_at, password_hash, password_salt, username) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&[
        hash.into(),
        email.clone().into(),
        (now as f64).into(),
        ((now + VERIFICATION_TTL_SECONDS) as f64).into(),
        password_hash.into(),
        salt.into(),
        chosen_username.clone().unwrap_or_default().into(),
    ])?
    .run()
    .await?;

    let verify_url = format!("{}/auth/verify?token={token}", base_url(&req)?);
    match send_verification_email(env, &email, &verify_url).await {
        Ok(()) => json_response(
            202,
            json!({ "ok": true, "message": "check your email to finish registering" }),
        ),
        Err(MailError::NotConfigured) => json_response(
            503,
            json!({ "error": "email_not_configured", "detail": "ZEPTO_TOKEN is not set" }),
        ),
        Err(MailError::SendFailed(detail)) => {
            json_response(502, json!({ "error": "email_send_failed", "detail": detail }))
        }
    }
}

/// Step 2: the ZeptoMail link lands here. Activates the account with
/// the password chosen at registration -- nothing is minted and nothing
/// secret is in this response, so hitting this twice (a mail scanner's
/// prefetch, then the person's real click) is harmless: the first
/// request creates the account, the second reads it back as already
/// done rather than failing.
pub async fn handle_verify(req: Request, env: &Env) -> Result<Response> {
    let url = req.url()?;
    let Some(token) = url.query_pairs().find(|(k, _)| k == "token").map(|(_, v)| v.into_owned()) else {
        return json_response(400, json!({ "error": "missing_token" }));
    };
    let hash = sha256_hex(token.as_bytes());

    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct PendingRow {
        email: String,
        expires_at: i64,
        password_hash: String,
        password_salt: String,
        #[serde(default)]
        username: String,
    }
    let stmt = db
        .prepare("SELECT email, expires_at, password_hash, password_salt, username FROM pending_verifications WHERE token_hash = ?1 LIMIT 1")
        .bind(&[hash.into()])?;
    let Some(pending): Option<PendingRow> = stmt.first(None).await? else {
        // No row at all for this token: it was never issued, or it was
        // issued and has since aged past `expires_at` and been reaped by
        // `handle_register`. Either way there is nothing left to tell
        // this apart from a genuinely bad link, which is the honest
        // answer here -- the row is kept around (not deleted) on
        // success specifically so a second hit of a *good* link lands
        // in the `email_is_registered` branch below instead of here.
        return json_response(400, json!({ "error": "unknown_or_expired_token" }));
    };
    if now_unix() >= pending.expires_at {
        return json_response(400, json!({ "error": "verification_expired" }));
    }

    // A link followed twice (a mail provider's link-scanner prefetch,
    // then the person's real click -- the *same* token both times, since
    // it's the same link) must not mint two accounts for one email --
    // check again, inside this same request, right before writing
    // `users`. Unlike before, this is not an error: whichever request
    // got here first already finished the job, and this one just reads
    // it back. The pending row is deliberately left in place rather than
    // deleted (below, and on the success path past this point) so that
    // this exact branch is reachable on a second hit -- deleting
    // eagerly turned the second, harmless hit back into the same
    // "unknown_or_expired_token" dead end this fix exists to remove.
    // It ages out on its own via `expires_at`; `handle_register` reaps
    // expired rows so this table doesn't grow unbounded.
    if email_is_registered(env, &pending.email).await? {
        return json_response(
            200,
            json!({ "ok": true, "already_verified": true, "message": "this account is already active -- sign in" }),
        );
    }

    // D1 serializes writes to one database, so a select-then-insert like
    // this is race-free the same way it already is elsewhere in this
    // org's D1-backed code: nothing else can slot a write in between
    // this check and the INSERT below. `users.username UNIQUE` remains
    // the real backstop regardless.
    let username = if !pending.username.is_empty() {
        if username_taken(env, &pending.username).await? {
            return json_response(
                409,
                json!({ "error": "username_taken_since_registration", "detail": "someone else took it first -- register again with a different one" }),
            );
        }
        pending.username.clone()
    } else {
        mint_username(env, &pending.email).await?
    };
    let user_id = random_id("user");
    let now = now_unix();

    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT INTO users (id, username, email, password_hash, password_salt, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&[
        user_id.clone().into(),
        username.clone().into(),
        pending.email.clone().into(),
        pending.password_hash.into(),
        pending.password_salt.into(),
        (now as f64).into(),
    ])?
    .run()
    .await?;

    // The pending row is left in place -- see the comment above the
    // `email_is_registered` check for why: a second hit of this same
    // link (the scanner-then-human case this whole fix is for) needs it
    // still there to recognize "already done" instead of "unknown
    // token". It ages out via `expires_at`; `handle_register` reaps
    // expired rows.

    let session_token = create_session(env, &user_id).await?;
    let mut resp = json_response(
        201,
        json!({
            "ok": true,
            "account": {
                "username": username,
                "email": pending.email,
            },
        }),
    )?;
    set_session_cookie(&mut resp, &session_token)?;
    Ok(resp)
}

async fn find_user_by_login(env: &Env, username_or_email: &str) -> Result<Option<(String, String, String)>> {
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        id: String,
        password_hash: String,
        password_salt: String,
    }
    let stmt = db
        .prepare("SELECT id, password_hash, password_salt FROM users WHERE username = ?1 OR email = ?1 LIMIT 1")
        .bind(&[username_or_email.into()])?;
    let row: Option<Row> = stmt.first(None).await?;
    Ok(row.map(|r| (r.id, r.password_hash, r.password_salt)))
}

pub async fn handle_login(mut req: Request, env: &Env) -> Result<Response> {
    let Some(body) = read_json::<LoginBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };

    // Turnstile first: an attacker must not be able to probe credentials
    // at all without solving the challenge, so this runs before any
    // database work, before password verification, before anything that
    // could be timed or counted.
    let turnstile_secret = match env.secret("TURNSTILE_SECRET") {
        Ok(s) => s.to_string(),
        Err(_) => return json_response(500, json!({ "error": "turnstile not configured" })),
    };
    let remote_ip = req.headers().get("cf-connecting-ip")?;
    match crate::verify_turnstile(&turnstile_secret, &body.turnstile_token, remote_ip.as_deref()).await {
        Ok(true) => {}
        Ok(false) => return json_response(403, json!({ "error": "turnstile_failed" })),
        Err(e) => return json_response(502, json!({ "error": e })),
    }

    let login = body.username_or_email.trim().to_lowercase();
    if login.is_empty() {
        return json_response(401, json!({ "error": "invalid_credentials" }));
    }

    let Some((user_id, password_hash, password_salt)) = find_user_by_login(env, &login).await? else {
        // Same message as a wrong password: whether a username/email is
        // registered is not this endpoint's information to give away.
        return json_response(401, json!({ "error": "invalid_credentials" }));
    };

    if !verify_password(&body.password, &password_salt, &password_hash) {
        return json_response(401, json!({ "error": "invalid_credentials" }));
    }

    let token = create_session(env, &user_id).await?;
    let mut resp = json_response(200, json!({ "ok": true }))?;
    set_session_cookie(&mut resp, &token)?;
    Ok(resp)
}

pub async fn handle_logout(req: Request, env: &Env) -> Result<Response> {
    if let Some(cookie_header) = req.headers().get("Cookie")? {
        if let Some(token) = cookie_value(&cookie_header, USER_SESSION_COOKIE) {
            let hash = sha256_hex(token.as_bytes());
            let db = env.d1("CURATOM_LEDGER")?;
            db.prepare("DELETE FROM user_sessions WHERE session_hash = ?1")
                .bind(&[hash.into()])?
                .run()
                .await?;
        }
    }
    let mut resp = json_response(200, json!({ "ok": true }))?;
    clear_session_cookie(&mut resp)?;
    Ok(resp)
}

pub async fn handle_me(req: Request, env: &Env) -> Result<Response> {
    let Some((id, username)) = resolve_user_session(&req, env).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct EmailRow {
        email: String,
    }
    let email: Option<EmailRow> = db
        .prepare("SELECT email FROM users WHERE id = ?1 LIMIT 1")
        .bind(&[id.clone().into()])?
        .first(None)
        .await?;
    json_response(
        200,
        json!({
            "ok": true,
            "user": {
                "id": id,
                "username": username,
                "email": email.map(|e| e.email),
            },
        }),
    )
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    #[serde(default)]
    current_password: String,
    #[serde(default)]
    new_password: String,
}

/// Requires the current password, the same as any account's password
/// change should -- a stolen session cookie alone must not be enough to
/// lock the real owner out by rotating the password under them.
pub async fn handle_change_password(mut req: Request, env: &Env) -> Result<Response> {
    let Some((user_id, _)) = resolve_user_session(&req, env).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    let Some(body) = read_json::<ChangePasswordBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    if body.new_password.len() < MIN_PASSWORD_LEN {
        return json_response(400, json!({ "error": "password_too_short", "min_length": MIN_PASSWORD_LEN }));
    }

    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        password_hash: String,
        password_salt: String,
    }
    let stmt = db
        .prepare("SELECT password_hash, password_salt FROM users WHERE id = ?1 LIMIT 1")
        .bind(&[user_id.clone().into()])?;
    let Some(row): Option<Row> = stmt.first(None).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    if !verify_password(&body.current_password, &row.password_salt, &row.password_hash) {
        return json_response(401, json!({ "error": "current_password_incorrect" }));
    }

    let salt = random_salt_hex();
    let password_hash = hash_password(&body.new_password, &salt);
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare("UPDATE users SET password_hash = ?1, password_salt = ?2 WHERE id = ?3")
        .bind(&[password_hash.into(), salt.into(), user_id.into()])?
        .run()
        .await?;

    json_response(200, json!({ "ok": true }))
}

// ---- forgot password: no session required to start, a real token to finish ----
//
// Same shape as email verification, on purpose: a token is minted,
// hashed, stored with a short TTL, and the plaintext exists only in the
// one email that carries it. The confirm step is a POST that carries
// both the token and the new password together -- unlike the old
// verify-link bug earlier in this file's history, there is nothing here
// for a mail scanner's GET prefetch to burn, because nothing mutates on
// a GET at all.

#[derive(Deserialize)]
struct ResetBeginBody {
    #[serde(default)]
    email: String,
}

/// Always answers the same way whether or not the email is registered --
/// telling them apart would let this endpoint be used to enumerate
/// accounts. The email only goes out when there is somewhere to send it.
pub async fn handle_password_reset_begin(mut req: Request, env: &Env) -> Result<Response> {
    let Some(body) = read_json::<ResetBeginBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    let Some(email) = normalize_email(&body.email) else {
        return json_response(202, json!({ "ok": true }));
    };

    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        id: String,
    }
    let user: Option<Row> = db
        .prepare("SELECT id FROM users WHERE email = ?1 LIMIT 1")
        .bind(&[email.clone().into()])?
        .first(None)
        .await?;

    if let Some(user) = user {
        let token = random_token_hex();
        let hash = sha256_hex(token.as_bytes());
        let now = now_unix();
        let db = env.d1("CURATOM_LEDGER")?;
        // Reaped the same opportunistic way pending_verifications is --
        // see handle_register for why a dedicated sweep isn't needed.
        db.prepare("DELETE FROM password_resets WHERE expires_at < ?1")
            .bind(&[(now as f64).into()])?
            .run()
            .await?;
        db.prepare(
            "INSERT INTO password_resets (token_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&[
            hash.into(),
            user.id.into(),
            (now as f64).into(),
            ((now + VERIFICATION_TTL_SECONDS) as f64).into(),
        ])?
        .run()
        .await?;

        let reset_url = format!("{}/?reset_token={token}", base_url(&req)?);
        // Best-effort: a send failure here must not tell the caller
        // whether the email exists, so it isn't surfaced as an error.
        let _ = send_password_reset_email(env, &email, &reset_url).await;
    }

    json_response(202, json!({ "ok": true }))
}

#[derive(Deserialize)]
struct ResetConfirmBody {
    #[serde(default)]
    token: String,
    #[serde(default)]
    new_password: String,
}

pub async fn handle_password_reset_confirm(mut req: Request, env: &Env) -> Result<Response> {
    let Some(body) = read_json::<ResetConfirmBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    if body.new_password.len() < MIN_PASSWORD_LEN {
        return json_response(400, json!({ "error": "password_too_short", "min_length": MIN_PASSWORD_LEN }));
    }
    let hash = sha256_hex(body.token.as_bytes());

    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        user_id: String,
        expires_at: i64,
    }
    let stmt = db
        .prepare("SELECT user_id, expires_at FROM password_resets WHERE token_hash = ?1 LIMIT 1")
        .bind(&[hash.clone().into()])?;
    let Some(row): Option<Row> = stmt.first(None).await? else {
        return json_response(400, json!({ "error": "unknown_or_expired_token" }));
    };
    // Deleted before anything else, single-use -- unlike the email-verify
    // link, a reset link has nothing worth reading twice: the second hit
    // just needs to fail, not fail *helpfully*, since a stranger holding
    // a stale reset link learning "this token already worked" reveals
    // nothing useful anyway.
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare("DELETE FROM password_resets WHERE token_hash = ?1")
        .bind(&[hash.into()])?
        .run()
        .await?;
    if now_unix() >= row.expires_at {
        return json_response(400, json!({ "error": "token_expired" }));
    }

    let salt = random_salt_hex();
    let password_hash = hash_password(&body.new_password, &salt);
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare("UPDATE users SET password_hash = ?1, password_salt = ?2 WHERE id = ?3")
        .bind(&[password_hash.into(), salt.into(), row.user_id.clone().into()])?
        .run()
        .await?;

    create_session_cookie(env, &row.user_id, json!({ "ok": true })).await
}

#[derive(Deserialize)]
struct DeleteAccountBody {
    /// Typed by the person, exactly. Guards against an accidental call:
    /// `"DELETE"` is one keystroke from every other destructive word
    /// and there is no confirm step on the client for an API this
    /// sharp. Case-sensitive on purpose.
    #[serde(default)]
    confirm: String,
    /// The account's current password. Proves intent and identity in
    /// one field -- `DELETE` says "I mean to destroy this," the
    /// password says "and I am the person who owns it." A stolen
    /// session cookie is enough to read and write; it must not be
    /// enough to permanently destroy. Same reasoning and same
    /// mechanism as `handle_change_password`.
    #[serde(default)]
    password: String,
}

/// Hard delete. Requires an authenticated session and the confirm word.
///
/// Erased: the `users` row, every `user_sessions` row, every
/// `user_passkeys` row, every `pending_verifications` row for this
/// email, and every `password_resets` row for this user.
///
/// Preserved: the `ledger` table (key_hash, session_id, kind,
/// body_ref, timestamps -- no PII), R2 objects behind those refs, and
/// the DO's own KernelState. The DO is not cleared because it holds
/// the account's tokens, knocks, connectors, repositories, and
/// checkpoints, and clearing it on the way out would destroy the
/// operator's own audit trail of what happened under their keys. A
/// follow-up part should decide whether a deleted account's DO should
/// be wiped on a delay (30 days is a common pattern) rather than
/// immediately; this handler does not guess.
///
/// The response is not a redirect to a login page. The caller's session
/// cookie was already invalidated by the `DELETE FROM user_sessions`
/// below, and the client (which will not exist for a while, this is an
/// API-first endpoint) is expected to clear its own cookie.
pub async fn handle_delete_account(mut req: Request, env: &Env) -> Result<Response> {
    let Some((user_id, _)) = resolve_user_session(&req, env).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    let Some(body) = read_json::<DeleteAccountBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    if body.confirm != "DELETE" {
        return json_response(400, json!({
            "error": "confirmation_required",
            "detail": "send {\"confirm\":\"DELETE\",\"password\":\"<your current password>\"} to proceed",
        }));
    }

    let db = env.d1("CURATOM_LEDGER")?;

    // One query, three things: the email (for the pending_verifications
    // sweep, which is keyed by email not user_id because that row
    // predates the user), the password hash and salt (for the
    // re-authentication below), and the presence of the row (which
    // distinguishes "valid session, missing user" -- a double-submit
    // or a session that outlived a deletion -- from the real case).
    #[derive(Deserialize)]
    struct UserRow {
        email: String,
        password_hash: String,
        password_salt: String,
    }
    let Some(user_row): Option<UserRow> = db
        .prepare("SELECT email, password_hash, password_salt FROM users WHERE id = ?1 LIMIT 1")
        .bind(&[user_id.clone().into()])?
        .first(None)
        .await?
    else {
        return json_response(200, json!({ "ok": true, "already_deleted": true }));
    };

    // Re-authenticate. The confirm word proves intent; the password
    // proves the caller is the owner. Both are required because both
    // fail independently -- a stolen cookie carries the first but not
    // the second, and a typo carries neither.
    if !verify_password(&body.password, &user_row.password_salt, &user_row.password_hash) {
        return json_response(401, json!({ "error": "password_incorrect" }));
    }

    // Ordering: children before parent. Each statement is a single D1
    // write and the whole block is not a transaction -- D1 serializes
    // single-database writes, so no other request can interleave, but
    // a crash between two statements leaves the account partially
    // deleted. The endpoint is idempotent (the second call finds no
    // `users` row and returns `already_deleted`), so a partial run
    // completes on retry.
    db.prepare("DELETE FROM user_sessions WHERE user_id = ?1")
        .bind(&[user_id.clone().into()])?
        .run()
        .await?;
    db.prepare("DELETE FROM user_passkeys WHERE user_id = ?1")
        .bind(&[user_id.clone().into()])?
        .run()
        .await?;
    db.prepare("DELETE FROM password_resets WHERE user_id = ?1")
        .bind(&[user_id.clone().into()])?
        .run()
        .await?;
    db.prepare("DELETE FROM pending_verifications WHERE email = ?1")
        .bind(&[user_row.email.into()])?
        .run()
        .await?;
    db.prepare("DELETE FROM users WHERE id = ?1")
        .bind(&[user_id.into()])?
        .run()
        .await?;

    // Explicitly do NOT clear the session cookie here. The row it
    // refers to no longer exists, so the cookie is inert -- every
    // subsequent request will fail `resolve_user_session` and be
    // treated as anonymous. Setting a `Set-Cookie` with `Max-Age=0`
    // would be tidier but is not load-bearing, and leaving it out
    // keeps this handler free of response-header surgery.
    json_response(200, json!({ "ok": true }))
}
