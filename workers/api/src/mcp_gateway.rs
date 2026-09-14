//! Curatom's capability broker in front of HostOS MCP.
//!
//! HostOS is the only thing on this account that holds GITHUB_APP_PRIVATE_KEY,
//! CLOUDFLARE_API_TOKEN, R2 keys and CURATOR_ACTION_TOKEN. This Worker holds
//! none of them. It holds one service binding, and it uses that binding to
//! ask HostOS a single question per request: does this Curatom key have a
//! live capability, and if so which scope?
//!
//! It does not reimplement HostOS tools. Every MCP tool the caller sees --
//! workspace_status, file_read, exec, checkpoint_create, repository_acquire,
//! curator_act -- is the real HostOS tool, dispatched by the real HostOS
//! handler, in a workspace HostOS derives from a subject *this* Worker
//! supplies. The tools are HostOS's. The identity is Curatom's.
//!
//! A request with no live capability never reaches HostOS at all.

use serde::Serialize;
use serde_json::Value;
use worker::*;

use curatom_ports::Clock;
use curatom_substrate_cloudflare::CloudflareClock;

/// hostos-mcp's internal route (INTERNAL_HOSTNAME in computer-v2/src/index.ts).
/// Reachable only through the HOSTOS_MCP service binding -- that hostname has
/// no DNS route. Renaming one without the other 404s here, not a silent
/// fallthrough to HostOS's public plane.
const INTERNAL_URL: &str = "https://hostos-mcp.internal/mcp";

/// Headers the caller's MCP client sets that matter to the upstream protocol
/// layer. Everything else -- Authorization (carries the Curatom key and must
/// not travel further than this Worker), cookies, CF-* -- is dropped.
/// `accept` is deliberately absent here: hostos-mcp's MCP transport (the
/// `agents/mcp/server` library) hard-requires
/// `application/json, text/event-stream` and 406s anything else, so the
/// gateway sets that itself below rather than trusting a caller to send it
/// correctly. Verified live: a caller without that exact Accept header got
/// upstream's real 406 before this fix.
const FORWARDED_HEADERS: &[&str] = &["content-type", "mcp-session-id", "mcp-protocol-version"];

const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, serde::Deserialize)]
struct CapabilityRow {
    scope: String,
    expires_at: i64,
    revoked: i64,
}

#[derive(Serialize)]
struct RpcErrorBody {
    jsonrpc: &'static str,
    id: Option<Value>,
    error: RpcErrorInner,
}

#[derive(Serialize)]
struct RpcErrorInner {
    code: i32,
    message: String,
}

/// A JSON-RPC error in the response body, not a bare HTTP status. An MCP
/// client often reacts to a 401 with a re-auth dance; an error object in the
/// protocol envelope is text the caller can read and relay verbatim.
fn rpc_error(id: Option<Value>, code: i32, message: impl Into<String>) -> Result<Response> {
    let body = RpcErrorBody {
        jsonrpc: "2.0",
        id,
        error: RpcErrorInner { code, message: message.into() },
    };
    let mut resp = Response::from_json(&body)?;
    resp.headers_mut().set("cache-control", "no-store")?;
    Ok(resp)
}

fn bearer(req: &Request) -> Option<String> {
    let raw = req.headers().get("authorization").ok().flatten()?;
    let trimmed = raw.trim();
    if trimmed.len() < 7 || !trimmed[..7].eq_ignore_ascii_case("bearer ") {
        return None;
    }
    let token = trimmed[7..].trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extracts the JSON-RPC id from a request body so an error can be matched
/// to the call that produced it. A null id on parse failure is what MCP
/// clients expect for parse-level errors.
fn extract_rpc_id(body: &str) -> Option<Value> {
    serde_json::from_str::<Value>(body).ok()?.get("id").cloned()
}

/// `None` for a key never minted. `Some` (with `revoked = 1`) for one that
/// was. Those produce different messages on purpose -- a legitimate
/// revocation should never read like a typo in a paste.
async fn lookup_capability(env: &Env, key_hash: &str) -> Result<Option<CapabilityRow>> {
    let db = env.d1("CURATOM_LEDGER")?;
    let stmt = db
        .prepare("SELECT scope, expires_at, revoked FROM capabilities WHERE key_hash = ?1 LIMIT 1")
        .bind(&[key_hash.into()])?;
    stmt.first::<CapabilityRow>(None).await
}

pub async fn handle_mcp(mut req: Request, env: &Env) -> Result<Response> {
    let Some(presented) = bearer(&req) else {
        return rpc_error(
            None,
            -32000,
            "No Curatom capability presented. Mint a key at curatom.rajvansh.dev, \
             then paste it along with the handoff block that key came with.",
        );
    };

    let key_hash = sha256_hex(&presented);

    // A store failure refuses. Admitting on a lookup failure would turn a D1
    // outage into an authentication bypass on the operator plane.
    let capability = match lookup_capability(env, &key_hash).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return rpc_error(
                None,
                -32000,
                "Unknown Curatom key. Mint a new key at curatom.rajvansh.dev.",
            );
        }
        Err(err) => {
            console_error!("capability lookup failed: {err}");
            return rpc_error(
                None,
                -32002,
                "Curatom could not verify this key right now. Access is refused until \
                 the capability store answers.",
            );
        }
    };

    if capability.revoked != 0 {
        return rpc_error(None, -32000, "This Curatom key has been revoked.");
    }

    let now = CloudflareClock.now_unix();
    if now >= capability.expires_at {
        return rpc_error(
            None,
            -32001,
            "Curatom capability window has expired. Ask the operator to approve a new one.",
        );
    }

    let scope = match capability.scope.as_str() {
        "oauth" | "full" => capability.scope.clone(),
        other => {
            console_error!("capability {key_hash} has invalid scope {other}");
            return rpc_error(None, -32002, "Curatom capability record is invalid.");
        }
    };

    let body = req.text().await?;
    if body.len() > MAX_BODY_BYTES {
        return rpc_error(extract_rpc_id(&body), -32600, "Request body exceeds the Curatom gateway limit.");
    }

    let headers = Headers::new();
    for name in FORWARDED_HEADERS {
        if let Ok(Some(value)) = req.headers().get(name) {
            headers.set(name, &value)?;
        }
    }
    headers.set("accept", "application/json, text/event-stream")?;
    headers.set("x-internal-caller", &format!("curatom-key:{key_hash}"))?;
    headers.set("x-internal-scope", &scope)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&body)));

    let upstream_req = Request::new_with_init(INTERNAL_URL, &init)?;
    let fetcher = env.service("HOSTOS_MCP")?;
    let mut upstream_resp = fetcher.fetch_request(upstream_req).await?;

    let content_type = upstream_resp
        .headers()
        .get("content-type")
        .ok()
        .flatten()
        .unwrap_or_else(|| "application/json".to_string());
    let upstream_body = upstream_resp.text().await?;

    let mut out = Response::ok(upstream_body)?;
    out.headers_mut().set("content-type", &content_type)?;
    out.headers_mut().set("cache-control", "no-store")?;
    Ok(out)
}
