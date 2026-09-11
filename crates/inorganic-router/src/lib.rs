//! Inorganic request/status views. Still no live tokens.

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
