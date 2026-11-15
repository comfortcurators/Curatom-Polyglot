//! Remote MCP server at `/mcp` (Streamable HTTP transport, JSON responses,
//! stateless). Every tool is a thin wrapper over an existing key endpoint,
//! reached through the same owner-DO routing as the HTTP path, so MCP adds
//! no new authority: a key can do over MCP exactly what it can do over HTTP,
//! minus running commands on its computer (not exposed here on purpose).

use crate::agent_docs::{json_cors, request_token, GUIDE_VERSION, LLM_GUIDE};
use serde_json::{json, Value};
use worker::*;

const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

fn tools() -> Value {
    json!([
        {"name":"curatom_guide","description":"Return the full Curatom guide for AI agents (markdown). Needs no key.",
         "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true}},
        {"name":"verify_key","description":"Check that the key is valid; returns the workspace id.",
         "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true}},
        {"name":"list_resources","description":"Get the request form and the resources this key may ask for.",
         "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true}},
        {"name":"request_access","description":"Ask for resources. If a standing grant covers them the data comes back at once (standing=true); otherwise a pending request is shown to the operator, who has 88 seconds to approve.",
         "inputSchema":{"type":"object","required":["name","reason","resources"],"properties":{
            "name":{"type":"string","description":"How the operator will see you. Use an honest, recognisable name."},
            "reason":{"type":"string","description":"One plain sentence."},
            "resources":{"type":"array","items":{"type":"string"},"description":"e.g. company.whitepaper (see list_resources)"},
            "permissions":{"type":"array","items":{"type":"string","enum":["read","write"]},"default":["read"]},
            "duration":{"type":"object","description":"{\"kind\":\"single_use\"} (default) or {\"kind\":\"ttl\",\"seconds\":N}"}}}},
        {"name":"notes_write","description":"Write a note to this key's private notepad (max 16000 bytes). The operator can read it. Start a note with PROBLEM: to flag an issue.",
         "inputSchema":{"type":"object","required":["body"],"properties":{
            "body":{"type":"string"},
            "kind":{"type":"string","enum":["note","intent","pattern"],"default":"note"},
            "new_round":{"type":"boolean","default":false}}}},
        {"name":"notes_read","description":"Read the current notepad session.",
         "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true}},
        {"name":"notes_history","description":"List past notepad sessions.",
         "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true}},
        {"name":"computer_status","description":"Status of this key's own Valhalla computer (Linux sandbox).",
         "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true}}
    ])
}

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn tool_text(v: Value, is_error: bool) -> Value {
    let text = match &v {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    json!({"content":[{"type":"text","text":text}],"isError":is_error})
}

async fn call_do(
    env: &Env,
    token: &str,
    method: Method,
    path_and_query: &str,
    body: Option<Value>,
) -> Result<(u16, Value)> {
    let owner_key = crate::owner_key_or_default(env, Some(token))?;
    let url = format!("https://curatom.rajvansh.dev{path_and_query}");
    let mut init = RequestInit::new();
    init.with_method(method);
    if let Some(b) = body {
        let h = Headers::new();
        h.set("content-type", "application/json")?;
        init.with_headers(h);
        init.with_body(Some(wasm_bindgen::JsValue::from_str(&b.to_string())));
    }
    let req = Request::new_with_init(&url, &init)?;
    let mut resp = crate::forward_to_owner_do(req, env, owner_key).await?;
    let status = resp.status_code();
    let text = resp.text().await.unwrap_or_default();
    let v = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
    Ok((status, v))
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

async fn run_tool(env: &Env, token: Option<&str>, name: &str, args: &Value) -> Result<Value> {
    if name == "curatom_guide" {
        return Ok(tool_text(Value::String(LLM_GUIDE.to_string()), false));
    }
    let Some(token) = token else {
        return Ok(tool_text(
            json!({"error":"missing_key","hint":"send Authorization: Bearer <key> or x-curatom-token: <key>"}),
            true,
        ));
    };
    let t = enc(token);
    let (status, v) = match name {
        "verify_key" => {
            call_do(
                env,
                token,
                Method::Get,
                &format!("/organic/keys/verify?token={t}"),
                None,
            )
            .await?
        }
        "list_resources" => {
            call_do(
                env,
                token,
                Method::Post,
                "/inorganic/handoff",
                Some(json!({"token": token})),
            )
            .await?
        }
        "request_access" => {
            let mut body = json!({
                "token": token,
                "name": args.get("name").cloned().unwrap_or(Value::Null),
                "reason": args.get("reason").cloned().unwrap_or(Value::Null),
                "resources": args.get("resources").cloned().unwrap_or(Value::Null),
                "permissions": args.get("permissions").cloned().unwrap_or(json!(["read"])),
            });
            if let Some(d) = args.get("duration") {
                body["duration"] = d.clone();
            }
            call_do(env, token, Method::Post, "/inorganic/submit", Some(body)).await?
        }
        "notes_write" => {
            let body = args.get("body").and_then(|b| b.as_str()).unwrap_or("");
            let kind = args.get("kind").and_then(|b| b.as_str()).unwrap_or("note");
            let nr = if args
                .get("new_round")
                .and_then(|b| b.as_bool())
                .unwrap_or(false)
            {
                "&new_round=1"
            } else {
                ""
            };
            call_do(
                env,
                token,
                Method::Get,
                &format!(
                    "/scratch/write?token={t}&kind={}&body={}{nr}",
                    enc(kind),
                    enc(body)
                ),
                None,
            )
            .await?
        }
        "notes_read" => {
            call_do(
                env,
                token,
                Method::Get,
                &format!("/scratch/notes?token={t}"),
                None,
            )
            .await?
        }
        "notes_history" => {
            call_do(
                env,
                token,
                Method::Get,
                &format!("/scratch/history?token={t}"),
                None,
            )
            .await?
        }
        "computer_status" => {
            let (s, v) = call_do(
                env,
                token,
                Method::Get,
                &format!("/organic/keys/verify?token={t}"),
                None,
            )
            .await?;
            if s != 200 {
                (s, v)
            } else {
                let ws = v
                    .get("workspace_id")
                    .and_then(|w| w.as_str())
                    .unwrap_or("")
                    .to_string();
                let base = env
                    .var("VALHALLA_URL")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| "https://valhalla.rajvansh.dev".into());
                let h = Headers::new();
                h.set("x-curatom-token", token)?;
                let mut init = RequestInit::new();
                init.with_method(Method::Get).with_headers(h);
                let req =
                    Request::new_with_init(&format!("{base}/valhalla/vh-{ws}/status"), &init)?;
                let mut resp = Fetch::Request(req).send().await?;
                let st = resp.status_code();
                let text = resp.text().await.unwrap_or_default();
                (
                    st,
                    serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)),
                )
            }
        }
        _ => return Ok(tool_text(json!({"error":"unknown_tool","tool":name}), true)),
    };
    Ok(tool_text(
        json!({"http_status": status, "response": v}),
        status >= 400,
    ))
}

pub async fn handle(mut req: Request, env: &Env) -> Result<Response> {
    if req.method() == Method::Get {
        return json_cors(
            405,
            &json!({"error":"method_not_allowed","hint":"POST JSON-RPC 2.0 messages to /mcp; no SSE stream is offered. Guide: /llm-guide.md"}),
        );
    }
    if req.method() != Method::Post {
        return json_cors(405, &json!({"error":"method_not_allowed"}));
    }
    let token = request_token(&req, true);
    let body = req.text().await.unwrap_or_default();
    let msg: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return json_cors(400, &rpc_error(&Value::Null, -32700, "parse error")),
    };
    if msg.is_array() {
        return json_cors(
            400,
            &rpc_error(&Value::Null, -32600, "batching is not supported"),
        );
    }
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    if msg.get("id").is_none() {
        // notification (e.g. notifications/initialized): accepted, no body
        return Ok(Response::empty()?.with_status(202));
    }
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let out = match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let pv = if PROTOCOL_VERSIONS.contains(&asked) {
                asked
            } else {
                PROTOCOL_VERSIONS[0]
            };
            rpc_result(
                &id,
                json!({
                    "protocolVersion": pv,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "curatom", "title": "Curatom (Comfort Curators)", "version": GUIDE_VERSION},
                    "instructions": "Comfort Curators' company workspace. Call curatom_guide first. Send your Curatom key as Authorization: Bearer or x-curatom-token. Be honest with the operator and flag problems with notes_write (start with PROBLEM:)."
                }),
            )
        }
        "ping" => rpc_result(&id, json!({})),
        "tools/list" => rpc_result(&id, json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match run_tool(env, token.as_deref(), name, &args).await {
                Ok(r) => rpc_result(&id, r),
                Err(e) => rpc_result(&id, tool_text(json!({"error": e.to_string()}), true)),
            }
        }
        "resources/list" => rpc_result(&id, json!({"resources": []})),
        "prompts/list" => rpc_result(&id, json!({"prompts": []})),
        _ => rpc_error(&id, -32601, "method not found"),
    };
    json_cors(200, &out)
}
