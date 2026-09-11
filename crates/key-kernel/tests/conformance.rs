//! Conformance: the capability rules, pinned against the real kernel surface.
//!
//! Every method called here exists on `Kernel` today. If a test stops
//! compiling, the surface moved and the surface is the API -- fix the caller
//! or say so in the commit, do not invent a type to make it build.
//!
//! Memory substrate only. Reload means: same store, a new `Kernel`, `load()`.

use curatom_key_kernel::Kernel;
use curatom_organic_router::{activity_view, approval_view, intent_view, leak_check};
use curatom_ports::StateStore;
use curatom_protocol::*;
use curatom_substrate_memory::{FrozenClock, MemoryArtifacts, MemoryLedger, MemoryStateStore};
use pollster::block_on;

type K = Kernel<MemoryStateStore, MemoryLedger, MemoryArtifacts, FrozenClock>;

struct Rig {
    store: MemoryStateStore,
    ledger: MemoryLedger,
}

impl Rig {
    fn boot(unix: i64) -> (K, Rig) {
        let rig = Rig {
            store: MemoryStateStore::new(),
            ledger: MemoryLedger::new(),
        };
        (rig.kernel(unix), rig)
    }

    /// A kernel over the same store. Calling this twice is a restart.
    fn kernel(&self, unix: i64) -> K {
        let mut k = Kernel::new(
            "org_owner".into(),
            self.store.clone(),
            self.ledger.clone(),
            MemoryArtifacts::new(),
            FrozenClock::new(unix),
        );
        block_on(k.load()).unwrap();
        k
    }
}

/// intent -> approve -> grant, the path everything else starts from.
fn granted(k: &mut K, text: &str) -> CapabilitySecret {
    block_on(k.create_intent(text, "org_owner")).unwrap();
    let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
    block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
    block_on(k.issue_grant(&appr.id)).unwrap().unwrap().handle.secret
}

fn inorganic(resources: Vec<&str>, duration: Duration) -> InorganicRequest {
    InorganicRequest {
        requester_id: "fleet.curatom".into(),
        reason: "read inventory".into(),
        resources: resources.into_iter().map(String::from).collect(),
        permissions: vec![Permission::Read],
        duration,
    }
}

#[test]
fn grant_single_use_consumes_once() {
    let (mut k, _rig) = Rig::boot(1_700_000_000);
    let s = granted(&mut k, "Fix HostOS");
    block_on(k.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Read))
        .unwrap();
    let err =
        block_on(k.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Read))
            .unwrap_err();
    assert_eq!(err, "already_consumed");
}

#[test]
fn grant_ttl_expires() {
    let (mut k, _rig) = Rig::boot(1000);
    let appr = block_on(k.submit_inorganic(inorganic(
        vec!["hostos.inventory"],
        Duration::Ttl { seconds: 10 },
    )))
    .unwrap();
    block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
    let s = block_on(k.issue_grant(&appr.id)).unwrap().unwrap().handle.secret;
    assert_eq!(s.expires_unix, 1010);

    k.clock().set(1011);
    let err =
        block_on(k.consume_capability(&s.token, "fleet.curatom", "hostos.inventory", Permission::Read))
            .unwrap_err();
    assert_eq!(err, "capability_expired");
}

#[test]
fn grant_read_does_not_authorize_write() {
    let (mut k, _rig) = Rig::boot(1);
    let s = granted(&mut k, "Fix HostOS");
    let err =
        block_on(k.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Write))
            .unwrap_err();
    assert_eq!(err, "permission_not_granted");
}

#[test]
fn grant_resource_scoped() {
    let (mut k, _rig) = Rig::boot(1);
    let s = granted(&mut k, "Fix HostOS");
    assert!(!s.resources.iter().any(|r| r == "cloudflare.inventory"));
    let err = block_on(k.consume_capability(
        &s.token,
        &s.requester_id,
        "cloudflare.inventory",
        Permission::Read,
    ))
    .unwrap_err();
    assert_eq!(err, "resource_not_granted");
}

#[test]
fn grant_requester_scoped() {
    let (mut k, _rig) = Rig::boot(1);
    let s = granted(&mut k, "Fix HostOS");
    let err = block_on(k.consume_capability(
        &s.token,
        "someone.else",
        "hostos.inventory",
        Permission::Read,
    ))
    .unwrap_err();
    assert_eq!(err, "requester_mismatch");
}

#[test]
fn grant_unknown_token_fails_closed() {
    let (mut k, _rig) = Rig::boot(1);
    let s = granted(&mut k, "Fix HostOS");
    let err = block_on(k.consume_capability(
        "tok_not_a_real_token",
        &s.requester_id,
        "hostos.inventory",
        Permission::Read,
    ))
    .unwrap_err();
    assert_eq!(err, "unknown_token");
}

