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
        at: a.at.clone(),
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
    fn activity_view_carries_at_for_the_dashboard() {
        // The dashboard's own ActivityEvent (apps/dashboard/src/api.rs)
        // requires a non-optional `at` field, and this view used to have
        // no such field at all -- every real activity fetch failed to
        // deserialize, and the dashboard's `if let Ok(list) = ...`
        // silently swallowed that, showing "Nothing yet." regardless of
        // how much activity actually existed. Confirmed live before this
        // fix: a real account's GET /organic/activity returned exactly
        // `{"kind":...,"summary":...}`, no `at`. This test would have
        // caught it without needing a live account to find out.
        let a = Activity {
            kind: "knock.approved".into(),
            summary: "Approved.".into(),
            at: "2026-09-18T16:58:20.927Z".into(),
            intent_id: None,
            approval_id: None,
        };
        let v = serde_json::to_value(activity_view(&a)).unwrap();
        assert_eq!(v.get("at").and_then(|x| x.as_str()), Some("2026-09-18T16:58:20.927Z"));
        assert_eq!(v.get("kind").and_then(|x| x.as_str()), Some("knock.approved"));
    }

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
        };
        let v = serde_json::to_value(approval_view(&a)).unwrap();
        leak_check(&v).unwrap();
        assert!(v.get("token").is_none());
    }
}
