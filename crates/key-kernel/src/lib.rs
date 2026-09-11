//! Capability kernel. TTL as integers. No Cloudflare imports.

use curatom_crypto::{random_id, request_digest};
use curatom_ports::{ArtifactStore, Clock, EventLedger, StateStore};
use curatom_protocol::*;
use curatom_resource_registry::known_resource;

pub struct Kernel<S, L, A, C>
where
    S: StateStore,
    L: EventLedger,
    A: ArtifactStore,
    C: Clock,
{
    owner_id: String,
    store: S,
    ledger: L,
    #[allow(dead_code)]
    artifacts: A,
    clock: C,
    state: Option<KernelState>,
}

impl<S, L, A, C> Kernel<S, L, A, C>
where
    S: StateStore,
    L: EventLedger,
    A: ArtifactStore,
    C: Clock,
{
    pub fn new(owner_id: String, store: S, ledger: L, artifacts: A, clock: C) -> Self {
        Self {
            owner_id,
            store,
            ledger,
            artifacts,
            clock,
            state: None,
        }
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    pub async fn load(&mut self) -> Result<(), String> {
        let mut state = self.store.get().await?.unwrap_or_default();
        if state.owner_id.is_empty() {
            state.owner_id = self.owner_id.clone();
        }
        self.state = Some(state);
        Ok(())
    }

    fn st(&self) -> Result<&KernelState, String> {
        self.state.as_ref().ok_or_else(|| "kernel not loaded".into())
    }

    fn st_mut(&mut self) -> Result<&mut KernelState, String> {
        self.state.as_mut().ok_or_else(|| "kernel not loaded".into())
    }

    async fn persist(&mut self) -> Result<(), String> {
        let s = self.st()?.clone();
        self.store.put(&s).await
    }

    fn note(&mut self, kind: &str, summary: impl Into<String>, intent_id: Option<String>, approval_id: Option<String>) {
        let at = self.clock.now_iso();
        if let Ok(st) = self.st_mut() {
            st.activity.push(Activity {
                kind: kind.into(),
                summary: summary.into(),
                at,
                intent_id,
                approval_id,
            });
        }
    }

    pub fn parse_intent_text(text: &str) -> (Vec<String>, Vec<Permission>) {
        let t = text.to_ascii_lowercase();
        let mut resources = Vec::new();
        if t.contains("cloudflare") {
            resources.push("cloudflare.inventory".to_string());
        }
        if t.contains("hostos") || t.contains("host os") || resources.is_empty() {
            resources.push("hostos.inventory".to_string());
            resources.push("hostos.metadata".to_string());
        }
        resources.retain(|r| known_resource(r));
        if resources.is_empty() {
            resources.push("hostos.inventory".to_string());
        }
        (resources, vec![Permission::Read])
    }

    pub async fn create_intent(&mut self, text: &str, requester_id: &str) -> Result<Intent, String> {
        let (resources, permissions) = Self::parse_intent_text(text);
        let intent = Intent {
            id: random_id("int"),
            text: text.to_string(),
            requester_id: requester_id.to_string(),
            resources: resources.clone(),
            permissions: permissions.clone(),
            duration: Duration::SingleUse,
            created_at: self.clock.now_iso(),
        };
        let approval = Approval {
            id: random_id("appr"),
            intent_id: intent.id.clone(),
            requester: "fleet.curatom".into(),
            reason: text.to_string(),
            resources,
            permissions,
            duration: Duration::SingleUse,
            digest: request_digest("fleet.curatom", &intent.resources, text),
            status: ApprovalStatus::Pending,
        };
        let iid = intent.id.clone();
        let aid = approval.id.clone();
        {
            let st = self.st_mut()?;
            st.intents.insert(iid.clone(), intent.clone());
            st.approvals.insert(aid.clone(), approval);
        }
        self.note("intent.created", "I heard you.", Some(iid), Some(aid));
        let _ = self.ledger.append("intent.created", &intent.id).await;
        self.persist().await?;
        Ok(intent)
    }

    pub async fn submit_inorganic(&mut self, req: InorganicRequest) -> Result<Approval, String> {
        for r in &req.resources {
            if !known_resource(r) {
                return Err(format!("unknown_resource:{r}"));
            }
        }
        let intent = Intent {
            id: random_id("int"),
            text: req.reason.clone(),
            requester_id: req.requester_id.clone(),
            resources: req.resources.clone(),
            permissions: req.permissions.clone(),
            duration: req.duration.clone(),
            created_at: self.clock.now_iso(),
        };
        let approval = Approval {
            id: random_id("appr"),
            intent_id: intent.id.clone(),
            requester: req.requester_id.clone(),
            reason: req.reason.clone(),
            resources: req.resources.clone(),
            permissions: req.permissions.clone(),
            duration: req.duration.clone(),
            digest: request_digest(&req.requester_id, &req.resources, &req.reason),
            status: ApprovalStatus::Pending,
        };
        let iid = intent.id.clone();
        let aid = approval.id.clone();
        {
            let st = self.st_mut()?;
            st.intents.insert(iid.clone(), intent);
            st.approvals.insert(aid.clone(), approval.clone());
        }
        self.note(
            "inorganic.submitted",
            format!("{} wants access", req.requester_id),
            Some(iid),
            Some(aid),
        );
        self.persist().await?;
        Ok(approval)
    }

    pub fn get_intent(&self, id: &str) -> Result<Option<Intent>, String> {
        Ok(self.st()?.intents.get(id).cloned())
    }

    pub fn list_pending_approvals(&self) -> Result<Vec<Approval>, String> {
        let mut v: Vec<_> = self
            .st()?
            .approvals
            .values()
            .filter(|a| a.status == ApprovalStatus::Pending)
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(v)
    }

    pub fn get_approval(&self, id: &str) -> Result<Option<Approval>, String> {
        Ok(self.st()?.approvals.get(id).cloned())
    }

    pub fn has_owner(&self) -> bool {
        self.st().map(|s| s.owner.is_some()).unwrap_or(false)
    }

    pub fn get_owner(&self) -> Option<OwnerRecord> {
        self.st().ok().and_then(|s| s.owner.clone())
    }

    pub async fn enroll_owner(&mut self, record: OwnerRecord) -> Result<OwnerRecord, String> {
        if self.st()?.owner.is_some() {
            return Err("owner_already_enrolled".into());
        }
        let oid = record.owner_id.clone();
        {
            let st = self.st_mut()?;
            st.owner = Some(record.clone());
        }
        self.note(
            "owner.enrolled",
            format!("{oid} enrolled"),
            None,
            None,
        );
        self.persist().await?;
        Ok(record)
    }

    pub async fn decide_approval(
        &mut self,
        approval_id: &str,
        decision: ApprovalDecision,
    ) -> Result<Option<Approval>, String> {
        let status = match decision {
            ApprovalDecision::Approve => ApprovalStatus::Approved,
            ApprovalDecision::Refuse => ApprovalStatus::Refused,
        };
        {
            let st = self.st_mut()?;
            let Some(a) = st.approvals.get_mut(approval_id) else {
                return Ok(None);
            };
            if a.status != ApprovalStatus::Pending {
                return Ok(None);
            }
            // The approval is a decision about a specific request. Re-derive the
            // digest from what the card now says and refuse if it has moved --
            // otherwise approving an inventory read could hand out a grant for
            // something the owner never saw.
            if decision == ApprovalDecision::Approve {
                let expected = request_digest(&a.requester, &a.resources, &a.reason);
                if expected != a.digest {
                    return Err("digest_mismatch".into());
                }
            }
            a.status = status;
        }
        let a = self.get_approval(approval_id)?.unwrap();
        let kind = match decision {
            ApprovalDecision::Approve => "approval.approved",
            ApprovalDecision::Refuse => "approval.refused",
        };
        let summary = match decision {
            ApprovalDecision::Approve => "Approved.".to_string(),
            ApprovalDecision::Refuse => "Refused.".to_string(),
        };
        self.note(kind, summary, Some(a.intent_id.clone()), Some(a.id.clone()));
        self.persist().await?;
        Ok(Some(a))
    }

    pub async fn issue_grant(&mut self, approval_id: &str) -> Result<Option<IssuedGrant>, String> {
        let approval = match self.get_approval(approval_id)? {
            Some(a) if a.status == ApprovalStatus::Approved => a,
            _ => return Ok(None),
        };
        // One approval, one grant. A second call is not a second permission.
        if self.st()?.issued.contains(approval_id) {
            return Ok(None);
        }
        let now = self.clock.now_unix();
        let (expires_unix, expires_at) = match &approval.duration {
            Duration::SingleUse => (0, None),
            Duration::Ttl { seconds } => {
                let exp = now + *seconds as i64;
                (exp, Some(exp.to_string()))
            }
        };
        let secret = CapabilitySecret {
            token: random_id("tok"),
            grant_id: random_id("grt"),
            requester_id: approval.requester.clone(),
            resources: approval.resources.clone(),
            permissions: approval.permissions.clone(),
            request_digest: approval.digest.clone(),
            intent_id: approval.intent_id.clone(),
            expires_at,
            expires_unix,
        };
        {
            let st = self.st_mut()?;
            st.grants.insert(secret.grant_id.clone(), secret.clone());
            st.issued.insert(approval_id.to_string());
        }
        self.note(
            "grant.issued",
            "A grant was issued.",
            Some(approval.intent_id.clone()),
            Some(approval.id.clone()),
        );
        self.persist().await?;
        Ok(Some(IssuedGrant {
            grant_id: secret.grant_id.clone(),
            approval_id: approval_id.to_string(),
            handle: GrantHandle { secret },
        }))
    }

    pub async fn consume_capability(
        &mut self,
        token: &str,
        requester_id: &str,
        resource: &str,
        op: Permission,
    ) -> Result<CapabilitySecret, String> {
        let now = self.clock.now_unix();
        let grant = {
            let st = self.st()?;
            st.grants
                .values()
                .find(|g| g.token == token)
                .cloned()
                .ok_or_else(|| "unknown_token".to_string())?
        };
        if grant.requester_id != requester_id {
            return Err("requester_mismatch".into());
        }
        if !grant.resources.iter().any(|r| r == resource) {
            return Err("resource_not_granted".into());
        }
        if !grant.permissions.iter().any(|p| *p == op) {
            return Err("permission_not_granted".into());
        }
        // TTL: only when expires_at is Some. expires_unix == 0 is single-use.
        if grant.expires_at.is_some() && grant.expires_unix <= now {
            return Err("capability_expired".into());
        }
        let key = KernelState::consume_key(&grant.grant_id, resource, op);
        {
            let st = self.st_mut()?;
            if st.consumed.contains(&key) {
                return Err("already_consumed".into());
            }
            st.consumed.insert(key);
        }
        self.note(
            "capability.consumed",
            format!("spent {resource} {}", op.as_str()),
            None,
            None,
        );
        self.persist().await?;
        Ok(grant)
    }

    pub async fn record_outcome(&mut self, o: Outcome) -> Result<(), String> {
        let summary = if o.ok {
            match o.resource.as_str() {
                "hostos.inventory" => "[MOCK] HostOS is healthy. I found 7 services.".into(),
                "hostos.metadata" => "[MOCK] HostOS metadata read.".into(),
                "cloudflare.inventory" => "[MOCK] Cloudflare inventory read.".into(),
                _ => "Done.".into(),
            }
        } else {
            format!("Failed: {}", o.error.clone().unwrap_or_else(|| "unknown".into()))
        };
        let kind = if o.ok { "outcome.recorded" } else { "execution.failed" };
        {
            let st = self.st_mut()?;
            st.outcomes.push(o.clone());
        }
        self.note(kind, summary, Some(o.intent_id.clone()), None);
        self.persist().await?;
        Ok(())
    }

    pub fn activity(&self) -> Result<Vec<Activity>, String> {
        Ok(self.st()?.activity.clone())
    }

    pub fn outcomes_for_intent(&self, intent_id: &str) -> Result<Vec<Outcome>, String> {
        Ok(self
            .st()?
            .outcomes
            .iter()
            .filter(|o| o.intent_id == intent_id)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curatom_substrate_memory::{FrozenClock, MemoryArtifacts, MemoryLedger, MemoryStateStore};

    fn k(unix: i64) -> Kernel<MemoryStateStore, MemoryLedger, MemoryArtifacts, FrozenClock> {
        let mut k = Kernel::new(
            "org_owner".into(),
            MemoryStateStore::new(),
            MemoryLedger::new(),
            MemoryArtifacts::new(),
            FrozenClock::new(unix),
        );
        pollster::block_on(k.load()).unwrap();
        k
    }

    #[test]
    fn consume_is_single_use_per_pair() {
        let mut k = k(1_700_000_000);
        let intent = pollster::block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
        let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
        assert_eq!(appr.intent_id, intent.id);
        pollster::block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
        let issued = pollster::block_on(k.issue_grant(&appr.id)).unwrap().unwrap();
        let secret = issued.handle.secret;
        assert_eq!(secret.intent_id, intent.id);
        assert_ne!(secret.intent_id, secret.request_digest);
        let g = pollster::block_on(k.consume_capability(
            &secret.token,
            &secret.requester_id,
            "hostos.inventory",
            Permission::Read,
        ))
        .unwrap();
        assert_eq!(g.grant_id, secret.grant_id);
        let err = pollster::block_on(k.consume_capability(
            &secret.token,
            &secret.requester_id,
            "hostos.inventory",
            Permission::Read,
        ))
        .unwrap_err();
        assert_eq!(err, "already_consumed");
        // a different resource on the same grant still works
        pollster::block_on(k.consume_capability(
            &secret.token,
            &secret.requester_id,
            "hostos.metadata",
            Permission::Read,
        ))
        .unwrap();
    }

    #[test]
    fn ttl_compares_integers_not_dates() {
        let mut k = k(1000);
        let req = InorganicRequest {
            requester_id: "fleet.curatom".into(),
            reason: "read inventory".into(),
            resources: vec!["hostos.inventory".into()],
            permissions: vec![Permission::Read],
            duration: Duration::Ttl { seconds: 30 },
        };
        let appr = pollster::block_on(k.submit_inorganic(req)).unwrap();
        pollster::block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
        let issued = pollster::block_on(k.issue_grant(&appr.id)).unwrap().unwrap();
        assert_eq!(issued.handle.secret.expires_unix, 1030);
        pollster::block_on(k.consume_capability(
            &issued.handle.secret.token,
            "fleet.curatom",
            "hostos.inventory",
            Permission::Read,
        ))
        .unwrap();
    }

    #[test]
    fn expired_grant_refuses() {
        let mut k = k(1000);
        let req = InorganicRequest {
            requester_id: "fleet.curatom".into(),
            reason: "read inventory".into(),
            resources: vec!["hostos.inventory".into()],
            permissions: vec![Permission::Read],
            duration: Duration::Ttl { seconds: 10 },
        };
        let appr = pollster::block_on(k.submit_inorganic(req)).unwrap();
        pollster::block_on(k.decide_approval(&appr.id, ApprovalDecision::Approve)).unwrap();
        let issued = pollster::block_on(k.issue_grant(&appr.id)).unwrap().unwrap();
        k.clock().set(1011);
        let err = pollster::block_on(k.consume_capability(
            &issued.handle.secret.token,
            "fleet.curatom",
            "hostos.inventory",
            Permission::Read,
        ))
        .unwrap_err();
        assert_eq!(err, "capability_expired");
    }

    #[test]
    fn refuse_does_not_issue() {
        let mut k = k(1);
        pollster::block_on(k.create_intent("Fix HostOS", "org_owner")).unwrap();
        let appr = k.list_pending_approvals().unwrap().into_iter().next().unwrap();
        pollster::block_on(k.decide_approval(&appr.id, ApprovalDecision::Refuse)).unwrap();
        assert!(pollster::block_on(k.issue_grant(&appr.id)).unwrap().is_none());
    }
}
