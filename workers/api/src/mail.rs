/*
Intent : Send the one email registration needs -- pure Rust, the worker crate's own fetch, nothing else.
Pattern: register with a real address, read the ZeptoMail Processed Emails log for the send.
Signed. Claude / 2026-09-18 UTC

ZeptoMail is the org's existing transactional-email provider (SuperhostOS's
apps/api/src/services/email.ts already carries the same contract in
TypeScript, for a JS Worker -- this is that same HTTP contract, in Rust,
for this Rust Worker; no code or dependency is shared between them).
*/

use worker::*;

const ZEPTO_ENDPOINT: &str = "https://api.zeptomail.in/v1.1/email";

fn from_address(env: &Env) -> String {
    env.var("EMAIL_FROM")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "noreply@rajvansh.dev".to_string())
}

pub enum MailError {
    /// ZEPTO_TOKEN is not set. Distinguished from a send failure so the
    /// caller can say "email is not configured" rather than "your email
    /// bounced" -- different problems, different fixes.
    NotConfigured,
    SendFailed(String),
}

/// Sends one HTML email through ZeptoMail. No fallback provider, on
/// purpose: registration should fail loudly and immediately if mail
/// cannot go out, not silently mint an account nobody can verify.
pub async fn send_verification_email(env: &Env, to: &str, verify_url: &str) -> std::result::Result<(), MailError> {
    let html = format!(
        "<p>Confirm this address to finish registering at Curatom Polyglot.</p>\
         <p><a href=\"{verify_url}\">{verify_url}</a></p>\
         <p>This link expires in 30 minutes. If you didn't request this, ignore it.</p>"
    );
    send_html_email(env, to, "Verify your Curatom account", &html).await
}

/// The forgot-password email. Same "fail loudly" discipline as
/// registration -- a reset that silently didn't send is worse than one
/// that errors, because the person is locked out either way and only
/// one of those tells them.
pub async fn send_password_reset_email(env: &Env, to: &str, reset_url: &str) -> std::result::Result<(), MailError> {
    let html = format!(
        "<p>Reset your Curatom password.</p>\
         <p><a href=\"{reset_url}\">{reset_url}</a></p>\
         <p>This link expires in 30 minutes. If you didn't request this, ignore it -- \
         your password hasn't changed.</p>"
    );
    send_html_email(env, to, "Reset your Curatom password", &html).await
}

async fn send_html_email(env: &Env, to: &str, subject: &str, html: &str) -> std::result::Result<(), MailError> {
    let token = env
        .secret("ZEPTO_TOKEN")
        .map_err(|_| MailError::NotConfigured)?
        .to_string();

    let body = serde_json::json!({
        "from": { "address": from_address(env) },
        "to": [ { "email_address": { "address": to } } ],
        "subject": subject,
        "htmlbody": html,
    });

    let headers = Headers::new();
    headers
        .set("authorization", &token)
        .map_err(|e| MailError::SendFailed(e.to_string()))?;
    headers
        .set("content-type", "application/json")
        .map_err(|e| MailError::SendFailed(e.to_string()))?;
    headers
        .set("accept", "application/json")
        .map_err(|e| MailError::SendFailed(e.to_string()))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&body.to_string())));

    let req = Request::new_with_init(ZEPTO_ENDPOINT, &init).map_err(|e| MailError::SendFailed(e.to_string()))?;
    let mut resp = Fetch::Request(req)
        .send()
        .await
        .map_err(|e| MailError::SendFailed(e.to_string()))?;

    if resp.status_code() >= 200 && resp.status_code() < 300 {
        return Ok(());
    }
    let detail = resp.text().await.unwrap_or_default();
    Err(MailError::SendFailed(format!(
        "ZeptoMail {}: {}",
        resp.status_code(),
        detail.chars().take(300).collect::<String>()
    )))
}
