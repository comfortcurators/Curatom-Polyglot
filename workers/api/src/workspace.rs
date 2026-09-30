//! Company-workspace helpers: Valhalla bootstrap + Cloudflare inventory.
//!
//! Kept out of `lib.rs` so the standing/workspace PR stays reviewable.

use serde_json::{json, Value};
use worker::*;

use crate::CuratomKernel;
use curatom_protocol::OrganicToken;

/// Best-effort Valhalla workspace provision right after key create.
/// Failure never fails key creation — the key is already persisted.
pub(crate) async fn bootstrap_valhalla_for_key(
    kernel_do: &CuratomKernel,
    key: &OrganicToken,
) -> (Option<String>, Option<String>) {
    let valhalla_url = kernel_do
        .env
        .var("VALHALLA_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "https://valhalla.rajvansh.dev".into());
    let provision_url = format!(
        "{}/valhalla/provision?token={}&bootstrap=1&label={}",
        valhalla_url,
        urlencoding::encode(&key.token),
        urlencoding::encode(&key.label),
    );
    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    let req = match Request::new_with_init(&provision_url, &init) {
        Ok(r) => r,
        Err(e) => return (None, Some(format!("valhalla_request:{e}"))),
    };
    match Fetch::Request(req).send().await {
        Ok(mut resp) if resp.status_code() < 400 => {
            let body_text = resp.text().await.unwrap_or_default();
            if let Ok(parsed) = serde_json::from_str::<Value>(&body_text) {
                let sid = parsed
                    .get("sandbox_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                return (sid, None);
            }
            (None, Some("valhalla_bad_json".into()))
        }
        Ok(mut resp) => {
            let detail = resp.text().await.unwrap_or_default();
            (None, Some(format!("valhalla_http:{}", detail.chars().take(200).collect::<String>())))
        }
        Err(e) => (None, Some(format!("valhalla_send:{e}"))),
    }
}

/// Cloudflare inventory adapter. Calls the CF API only when both
/// `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` Worker secrets are
/// present. Does not invent secrets or invent inventory.
pub(crate) async fn fetch_cloudflare_inventory(env: &Env) -> (bool, Option<Value>, Option<String>) {
    let token = match env.secret("CLOUDFLARE_API_TOKEN") {
        Ok(s) => s.to_string(),
        Err(_) => {
            return (
                false,
                Some(json!({
                    "configured": false,
                    "hint": "Set Worker secrets CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID to enable live CF inventory."
                })),
                Some("credentials_not_configured".into()),
            );
        }
    };
    let account_id = match env.secret("CLOUDFLARE_ACCOUNT_ID") {
        Ok(s) => s.to_string(),
        Err(_) => match env.var("CLOUDFLARE_ACCOUNT_ID") {
            Ok(s) => s.to_string(),
            Err(_) => {
                return (
                    false,
                    Some(json!({
                        "configured": false,
                        "hint": "Set CLOUDFLARE_ACCOUNT_ID (secret or var) alongside CLOUDFLARE_API_TOKEN."
                    })),
                    Some("credentials_not_configured".into()),
                );
            }
        },
    };
    let auth = format!("Bearer {token}");
    let workers_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/workers/scripts"
    );
    let d1_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/d1/database"
    );
    let r2_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/r2/buckets"
    );
    let workers = crate::github_get(&workers_url, Some(&auth)).await.ok();
    let d1 = crate::github_get(&d1_url, Some(&auth)).await.ok();
    let r2 = crate::github_get(&r2_url, Some(&auth)).await.ok();
    if workers.is_none() && d1.is_none() && r2.is_none() {
        return (
            false,
            Some(json!({ "configured": true, "account_id": account_id })),
            Some("cloudflare_api_unreachable".into()),
        );
    }
    (
        true,
        Some(json!({
            "configured": true,
            "account_id": account_id,
            "workers": workers.and_then(|v| v.get("result").cloned()).unwrap_or(json!([])),
            "d1": d1.and_then(|v| v.get("result").cloned()).unwrap_or(json!([])),
            "r2": r2.and_then(|v| v.get("result").cloned()).unwrap_or(json!([])),
            "source": "cloudflare_api",
        })),
        None,
    )
}
