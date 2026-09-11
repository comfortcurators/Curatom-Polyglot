//! Inorganic handoff/submit. Token is the only credential. No login.

use curatom_protocol::*;
use serde_json::{json, Value};

pub fn submitted_view(a: &Approval) -> Value {
    json!({
        "id": a.id,
        "status": "pending_approval",
        "resources": a.resources,
    })
}

pub fn status_view(a: &Approval) -> Value {
    let status = match a.status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Refused => "refused",
    };
    json!({
        "id": a.id,
        "status": status,
    })
}

pub fn handoff_form(token: &str) -> Value {
    json!({
        "message": "Fill this form and POST it to /inorganic/submit with the same token.",
        "form": {
            "token": token,
            "name": "<the name you want the operator to see you as>",
            "reason": "<one-sentence plain-language reason>",
            "resources": ["<resource-id>"],
            "permissions": ["read"],
            "duration": { "kind": "single_use" }
        },
        "available_resources": [
            "hostos.inventory",
            "hostos.metadata",
            "cloudflare.inventory",
            "company.whitepaper",
            "company.inventory",
            "repository.inventory"
        ],
        "available_permissions": ["read", "write"],
        "note": "The operator will see your request. They have 88 seconds to approve. If they do not recognize your name, they will not approve it."
    })
}

pub struct SubmitBody {
    pub token: String,
    pub name: String,
    pub reason: String,
    pub resources: Vec<String>,
    pub permissions: Vec<Permission>,
    pub duration: Duration,
}

pub fn parse_submit(body: &str) -> Result<SubmitBody, (u16, Value)> {
    let body: Value = serde_json::from_str(body)
        .map_err(|_| (400, json!({"error":"invalid_json"})))?;
    let token = body
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or((400, json!({"error":"missing_token"})))?
        .to_string();
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or((400, json!({"error":"missing_name"})))?
        .to_string();
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .ok_or((400, json!({"error":"missing_reason"})))?
        .to_string();
    let resources_v = body
        .get("resources")
        .and_then(|v| v.as_array())
        .ok_or((400, json!({"error":"missing_resources"})))?;
    let resources: Vec<String> = resources_v
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let perms_v = body
        .get("permissions")
        .and_then(|v| v.as_array())
        .ok_or((400, json!({"error":"missing_permissions"})))?;
    let mut permissions = Vec::new();
    for p in perms_v {
        match p.as_str() {
            Some("read") => permissions.push(Permission::Read),
            Some("write") => permissions.push(Permission::Write),
            _ => return Err((400, json!({"error":"bad_permission"}))),
        }
    }
    let duration = match body.get("duration") {
        None => Duration::SingleUse,
        Some(dur_v) => parse_duration(dur_v)?,
    };
    Ok(SubmitBody {
        token,
        name,
        reason,
        resources,
        permissions,
        duration,
    })
}

fn parse_duration(v: &Value) -> Result<Duration, (u16, Value)> {
    if let Ok(d) = serde_json::from_value::<Duration>(v.clone()) {
        return Ok(d);
    }
    match v
        .get("mode")
        .or_else(|| v.get("kind"))
        .and_then(|x| x.as_str())
    {
        Some("single_use") | Some("single-use") => Ok(Duration::SingleUse),
        Some("ttl") => {
            let seconds = v
                .get("seconds")
                .and_then(|s| s.as_u64())
                .ok_or((400, json!({"error":"bad_duration"})))?;
            Ok(Duration::Ttl { seconds })
        }
        _ => Err((400, json!({"error":"bad_duration"}))),
    }
}
