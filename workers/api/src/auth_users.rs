/*
Intent : Give every person their own username and password. Nothing here reads RAJ_TOKEN or Access.
Pattern: npx wrangler d1 execute CURATOM_LEDGER --command "select count(*) from users". Register, then /auth/me with the cookie.
Signed. Claude / 2026-09-18 UTC

Step 1 of the founder's own workflow sketch: curatom.rajvansh.dev -> register
via email -> get your own username and password. This module is additive --
it does not touch `CloudflareAccessIdentityProvider`, RAJ_TOKEN, or the
single-owner `CuratomKernel` Durable Object. A registered user can prove who
they are (this file) before there is anywhere else in the system that acts
on that identity. Wiring a registered user's own workspace (their own
Durable Object, keyed on their user id, the same shape HostOS already
proves in `computer-v2/src/workspace.ts`) is the next step, not this one --
ripping out RAJ_TOKEN before that exists would strand the one person who
currently has no other way in.

Every credential here follows the discipline `0001_capabilities.sql`
already states: storage holds a hash, the plaintext exists exactly once,
in the browser that presented it.
*/

use worker::*;

use curatom_crypto::{
    hash_password, random_id, random_salt_hex, random_token_hex, sha256_hex, verify_password,
};
use serde::Deserialize;
use serde_json::json;

use crate::json_response;

pub const USER_SESSION_COOKIE: &str = "curatom_user_session";
const SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60; // 30 days, same as the RAJ_TOKEN cookie.
const MIN_PASSWORD_LEN: usize = 10;
const MAX_EMAIL_LEN: usize = 254;

#[derive(Deserialize)]
struct Credentials {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
}

struct UserRow {
    id: String,
    email: String,
    password_hash: String,
    password_salt: String,
}

fn normalize_email(raw: &str) -> Option<String> {
    let email = raw.trim().to_lowercase();
    if email.is_empty() || email.len() > MAX_EMAIL_LEN {
        return None;
    }
    // Deliberately loose: "contains exactly one @, with something on each
    // side" catches every typo worth catching without maintaining a regex
    // that will always be wrong about some real address. The confirmation
    // step this org's other login flows use (a real send) is the actual
    // proof an address works; this is just enough to reject empty/garbage
    // input before it reaches D1.
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

async fn read_credentials(req: &mut Request) -> Result<Option<Credentials>> {
    let body = req.text().await?;
    Ok(serde_json::from_str(&body).ok())
}

async fn find_user_by_email(env: &Env, email: &str) -> Result<Option<UserRow>> {
    let db = env.d1("CURATOM_LEDGER")?;
    let stmt = db
        .prepare("SELECT id, email, password_hash, password_salt FROM users WHERE email = ?1 LIMIT 1")
        .bind(&[email.into()])?;
    #[derive(Deserialize)]
    struct Row {
        id: String,
        email: String,
        password_hash: String,
        password_salt: String,
    }
    let row: Option<Row> = stmt.first(None).await?;
    Ok(row.map(|r| UserRow {
        id: r.id,
        email: r.email,
        password_hash: r.password_hash,
        password_salt: r.password_salt,
    }))
}

async fn create_session(env: &Env, user_id: &str) -> Result<String> {
    let token = random_token_hex();
    let hash = sha256_hex(token.as_bytes());
    let now = now_unix();
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT INTO sessions (session_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&[
        hash.into(),
        user_id.into(),
        now.into(),
        (now + SESSION_TTL_SECONDS).into(),
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

/// The identity this session resolves to, or `None` for no/invalid/expired
/// session. Every other route that wants "which registered user is this"
/// goes through this, not through re-reading the cookie by hand.
pub async fn resolve_user_session(req: &Request, env: &Env) -> Result<Option<(String, String)>> {
    let Some(cookie_header) = req.headers().get("Cookie")? else {
        return Ok(None);
    };
    let Some(token) = cookie_value(&cookie_header, USER_SESSION_COOKIE) else {
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
        .prepare("SELECT user_id, expires_at FROM sessions WHERE session_hash = ?1 LIMIT 1")
        .bind(&[hash.into()])?;
    let Some(session): Option<SessionRow> = stmt.first(None).await? else {
        return Ok(None);
    };
    if now_unix() >= session.expires_at {
        return Ok(None);
    }
    let db = env.d1("CURATOM_LEDGER")?;
    let stmt = db
        .prepare("SELECT email FROM users WHERE id = ?1 LIMIT 1")
        .bind(&[session.user_id.clone().into()])?;
    #[derive(Deserialize)]
    struct EmailRow {
        email: String,
    }
    let Some(row): Option<EmailRow> = stmt.first(None).await? else {
        return Ok(None);
    };
    Ok(Some((session.user_id, row.email)))
}

pub async fn handle_register(mut req: Request, env: &Env) -> Result<Response> {
    let Some(creds) = read_credentials(&mut req).await? else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    let Some(email) = normalize_email(&creds.email) else {
        return json_response(400, json!({ "error": "invalid_email" }));
    };
    if creds.password.len() < MIN_PASSWORD_LEN {
        return json_response(
            400,
            json!({ "error": "password_too_short", "min_length": MIN_PASSWORD_LEN }),
        );
    }

    if find_user_by_email(env, &email).await?.is_some() {
        return json_response(409, json!({ "error": "email_already_registered" }));
    }

    let salt = random_salt_hex();
    let hash = hash_password(&creds.password, &salt);
    let user_id = random_id("user");
    let now = now_unix();

    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT INTO users (id, email, password_hash, password_salt, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&[
        user_id.clone().into(),
        email.clone().into(),
        hash.into(),
        salt.into(),
        now.into(),
    ])?
    .run()
    .await?;

    let token = create_session(env, &user_id).await?;
    let mut resp = json_response(201, json!({ "ok": true, "user": { "id": user_id, "email": email } }))?;
    set_session_cookie(&mut resp, &token)?;
    Ok(resp)
}

pub async fn handle_login(mut req: Request, env: &Env) -> Result<Response> {
    let Some(creds) = read_credentials(&mut req).await? else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };
    let Some(email) = normalize_email(&creds.email) else {
        return json_response(401, json!({ "error": "invalid_credentials" }));
    };

    let Some(user) = find_user_by_email(env, &email).await? else {
        // Same message as a wrong password: whether an email is registered
        // is not this endpoint's information to give away.
        return json_response(401, json!({ "error": "invalid_credentials" }));
    };

    if !verify_password(&creds.password, &user.password_salt, &user.password_hash) {
        return json_response(401, json!({ "error": "invalid_credentials" }));
    }

    let token = create_session(env, &user.id).await?;
    let mut resp = json_response(
        200,
        json!({ "ok": true, "user": { "id": user.id, "email": user.email } }),
    )?;
    set_session_cookie(&mut resp, &token)?;
    Ok(resp)
}

pub async fn handle_logout(req: Request, env: &Env) -> Result<Response> {
    if let Some(cookie_header) = req.headers().get("Cookie")? {
        if let Some(token) = cookie_value(&cookie_header, USER_SESSION_COOKIE) {
            let hash = sha256_hex(token.as_bytes());
            let db = env.d1("CURATOM_LEDGER")?;
            db.prepare("DELETE FROM sessions WHERE session_hash = ?1")
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
        Some((id, email)) => json_response(200, json!({ "ok": true, "user": { "id": id, "email": email } })),
        None => json_response(401, json!({ "error": "unauthenticated" })),
    }
}
