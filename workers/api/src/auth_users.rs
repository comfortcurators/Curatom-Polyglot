/*
Intent : Register with an email, verify it, and Curatom hands you your own username and password.
Pattern: POST /auth/register {email}, follow the ZeptoMail link, read the minted credentials back once.
Signed. Claude / 2026-09-18 UTC

The founder's own workflow sketch, followed step for step: go to
curatom.rajvansh.dev, register via email (ZeptoMail verification), get
your own username and password. Nobody types a password in at signup --
the system mints one, the same mint-once, show-once shape every other
credential in this org already uses (organic tokens, capability keys,
business_register's keys). `mail.rs` sends the one email this needs.

Additive, still: this does not touch RAJ_TOKEN or
CloudflareAccessIdentityProvider. What it does reach, past this file, is
per-user Durable Object routing in lib.rs -- a verified account's session
now resolves to its own CuratomKernel instance, not the founder's.
*/

use worker::*;

use curatom_crypto::{
    hash_password, random_id, random_readable_secret, random_salt_hex, random_token_hex,
    sha256_hex, verify_password,
};
use serde::Deserialize;
use serde_json::json;

use crate::json_response;
use crate::mail::{send_verification_email, MailError};

pub const USER_SESSION_COOKIE: &str = "curatom_user_session";
const SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60; // 30 days, same as the RAJ_TOKEN cookie.
const VERIFICATION_TTL_SECONDS: i64 = 30 * 60; // matches the wording in the email itself.
const MAX_EMAIL_LEN: usize = 254;
const MINTED_PASSWORD_LEN: usize = 20;

#[derive(Deserialize)]
struct RegisterBody {
    #[serde(default)]
    email: String,
}

#[derive(Deserialize)]
struct LoginBody {
    /// Either the minted username or the registered email -- whichever
    /// the person has to hand.
    #[serde(default)]
    username_or_email: String,
    #[serde(default)]
    password: String,
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

/// Step 1: claim an email address. No account exists yet -- only a
/// `pending_verifications` row and a sent email.
pub async fn handle_register(mut req: Request, env: &Env) -> Result<Response> {
    let Some(body) = read_json::<RegisterBody>(&mut req).await else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    let Some(email) = normalize_email(&body.email) else {
        return json_response(400, json!({ "error": "invalid_email" }));
    };

    if email_is_registered(env, &email).await? {
        return json_response(409, json!({ "error": "email_already_registered" }));
    }

    let token = random_token_hex();
    let hash = sha256_hex(token.as_bytes());
    let now = now_unix();
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT OR REPLACE INTO pending_verifications (token_hash, email, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&[
        hash.into(),
        email.clone().into(),
        (now as f64).into(),
        ((now + VERIFICATION_TTL_SECONDS) as f64).into(),
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

/// Step 2: the ZeptoMail link lands here. Mints the account -- username,
/// password, session -- and shows the password exactly once, in this
/// response body. Nothing after this request will ever display it again.
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
    }
    let stmt = db
        .prepare("SELECT email, expires_at FROM pending_verifications WHERE token_hash = ?1 LIMIT 1")
        .bind(&[hash.clone().into()])?;
    let Some(pending): Option<PendingRow> = stmt.first(None).await? else {
        return json_response(400, json!({ "error": "unknown_or_used_token" }));
    };
    if now_unix() >= pending.expires_at {
        return json_response(400, json!({ "error": "verification_expired" }));
    }

    // A link followed twice (double-click, a mail client's link
    // pre-fetch) must not mint two accounts for one email -- check
    // again, inside this same request, right before writing `users`.
    if email_is_registered(env, &pending.email).await? {
        let db = env.d1("CURATOM_LEDGER")?;
        db.prepare("DELETE FROM pending_verifications WHERE token_hash = ?1")
            .bind(&[hash.into()])?
            .run()
            .await?;
        return json_response(409, json!({ "error": "email_already_registered" }));
    }

    let username = mint_username(env, &pending.email).await?;
    let password = random_readable_secret(MINTED_PASSWORD_LEN);
    let salt = random_salt_hex();
    let password_hash = hash_password(&password, &salt);
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
        password_hash.into(),
        salt.into(),
        (now as f64).into(),
    ])?
    .run()
    .await?;

    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare("DELETE FROM pending_verifications WHERE token_hash = ?1")
        .bind(&[hash.into()])?
        .run()
        .await?;

    let session_token = create_session(env, &user_id).await?;
    let mut resp = json_response(
        201,
        json!({
            "ok": true,
            "account": {
                "username": username,
                "password": password,
                "email": pending.email,
            },
            "notice": "this password is shown once -- save it now",
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
    match resolve_user_session(&req, env).await? {
        Some((id, username)) => json_response(200, json!({ "ok": true, "user": { "id": id, "username": username } })),
        None => json_response(401, json!({ "error": "unauthenticated" })),
    }
}
