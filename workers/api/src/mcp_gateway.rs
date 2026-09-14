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
use serde_json::{json, Value};
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

/// Outcome of a validated capability, shared by every entry point (`/mcp`,
/// `/tools/list`, `/tools/call`) so the bearer -> hash -> lookup ->
/// revoked/expired/scope sequence exists exactly once.
struct Authorized {
    key_hash: String,
    scope: String,
}

enum AuthFailure {
    /// No credential, unknown key, revoked, or expired -- code and message
    /// are exactly what the JSON-RPC error body needs.
    Rpc(i32, &'static str),
    /// D1 itself did not answer -- refuse, do not admit.
    Store,
}

async fn authorize(req: &Request, env: &Env) -> std::result::Result<Authorized, AuthFailure> {
    let Some(presented) = bearer(req) else {
        return Err(AuthFailure::Rpc(
            -32000,
            "No Curatom capability presented. Mint a key at curatom.rajvansh.dev, \
             then paste it along with the handoff block that key came with.",
        ));
    };

    let key_hash = sha256_hex(&presented);

    // A store failure refuses. Admitting on a lookup failure would turn a D1
    // outage into an authentication bypass on the operator plane.
    let capability = match lookup_capability(env, &key_hash).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err(AuthFailure::Rpc(
                -32000,
                "Unknown Curatom key. Mint a new key at curatom.rajvansh.dev.",
            ));
        }
        Err(err) => {
            console_error!("capability lookup failed: {err}");
            return Err(AuthFailure::Store);
        }
    };

    if capability.revoked != 0 {
        return Err(AuthFailure::Rpc(-32000, "This Curatom key has been revoked."));
    }

    let now = CloudflareClock.now_unix();
    if now >= capability.expires_at {
        return Err(AuthFailure::Rpc(
            -32001,
            "Curatom capability window has expired. Ask the operator to approve a new one.",
        ));
    }

    let scope = match capability.scope.as_str() {
        "oauth" | "full" => capability.scope.clone(),
        other => {
            console_error!("capability {key_hash} has invalid scope {other}");
            return Err(AuthFailure::Rpc(-32002, "Curatom capability record is invalid."));
        }
    };

    Ok(Authorized { key_hash, scope })
}

pub async fn handle_mcp(mut req: Request, env: &Env) -> Result<Response> {
    let authorized = match authorize(&req, env).await {
        Ok(a) => a,
        Err(AuthFailure::Rpc(code, message)) => return rpc_error(None, code, message),
        Err(AuthFailure::Store) => {
            return rpc_error(
                None,
                -32002,
                "Curatom could not verify this key right now. Access is refused until \
                 the capability store answers.",
            );
        }
    };
    let Authorized { key_hash, scope } = authorized;

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

// ---------------------------------------------------------------------------
// REST facade: /tools/list, /tools/call
//
// Same capability (same D1 row, same expiry, same scope) as /mcp, reachable
// by a plain-HTTP caller that has no MCP client and no JSON-RPC framing --
// DeepSeek, or anything else that only speaks "POST JSON, get JSON back".
// A body here is a plain result or a plain {"error": "..."} object, never a
// JSON-RPC envelope; a status code carries the outcome, same as any other
// REST endpoint in this Worker.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct RestErrorBody {
    error: String,
}

fn rest_error(status: u16, message: impl Into<String>) -> Result<Response> {
    let body = RestErrorBody { error: message.into() };
    let resp = Response::from_json(&body)?.with_status(status);
    Ok(resp)
}

