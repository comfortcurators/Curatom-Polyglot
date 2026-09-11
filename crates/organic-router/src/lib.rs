//! Organic views. If a token can appear here, the view is wrong.

use curatom_protocol::*;
use serde_json::{json, Value};

pub fn approval_view(a: &Approval) -> ApprovalView {
    ApprovalView {
        id: a.id.clone(),
        requester: a.requester.clone(),
        reason: a.reason.clone(),
        resources: a.resources.clone(),
        permissions: a.permissions.iter().map(|p| p.as_str().to_string()).collect(),
        duration: a.duration.label(),
    }
}

pub fn activity_view(a: &Activity) -> ActivityView {
    ActivityView {
        kind: a.kind.clone(),
        summary: a.summary.clone(),
    }
}

pub fn me_view(
    owner_id: &str,
    enrolled: bool,
    enrolled_at: Option<String>,
    display_name: &str,
) -> Value {
    json!({
        "id": owner_id,
        "display_name": display_name,
        "mode": "owner",
        "enrolled": enrolled,
        "enrolled_at": enrolled_at,
        "kind": "organic",
        "role": "owner",
    })
}

pub fn intent_view(i: &Intent) -> Value {
    json!({
        "id": i.id,
        "text": i.text,
        "created_at": i.created_at,
        "status": "recorded",
    })
}

pub fn leak_check(v: &Value) -> Result<(), String> {
    assert_organic_safe(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_do_not_serialize_grants() {
        let a = Approval {
            id: "appr_1".into(),
            intent_id: "int_1".into(),
            requester: "fleet.curatom".into(),
            reason: "read".into(),
            resources: vec!["hostos.inventory".into()],
            permissions: vec![Permission::Read],
            duration: Duration::SingleUse,
            digest: "abc".into(),
            status: ApprovalStatus::Pending,
            decision_signature_digest: None,
            decision_signature_ref: None,
        };
        let v = serde_json::to_value(approval_view(&a)).unwrap();
        leak_check(&v).unwrap();
        assert!(v.get("token").is_none());
    }
}
