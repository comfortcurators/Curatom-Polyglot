//! Cloudflare Access JWT verification, JWKS-backed.
//!
//! Contract 1's original gate. `curatom-substrate-cloudflare`'s
//! `CloudflareAccessIdentityProvider` deliberately removed the old
//! `cf-access-authenticated-user-email` / `cf-access-jwt-assertion`
//! header-trust branches, because neither ever verified anything: the
//! first was trusted unconditionally (`|| true`), the second was trusted
//! for mere presence, not a valid signature. That was only safe while
//! Cloudflare's edge gate stopped unauthenticated traffic before the
//! Worker ran -- true on the custom domain, false the moment this Worker
//! is reachable anywhere Access does not cover.
//!
//! This module is what restores the check honestly. It fetches
//! Cloudflare's JWKS, verifies the RS256 signature on the
//! `Cf-Access-Jwt-Assertion` header, checks issuer, audience, and
//! expiry, and only then reports an email. The caller maps that email to
//! a `users` row; if no row exists, the request is not authenticated as
//! anyone.
//!
//! Nothing here trusts a header for its presence. Every field that
//! decides authentication is checked against a signature over the whole
//! token.
//!
//! Inert unless `TEAM_DOMAIN` and `POLICY_AUD` are both set to non-empty
//! values in the Worker's environment. With either unset, the public
//! entry point `verify_and_resolve` returns `Ok(None)` without doing any
//! work, and the caller falls through to whatever other authentication
//! it has.

