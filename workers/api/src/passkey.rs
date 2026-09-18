/*
Intent : Let a person log in with their face or fingerprint, and prove the signature server-side.
Pattern: register a passkey, then sign in with it. Delete the challenge row mid-ceremony and it fails.
Signed. Claude / 2026-09-18 UTC

This file is transport only. Every actual check -- challenge, origin,
relying-party id, user-presence flag, the ECDSA signature itself -- lives in
`curatom-webauthn`, which is pure Rust with its own tests and no Cloudflare
in it. If a rule looks like it is enforced here, that is a bug.

The challenge is issued, stored once, and deleted the moment it is spent.
That, not the expiry, is what stops a replay: a genuine assertion presented
twice finds no challenge row the second time.
*/

use worker::*;

use curatom_crypto::random_bytes_32;
use curatom_webauthn::{
    b64url_decode, b64url_encode, verify_assertion, verify_registration, WebauthnError,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth_users::{self, create_session_cookie};
use crate::json_response;

/// Ten minutes: long enough for someone to find their phone, short enough
/// that an abandoned ceremony does not linger.
const CHALLENGE_TTL_SECONDS: i64 = 600;

fn now_unix() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

/// The relying-party id is the bare hostname of whatever origin served this
/// page, and the expected origin is that page's own origin. Derived from the
/// request rather than hardcoded, so this works on the custom domain and on
/// workers.dev without a second configuration to keep in step -- and a
/// passkey registered on one is correctly refused on the other, because the
/// authenticator binds the key to the hostname it saw.
fn relying_party(req: &Request) -> Result<(String, String)> {
    let url = req.url()?;
    let host = url.host_str().unwrap_or_default().to_string();
    let origin = format!("{}://{}", url.scheme(), host);
    Ok((host, origin))
}

async fn issue_challenge(env: &Env, user_id: Option<&str>, ceremony: &str) -> Result<String> {
    let challenge = b64url_encode(&random_bytes_32());
    let now = now_unix();
    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT INTO webauthn_challenges (challenge, user_id, ceremony, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&[
        challenge.clone().into(),
        match user_id {
            Some(u) => u.into(),
            None => wasm_bindgen::JsValue::NULL,
        },
        ceremony.into(),
        (now as f64).into(),
        ((now + CHALLENGE_TTL_SECONDS) as f64).into(),
    ])?
    .run()
    .await?;
    Ok(challenge)
}

/// Reads a challenge and deletes it in the same breath. Returns the user it
/// was issued to, if any. A challenge that is missing, expired, or for the
/// wrong ceremony is simply not found -- all three are the same answer to
/// the caller, because telling them apart tells an attacker which guess was
/// closer.
async fn spend_challenge(
    env: &Env,
    challenge: &str,
    ceremony: &str,
) -> Result<Option<Option<String>>> {
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        user_id: Option<String>,
        expires_at: i64,
    }
    let row: Option<Row> = db
        .prepare("SELECT user_id, expires_at FROM webauthn_challenges WHERE challenge = ?1 AND ceremony = ?2 LIMIT 1")
        .bind(&[challenge.into(), ceremony.into()])?
        .first(None)
        .await?;

    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare("DELETE FROM webauthn_challenges WHERE challenge = ?1")
        .bind(&[challenge.into()])?
        .run()
        .await?;

    Ok(match row {
        Some(r) if now_unix() < r.expires_at => Some(r.user_id),
        _ => None,
    })
}

fn refusal(err: WebauthnError) -> Result<Response> {
    json_response(401, json!({ "error": "passkey_refused", "detail": err.to_string() }))
}

// ---- registration: adding a passkey to an account you are signed in to ----

pub async fn handle_register_begin(req: Request, env: &Env) -> Result<Response> {
    let Some((user_id, username)) = auth_users::resolve_user_session(&req, env).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    let (rp_id, _) = relying_party(&req)?;
    let challenge = issue_challenge(env, Some(&user_id), "register").await?;

    json_response(
        200,
        json!({
            "challenge": challenge,
            "rp_id": rp_id,
            "user_id": b64url_encode(user_id.as_bytes()),
            "username": username,
        }),
    )
}

#[derive(Deserialize)]
struct RegisterFinish {
    #[serde(default)]
    challenge: String,
    #[serde(default)]
    client_data_json: String,
    #[serde(default)]
    attestation_object: String,
    #[serde(default)]
    label: String,
}