#[test]
fn consumed_grant_survives_reload() {
    let (mut k, rig) = Rig::boot(1);
    let s = granted(&mut k, "Fix HostOS");
    block_on(k.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Read))
        .unwrap();
    drop(k);

    let mut k2 = rig.kernel(2);
    let err =
        block_on(k2.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Read))
            .unwrap_err();
    assert_eq!(err, "already_consumed");
}

#[test]
fn approval_binds_request_digest() {
    let (mut k, rig) = Rig::boot(1);
    block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
    let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
    drop(k);

    // Widen the card underneath the owner without re-deriving its digest.
    // This is what a compromised store, or a careless migration, looks like.
    let mut state = block_on(rig.store.get()).unwrap().unwrap();
    state
        .approvals
        .get_mut(&appr.id)
        .unwrap()
        .resources
        .push("cloudflare.inventory".into());
    block_on(rig.store.put(&state)).unwrap();

    let mut k = rig.kernel(1);
    let err = block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap_err();
    assert_eq!(err, "digest_mismatch");
    assert!(block_on(k.issue_grant(&appr.id)).unwrap().is_none());
}

#[test]
fn refuse_does_not_issue_grant() {
    let (mut k, _rig) = Rig::boot(1);
    block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
    let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
    block_on(k.decide_approval(&appr.id, ApprovalDecision::Refuse)).unwrap();
    assert!(block_on(k.issue_grant(&appr.id)).unwrap().is_none());
}

#[test]
fn duplicate_approval_does_not_mint_second_grant() {
    let (mut k, rig) = Rig::boot(1);
    block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
    let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
    block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
    let first = block_on(k.issue_grant(&appr.id)).unwrap().unwrap();

    assert!(block_on(k.issue_grant(&appr.id)).unwrap().is_none());
    // and not across a restart either
    drop(k);
    let mut k2 = rig.kernel(2);
    assert!(block_on(k2.issue_grant(&appr.id)).unwrap().is_none());

    let state = block_on(rig.store.get()).unwrap().unwrap();
    assert_eq!(state.grants.len(), 1);
    assert!(state.grants.contains_key(&first.grant_id));
}

#[test]
fn pending_approval_survives_reload() {
    let (mut k, rig) = Rig::boot(1);
    block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
    let before = k.list_pending_approvals().unwrap();
    drop(k);

    let k2 = rig.kernel(2);
    let after = k2.list_pending_approvals().unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, before[0].id);
    assert_eq!(after[0].digest, before[0].digest);
    assert_eq!(after[0].status, ApprovalStatus::Pending);
}

#[test]
fn ledger_contains_no_capability_token() {
    let (mut k, rig) = Rig::boot(1);
    let s = granted(&mut k, "Fix HostOS");
    block_on(k.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Read))
        .unwrap();

    for (kind, body) in rig.ledger.entries() {
        let line = format!("{kind} {body}");
        assert!(!line.contains(&s.token), "ledger holds a live token: {line}");
        assert!(!line.contains(&s.grant_id), "ledger holds a grant id: {line}");
    }
}

#[test]
fn organic_view_contains_no_capability_token() {
    let (mut k, _rig) = Rig::boot(1);
    let intent = block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
    let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
    block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
    let s = block_on(k.issue_grant(&appr.id)).unwrap().unwrap().handle.secret;
    block_on(k.consume_capability(&s.token, &s.requester_id, "hostos.inventory", Permission::Read))
        .unwrap();

    let views = vec![
        serde_json::to_value(approval_view(&appr)).unwrap(),
        intent_view(&k.get_intent(&intent.id).unwrap().unwrap()),
        serde_json::to_value(
            k.activity().unwrap().iter().map(activity_view).collect::<Vec<_>>(),
        )
        .unwrap(),
    ];
    for v in &views {
        leak_check(v).unwrap();
        let dumped = v.to_string();
        assert!(!dumped.contains(&s.token), "organic view leaks a token: {dumped}");
        assert!(!dumped.contains(&s.grant_id), "organic view leaks a grant id: {dumped}");
    }
}

#[test]
fn unknown_resource_rejected_at_submit() {
    let (mut k, _rig) = Rig::boot(1);
    let err = block_on(k.submit_inorganic(inorganic(
        vec!["hostos.inventory", "aws.everything"],
        Duration::SingleUse,
    )))
    .unwrap_err();
    assert_eq!(err, "unknown_resource:aws.everything");
    assert!(k.list_pending_approvals().unwrap().is_empty());
}