/// Sends one JSON-RPC request upstream through the same scope-stamped path
/// as `handle_mcp`, and unwraps the envelope: `Ok(result)` on success,
/// `Err(error_value)` for an upstream-reported failure (e.g. "unknown
/// tool" from HostOS's own scoped registration refusal). This is the piece
/// that lets a plain-HTTP caller receive a normal JSON body instead of a
/// protocol envelope it was never going to parse.
async fn call_upstream(
    env: &Env,
    key_hash: &str,
    scope: &str,
    method: &str,
    params: Value,
) -> Result<std::result::Result<Value, Value>> {
    let rpc_body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    })
    .to_string();

    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    headers.set("accept", "application/json, text/event-stream")?;
    headers.set("x-internal-caller", &format!("curatom-key:{key_hash}"))?;
    headers.set("x-internal-scope", scope)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&rpc_body)));

    let upstream_req = Request::new_with_init(INTERNAL_URL, &init)?;
    let fetcher = env.service("HOSTOS_MCP")?;
    let mut upstream_resp = fetcher.fetch_request(upstream_req).await?;
    let raw = upstream_resp.text().await?;

    // hostos-mcp answers either bare JSON or an SSE `data:` frame depending
    // on the call; both carry the same JSON-RPC envelope underneath.
    let envelope: Value = match raw.lines().find(|l| l.starts_with("data:")) {
        Some(data_line) => {
            serde_json::from_str(data_line.trim_start_matches("data:").trim()).unwrap_or(Value::Null)
        }
        None => serde_json::from_str(&raw).unwrap_or(Value::Null),
    };

    if let Some(err) = envelope.get("error") {
        return Ok(Err(err.clone()));
    }
    Ok(Ok(envelope.get("result").cloned().unwrap_or(Value::Null)))
}

fn upstream_error_message(err: &Value) -> String {
    err.get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("upstream error")
        .to_string()
}

/// JSON-RPC's own code for calling a tool that isn't registered on the
/// scoped surface -- verified live against hostos-mcp's real response
/// (`{"code":-32602,"message":"Tool exec not found"}`). This is the MCP
/// SDK's own dispatch behavior, not HostOS's wording, so it doesn't move
/// if HostOS ever rephrases the message. Matching on `message` instead
/// would be the same fragility this gateway exists to avoid: a fact that
/// lives in a service this Worker can't verify, silently reverting to a
/// bare 502 the moment the string changes.
const RPC_UNKNOWN_TOOL: i64 = -32602;

pub async fn handle_tools_list(req: Request, env: &Env) -> Result<Response> {
    let authorized = match authorize(&req, env).await {
        Ok(a) => a,
        Err(AuthFailure::Rpc(_, message)) => return rest_error(401, message),
        Err(AuthFailure::Store) => {
            return rest_error(
                503,
                "Curatom could not verify this key right now. Access is refused until \
                 the capability store answers.",
            );
        }
    };

    match call_upstream(env, &authorized.key_hash, &authorized.scope, "tools/list", json!({})).await? {
        Ok(result) => {
            let mut resp = Response::from_json(&result)?;
            resp.headers_mut().set("cache-control", "no-store")?;
            Ok(resp)
        }
        Err(err) => rest_error(502, upstream_error_message(&err)),
    }
}

pub async fn handle_tools_call(mut req: Request, env: &Env) -> Result<Response> {
    let authorized = match authorize(&req, env).await {
        Ok(a) => a,
        Err(AuthFailure::Rpc(_, message)) => return rest_error(401, message),
        Err(AuthFailure::Store) => {
            return rest_error(
                503,
                "Curatom could not verify this key right now. Access is refused until \
                 the capability store answers.",
            );
        }
    };

    let body = req.text().await?;
    if body.len() > MAX_BODY_BYTES {
        return rest_error(400, "Request body exceeds the Curatom gateway limit.");
    }
    let parsed: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return rest_error(400, "Invalid JSON body."),
    };
    let Some(name) = parsed.get("name").and_then(|v| v.as_str()) else {
        return rest_error(400, "Missing \"name\" field naming the tool to call.");
    };
    let arguments = parsed.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let params = json!({ "name": name, "arguments": arguments });
    match call_upstream(env, &authorized.key_hash, &authorized.scope, "tools/call", params).await? {
        Ok(result) => {
            let mut resp = Response::from_json(&result)?;
            resp.headers_mut().set("cache-control", "no-store")?;
            Ok(resp)
        }
        Err(err) => {
            let msg = upstream_error_message(&err);
            // An unregistered tool on a scoped surface is a 404-shaped fact
            // about what this key can reach, not a gateway fault. Matched
            // on the JSON-RPC code, not the message -- see RPC_UNKNOWN_TOOL.
            let is_unknown_tool = err.get("code").and_then(|c| c.as_i64()) == Some(RPC_UNKNOWN_TOOL);
            let status = if is_unknown_tool { 404 } else { 502 };
            rest_error(status, msg)
        }
    }
}