use std::sync::Mutex;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rsa::{BigUint, Pkcs1v15Sign, RsaPublicKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use worker::*;

/// One hour. Cloudflare's JWKS endpoint sends its own `Cache-Control`,
/// but a hard cap here means a stale in-memory copy cannot outlive its
/// usefulness if a Worker isolate stays warm for longer than that.
const JWKS_TTL_SECONDS: i64 = 3600;

/// Cloudflare's JWKS path, under whatever team domain Access uses.
/// `/cdn-cgi/access/certs` is the documented location.
const JWKS_PATH: &str = "/cdn-cgi/access/certs";

#[derive(Debug, Clone)]
struct JwkKey {
    kid: String,
    n: String,
    e: String,
}

struct CachedJwks {
    keys: Vec<JwkKey>,
    fetched_at: i64,
}

/// Per-isolate, populated on first verification and refreshed on TTL or
/// on an unknown `kid`. `std::sync::Mutex` on wasm32-unknown-unknown is a
/// single-threaded no-op lock; it exists so the same code compiles if
/// this crate is ever also built for a threaded target.
static JWKS_CACHE: Mutex<Option<CachedJwks>> = Mutex::new(None);

fn now_unix() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn b64url_decode(s: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(s.trim())
        .map_err(|e| format!("bad_base64url: {e}"))
}

#[derive(Deserialize)]
struct JwtHeader {
    alg: String,
    #[serde(default)]
    kid: String,
}

#[derive(Deserialize)]
struct JwtClaims {
    #[serde(default)]
    email: String,
    #[serde(default)]
    sub: String,
    exp: i64,
    iss: String,
    aud: serde_json::Value,
}

fn aud_contains(aud: &serde_json::Value, want: &str) -> bool {
    match aud {
        serde_json::Value::String(s) => s == want,
        serde_json::Value::Array(v) => v.iter().any(|x| x.as_str() == Some(want)),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct AccessIdentity {
    pub email: String,
    #[allow(dead_code)]
    pub sub: String,
}

async fn fetch_jwks(env: &Env, team_domain: &str) -> Result<Vec<JwkKey>, String> {
    let _ = env;
    let url = format!("{}{}", team_domain.trim_end_matches('/'), JWKS_PATH);
    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    let req = Request::new_with_init(&url, &init).map_err(|e| e.to_string())?;
    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    if resp.status_code() >= 400 {
        return Err(format!("jwks_status_{}", resp.status_code()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let keys = parsed
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "jwks_no_keys".to_string())?;
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        // Only RSA signing keys are usable here. A JWKS response is
        // allowed to carry other key types; skipping them is not an
        // error, only having none left at the end is.
        let kty = k.get("kty").and_then(|v| v.as_str()).unwrap_or("");
        let use_ = k.get("use").and_then(|v| v.as_str()).unwrap_or("sig");
        if kty != "RSA" || use_ != "sig" {
            continue;
        }
        let kid = k.get("kid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let n = k.get("n").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let e = k.get("e").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if kid.is_empty() || n.is_empty() || e.is_empty() {
            continue;
        }
        out.push(JwkKey { kid, n, e });
    }
    if out.is_empty() {
        return Err("jwks_no_rsa_keys".into());
    }
    Ok(out)
}

async fn get_key(env: &Env, team_domain: &str, kid: &str) -> Result<JwkKey, String> {
    // Fresh cache hit.
    {
        let guard = JWKS_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(c) = guard.as_ref() {
            if now_unix() - c.fetched_at < JWKS_TTL_SECONDS {
                if let Some(k) = c.keys.iter().find(|k| k.kid == kid) {
                    return Ok(k.clone());
                }
                // Fresh cache, unknown kid: fall through to one refetch.
                // This is the key-rotation case and should be rare. A
                // request storm with a fabricated kid will refetch each
                // time; Cloudflare rate-limits the JWKS endpoint, and a
                // future revision can add a per-isolate negative cache.
            }
        }
    }

    let fetched = fetch_jwks(env, team_domain).await?;
    let found = fetched.iter().find(|k| k.kid == kid).cloned();
    {
        let mut guard = JWKS_CACHE.lock().map_err(|e| e.to_string())?;
        *guard = Some(CachedJwks {
            keys: fetched,
            fetched_at: now_unix(),
        });
    }
    found.ok_or_else(|| format!("unknown_kid:{kid}"))
}

fn verify_rs256(key: &JwkKey, message: &[u8], signature: &[u8]) -> Result<(), String> {
    let n = BigUint::from_bytes_be(&b64url_decode(&key.n)?);
    let e = BigUint::from_bytes_be(&b64url_decode(&key.e)?);
    let public = RsaPublicKey::new(n, e).map_err(|e| format!("bad_rsa_key: {e}"))?;
    let digest = Sha256::digest(message);
    public
        .verify(Pkcs1v15Sign::new::<Sha256>(), digest.as_slice(), signature)
        .map_err(|_| "signature_invalid".to_string())
}

/// Verify a Cloudflare Access JWT end to end and return the email and
/// subject it carries. Fails closed on every possible badness: malformed
/// token, unsupported algorithm, unknown kid after a refetch, bad
/// signature, expired, wrong issuer, wrong audience.
pub async fn verify(
    env: &Env,
    jwt: &str,
    team_domain: &str,
    policy_aud: &str,
) -> Result<AccessIdentity, String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("malformed_jwt".into());
    }
    let header_b64 = parts[0];
    let payload_b64 = parts[1];
    let signature_b64 = parts[2];

    let header: JwtHeader = serde_json::from_slice(&b64url_decode(header_b64)?)
        .map_err(|e| format!("bad_header: {e}"))?;
    if header.alg != "RS256" {
        return Err(format!("unsupported_alg: {}", header.alg));
    }
    if header.kid.is_empty() {
        return Err("missing_kid".into());
    }

    let signature = b64url_decode(signature_b64)?;
    let signing_input = format!("{header_b64}.{payload_b64}");
    let key = get_key(env, team_domain, &header.kid).await?;
    verify_rs256(&key, signing_input.as_bytes(), &signature)?;

    let claims: JwtClaims = serde_json::from_slice(&b64url_decode(payload_b64)?)
        .map_err(|e| format!("bad_claims: {e}"))?;
    let now = now_unix();
    if claims.exp <= now {
        return Err("expired".into());
    }
    let expected_iss = team_domain.trim_end_matches('/');
    if claims.iss.trim_end_matches('/') != expected_iss {
        return Err(format!("bad_iss: {}", claims.iss));
    }
    if !aud_contains(&claims.aud, policy_aud) {
        return Err("bad_aud".into());
    }
    if claims.email.is_empty() {
        return Err("missing_email".into());
    }
    Ok(AccessIdentity {
        email: claims.email.to_lowercase(),
        sub: claims.sub,
    })
}

/// The whole path: verify the JWT, then map its email to a `users.id`.
///
/// Return values, and why they are distinct:
///   `Ok(Some(user_id))` -- token verified, email is a registered user.
///   `Ok(None)`         -- TEAM_DOMAIN or POLICY_AUD not configured.
///                         Caller should fall through to whatever other
///                         authentication it has. This is the case that
///                         makes the whole module inert on a deployment
///                         that has not set those vars.
///   `Err(_)`           -- a token was presented and did not verify, or
///                         a D1 error while resolving the email. A bad
///                         token is not the same as no token, and the
///                         caller must fail closed rather than fall
///                         through.
pub async fn verify_and_resolve(env: &Env, jwt: &str) -> Result<Option<String>, String> {
    let team_domain = match env.var("TEAM_DOMAIN") {
        Ok(v) => v.to_string(),
        Err(_) => return Ok(None),
    };
    let policy_aud = match env.var("POLICY_AUD") {
        Ok(v) => v.to_string(),
        Err(_) => return Ok(None),
    };
    if team_domain.trim().is_empty() || policy_aud.trim().is_empty() {
        return Ok(None);
    }

    let identity = verify(env, jwt, &team_domain, &policy_aud).await?;

    let db = env.d1("CURATOM_LEDGER").map_err(|e| format!("d1: {e}"))?;
    #[derive(Deserialize)]
    struct Row {
        id: String,
    }
    let row: Option<Row> = db
        .prepare("SELECT id FROM users WHERE email = ?1 LIMIT 1")
        .bind(&[identity.email.into()])
        .map_err(|e| format!("bind: {e}"))?
        .first(None)
        .await
        .map_err(|e| format!("query: {e}"))?;
    Ok(row.map(|r| r.id))
}