pub async fn handle_register_finish(mut req: Request, env: &Env) -> Result<Response> {
    let Some((session_user, _)) = auth_users::resolve_user_session(&req, env).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    let body = req.text().await?;
    let Ok(body) = serde_json::from_str::<RegisterFinish>(&body) else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };

    let Some(issued_to) = spend_challenge(env, &body.challenge, "register").await? else {
        return json_response(400, json!({ "error": "unknown_or_expired_challenge" }));
    };
    // The challenge was issued to a signed-in account; it must be the same
    // account finishing the ceremony.
    if issued_to.as_deref() != Some(session_user.as_str()) {
        return json_response(403, json!({ "error": "challenge_belongs_to_another_account" }));
    }

    let (rp_id, origin) = relying_party(&req)?;
    let expected = b64url_decode(&body.challenge, "challenge")
        .map_err(|e| Error::RustError(e.to_string()))?;

    let credential = match verify_registration(
        &body.client_data_json,
        &body.attestation_object,
        &expected,
        &origin,
        &rp_id,
    ) {
        Ok(c) => c,
        Err(e) => return refusal(e),
    };

    let label = {
        let trimmed = body.label.trim();
        if trimmed.is_empty() { "passkey".to_string() } else { trimmed.chars().take(64).collect() }
    };

    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare(
        "INSERT OR REPLACE INTO user_passkeys (credential_id, user_id, public_key, sign_count, label, created_at, last_used_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
    )
    .bind(&[
        b64url_encode(&credential.credential_id).into(),
        session_user.into(),
        b64url_encode(&credential.public_key_sec1).into(),
        (credential.sign_count as f64).into(),
        label.clone().into(),
        (now_unix() as f64).into(),
    ])?
    .run()
    .await?;

    json_response(201, json!({ "ok": true, "label": label }))
}

// ---- login: signing in with a passkey, no password typed ----

pub async fn handle_login_begin(req: Request, env: &Env) -> Result<Response> {
    let (rp_id, _) = relying_party(&req)?;
    // No user_id: the browser offers whichever passkey it holds for this
    // site, and the assertion names the credential. Nothing is revealed
    // about who has an account here.
    let challenge = issue_challenge(env, None, "login").await?;
    json_response(200, json!({ "challenge": challenge, "rp_id": rp_id }))
}

#[derive(Deserialize)]
struct LoginFinish {
    #[serde(default)]
    challenge: String,
    #[serde(default)]
    credential_id: String,
    #[serde(default)]
    client_data_json: String,
    #[serde(default)]
    authenticator_data: String,
    #[serde(default)]
    signature: String,
}

pub async fn handle_login_finish(mut req: Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    let Ok(body) = serde_json::from_str::<LoginFinish>(&body) else {
        return json_response(400, json!({ "error": "malformed_body" }));
    };

    if spend_challenge(env, &body.challenge, "login").await?.is_none() {
        return json_response(400, json!({ "error": "unknown_or_expired_challenge" }));
    }

    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize)]
    struct Row {
        user_id: String,
        public_key: String,
        sign_count: i64,
    }
    let stored: Option<Row> = db
        .prepare("SELECT user_id, public_key, sign_count FROM user_passkeys WHERE credential_id = ?1 LIMIT 1")
        .bind(&[body.credential_id.clone().into()])?
        .first(None)
        .await?;
    let Some(stored) = stored else {
        return json_response(401, json!({ "error": "unknown_passkey" }));
    };

    let (rp_id, origin) = relying_party(&req)?;
    let expected = b64url_decode(&body.challenge, "challenge")
        .map_err(|e| Error::RustError(e.to_string()))?;
    let public_key = b64url_decode(&stored.public_key, "stored public key")
        .map_err(|e| Error::RustError(e.to_string()))?;

    let new_count = match verify_assertion(
        &body.client_data_json,
        &body.authenticator_data,
        &body.signature,
        &expected,
        &origin,
        &rp_id,
        &public_key,
        stored.sign_count.max(0) as u32,
    ) {
        Ok(c) => c,
        Err(e) => return refusal(e),
    };

    let db = env.d1("CURATOM_LEDGER")?;
    db.prepare("UPDATE user_passkeys SET sign_count = ?1, last_used_at = ?2 WHERE credential_id = ?3")
        .bind(&[
            (new_count as f64).into(),
            (now_unix() as f64).into(),
            body.credential_id.into(),
        ])?
        .run()
        .await?;

    create_session_cookie(env, &stored.user_id, json!({ "ok": true, "via": "passkey" })).await
}

// ---- listing and removing, for the Account tile ----

pub async fn handle_list(req: Request, env: &Env) -> Result<Response> {
    let Some((user_id, _)) = auth_users::resolve_user_session(&req, env).await? else {
        return json_response(401, json!({ "error": "unauthenticated" }));
    };
    let db = env.d1("CURATOM_LEDGER")?;
    #[derive(Deserialize, serde::Serialize)]
    struct Row {
        credential_id: String,
        label: Option<String>,
        created_at: i64,
        last_used_at: Option<i64>,
    }
    let rows: Vec<Row> = db
        .prepare("SELECT credential_id, label, created_at, last_used_at FROM user_passkeys WHERE user_id = ?1 ORDER BY created_at DESC")
        .bind(&[user_id.into()])?
        .all()
        .await?
        .results()?;
    json_response(200, json!({ "passkeys": rows }))
}
