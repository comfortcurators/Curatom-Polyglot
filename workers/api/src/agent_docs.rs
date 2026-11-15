//! Agent-facing documentation and the remote MCP endpoint.
//!
//! One source of truth: `docs/llm-guide.md`. It is served as JSON at
//! `/llm-guide` (unchanged shape), as markdown at `/llm-guide.md`, and is
//! what the MCP `curatom_guide` tool returns. `/llms.txt` and
//! `/openapi.json` are short machine-readable companions kept in `docs/`.
//! `scripts/check_agent_docs.py` fails if the docs name a route the
//! Worker does not serve.

use serde_json::{json, Value};
use worker::*;

pub const LLM_GUIDE: &str = include_str!("../../../docs/llm-guide.md");
pub const LLMS_TXT: &str = include_str!("../../../docs/llms.txt");
pub const OPENAPI_JSON: &str = include_str!("../../../docs/openapi.json");
pub const GUIDE_VERSION: &str = "2026-11-15";

/// Path prefixes the Worker itself answers. Anything else that is not a
/// dashboard asset gets a real 404 instead of the dashboard page.
pub const API_PREFIXES: &[&str] = &[
    "/organic/",
    "/inorganic/",
    "/internal/",
    "/scratch/",
    "/auth/",
];
pub const API_EXACT: &[&str] = &[
    "/health",
    "/llm-guide",
    "/llm-guide.md",
    "/llms.txt",
    "/.well-known/llms.txt",
    "/openapi.json",
    "/.well-known/openapi.json",
    "/mcp",
];

pub fn is_known_path(path: &str) -> bool {
    API_EXACT.contains(&path) || API_PREFIXES.iter().any(|p| path.starts_with(p))
}

fn cors(h: &mut Headers) -> Result<()> {
    h.set("access-control-allow-origin", "*")?;
    h.set(
        "access-control-allow-headers",
        "content-type, x-curatom-token, authorization, mcp-protocol-version, mcp-session-id",
    )?;
    h.set("access-control-allow-methods", "GET, POST, OPTIONS")?;
    Ok(())
}

pub fn text(status: u16, body: &str, content_type: &str) -> Result<Response> {
    let mut h = Headers::new();
    h.set("content-type", content_type)?;
    h.set("cache-control", "public, max-age=300")?;
    cors(&mut h)?;
    Ok(Response::ok(body)?.with_status(status).with_headers(h))
}

pub fn json_cors(status: u16, body: &Value) -> Result<Response> {
    let mut h = Headers::new();
    h.set("content-type", "application/json")?;
    cors(&mut h)?;
    Ok(Response::ok(body.to_string())?
        .with_status(status)
        .with_headers(h))
}

pub fn not_found(path: &str) -> Result<Response> {
    json_cors(
        404,
        &json!({
            "error": "not_found",
            "path": path,
            "hint": "AI agents: start at https://curatom.rajvansh.dev/llms.txt",
        }),
    )
}

/// Static, unauthenticated documents. `None` = not one of ours.
pub fn serve_static(method: &Method, path: &str) -> Option<Result<Response>> {
    if *method == Method::Options && (path == "/mcp" || API_EXACT.contains(&path)) {
        return Some(json_cors(204, &json!({})).map(|r| r.with_status(204)));
    }
    if *method != Method::Get && *method != Method::Head {
        return None;
    }
    match path {
        "/llms.txt" | "/.well-known/llms.txt" => {
            Some(text(200, LLMS_TXT, "text/plain; charset=utf-8"))
        }
        "/llm-guide.md" => Some(text(200, LLM_GUIDE, "text/markdown; charset=utf-8")),
        "/openapi.json" | "/.well-known/openapi.json" => {
            Some(text(200, OPENAPI_JSON, "application/json; charset=utf-8"))
        }
        _ => None,
    }
}

/// The key a request carries: `?token=` first (unchanged legacy path),
/// then the `x-curatom-token` header. `allow_bearer` is only set for
/// `/mcp`, so `Authorization` keeps its existing meaning everywhere else.
pub fn request_token(req: &Request, allow_bearer: bool) -> Option<String> {
    if let Ok(url) = req.url() {
        if let Some((_, v)) = url.query_pairs().find(|(k, _)| k == "token") {
            if !v.is_empty() {
                return Some(v.into_owned());
            }
        }
    }
    if let Ok(Some(v)) = req.headers().get("x-curatom-token") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    if allow_bearer {
        if let Ok(Some(v)) = req.headers().get("authorization") {
            if let Some(t) = v
                .strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
            {
                let t = t.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    None
}

/// Short, accurate text handed out with every newly issued key.
pub fn key_file_text(
    label: &str,
    token: &str,
    workspace_id: &str,
    expires_unix: i64,
    sandbox_id: Option<&str>,
) -> String {
    let expiry = if expires_unix == 0 {
        "never (until the operator revokes it)".to_string()
    } else {
        format!("unix time {expires_unix} (UTC)")
    };
    let sandbox = match sandbox_id {
        Some(s) if !s.is_empty() => {
            format!("{s}  (https://valhalla.rajvansh.dev/valhalla/{s}/status)")
        }
        _ => "not created yet".to_string(),
    };
    format!(
        "CURATOM KEY — Comfort Curators company workspace\n\
         ==============================================\n\
         Label (set by the operator): {label}\n\
         Workspace: {workspace_id}\n\
         Expires: {expiry}\n\
         Your computer (Valhalla sandbox): {sandbox}\n\
         Key: {token}\n\n\
         Keep the key secret. Send it as the header  x-curatom-token: <key>\n\
         (?token=<key> also works, but URLs end up in logs).\n\n\
         Start here (no key needed):\n\
         - https://curatom.rajvansh.dev/llms.txt       short index\n\
         - https://curatom.rajvansh.dev/llm-guide.md   full guide (what the key can and cannot do, every endpoint, limits, errors)\n\
         - https://curatom.rajvansh.dev/openapi.json   for function calling / GPT Actions\n\
         - https://curatom.rajvansh.dev/mcp            remote MCP server (Authorization: Bearer <key>)\n\n\
         First steps:\n\
         1. GET  /organic/keys/verify            -> confirms the key works\n\
         2. POST /inorganic/handoff  {{}}          -> what you may request\n\
         3. POST /inorganic/submit   {{name, reason, resources, permissions}}\n\
            200 = covered by a standing grant (data returned); 201 = waiting for the operator (88 s)\n\
         4. Notes: GET /scratch/write?kind=note&body=...   (private to this key; the operator can read them)\n\n\
         Be honest with the operator: if anything fails, looks wrong or unsafe, say so plainly.\n\
         Do not try to get around a refusal. Guide version {GUIDE_VERSION}.\n"
    )
}
