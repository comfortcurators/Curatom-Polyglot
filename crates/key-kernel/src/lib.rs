//! Capability kernel. TTL as integers. No Cloudflare imports.

use curatom_crypto::{iso_plus_secs, random_id, request_digest, sha256_hex};
use curatom_ports::{ArtifactStore, Clock, DirtyKinds, EventLedger, StateStore};
use curatom_protocol::*;
use curatom_resource_registry::known_resource;

/// Storage-budget caps for the two unbounded `Vec`s in `KernelState`.
///
/// DO storage values are limited to 128 KiB. Every entry in `activity`
/// is roughly 150-250 bytes of JSON; every entry in `outcomes` is
/// larger and variable because `Outcome::data` carries an arbitrary
/// `serde_json::Value`. Left unbounded, `activity` grows past the limit
/// at roughly 500-800 entries and `outcomes` at some smaller number
/// depending on what the data payloads look like. The failure mode is
/// the DO's `put` returning an error, which `persist` surfaces as a
/// `String` and the caller sees as a 500 -- delayed and attributed to
/// whatever unrelated mutation happens to hit the limit first.
///
/// These are rolling windows, not the audit log. The design's "all
/// intents, patterns, kept" promise is fulfilled by D1 (see
/// `workers/api/src/scratchpad.rs`'s `mirror_note`), not by the in-DO
/// `activity` Vec. The Vec exists so the dashboard feed can be served
/// from DO state without a D1 round-trip per poll; a rolling window is
/// what a feed is.
///
/// Budget math, worst case:
///   256 activity entries * ~250 B = ~64 KiB
///    32 outcome entries  * ~1.3 KB = ~42 KiB
/// plus the small entity collections (tokens, knocks, connectors,
/// repositories, meta) which grow only on explicit user action, not
/// from automated traffic. Comfortably under 128 KiB for a typical
/// owner. A pathological owner with thousands of checkpoints across
/// many tokens can still exceed it -- see the note at
/// `create_checkpoint` for why that is a separate concern.
const MAX_ACTIVITY_ENTRIES: usize = 256;
const MAX_OUTCOME_ENTRIES: usize = 32;

/// Outcome data payloads larger than this are replaced with a
/// truncation marker (`{"truncated": true, "original_bytes": N,
/// "preview": "..."}`) before being stored. 1 KiB is generous for the
/// structured responses this system's connectors produce
/// (`{"workers":1,"buckets":1}` and the like) and small enough that 32
/// of them fit in a DO storage value with headroom for everything
/// else. A whitepaper read, whose `data` is the full whitepaper text,
/// is the case this exists for: without it, a single such outcome
/// could exceed the per-value limit on its own.
const MAX_OUTCOME_DATA_BYTES: usize = 1_024;

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
    /// Which `KernelState` fields have changed since the last successful
    /// `persist()`. Set by every mutation, cleared by `persist()`. Never
    /// serialized -- it describes the write that is *about to* happen,
    /// not anything about the state itself.
    dirty: DirtyKinds,
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
            dirty: DirtyKinds::new(),
        }
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    pub async fn load(&mut self) -> Result<(), String> {
        let mut state = self.store.load().await?.unwrap_or_default();
        if state.owner_id.is_empty() {
            state.owner_id = self.owner_id.clone();
        }
        self.state = Some(state);
        // Loading does not mark anything dirty: whatever the store held
        // is now the baseline, and `persist()` on a fresh kernel with no
        // mutations writes nothing.
        self.dirty = DirtyKinds::new();
        Ok(())
    }

    fn st(&self) -> Result<&KernelState, String> {
        self.state.as_ref().ok_or_else(|| "kernel not loaded".into())
    }

    /// The whole current `KernelState`, by reference. Exists for exactly
    /// one caller -- `h_admin_export` in the Worker, which serializes it
    /// to JSON for owner-initiated backup. Every other read path in this
    /// crate goes through a narrower accessor (`list_keys`, `activity`,
    /// `get_whitepaper`, ...), and the reader should keep it that way.
    /// This is deliberately the only method that hands out everything.
    pub fn export_state(&self) -> Result<&KernelState, String> {
        self.st()
    }

    fn st_mut(&mut self) -> Result<&mut KernelState, String> {
        self.state.as_mut().ok_or_else(|| "kernel not loaded".into())
    }

    async fn persist(&mut self) -> Result<(), String> {
        if self.dirty.is_empty() {
            return Ok(());
        }
        let s = self.st()?.clone();
        let dirty = self.dirty;
        self.store.persist(&s, &dirty).await?;
        // Clear only after a successful write. A failed persist leaves
        // the dirty set intact so the next persist retries the same
        // fields -- the alternative silently loses a mutation.
        self.dirty = DirtyKinds::new();
        Ok(())
    }

    /// Mark a field dirty. Called by every mutating method after it has
    /// actually changed the corresponding slice of `self.state`. Small
    /// and inline; the compiler folds the repeated calls into a single
    /// store because `DirtyKinds` is a plain `u16`.
    fn mark(&mut self, kind: DirtyKinds) {
        self.dirty = self.dirty.with(kind);
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
            // Rolling window -- drop the oldest when over the cap. A
            // single `drain(0..n)` rather than repeated `remove(0)`
            // because drain is O(n) in the number dropped, not O(n^2).
            if st.activity.len() > MAX_ACTIVITY_ENTRIES {
                let drop = st.activity.len() - MAX_ACTIVITY_ENTRIES;
                st.activity.drain(0..drop);
            }
        }
        // `note` pushes into `activity` on every call, so it owns the
        // ACTIVITY bit itself -- callers that mark ACTIVITY too are
        // redundant but not wrong; the bit is idempotent.
        self.mark(DirtyKinds::ACTIVITY);
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
        self.mark(DirtyKinds::INTENTS.with(DirtyKinds::APPROVALS).with(DirtyKinds::ACTIVITY));
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
        self.mark(DirtyKinds::INTENTS.with(DirtyKinds::APPROVALS).with(DirtyKinds::ACTIVITY));
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
        self.mark(DirtyKinds::META.with(DirtyKinds::ACTIVITY));
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
        self.mark(DirtyKinds::APPROVALS.with(DirtyKinds::ACTIVITY));
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
        self.mark(DirtyKinds::GRANTS.with(DirtyKinds::ISSUED).with(DirtyKinds::ACTIVITY));
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
        self.mark(DirtyKinds::CONSUMED.with(DirtyKinds::ACTIVITY));
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
        let o = cap_outcome_data(o);
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
            // Rolling window, same discipline as `activity` above.
            if st.outcomes.len() > MAX_OUTCOME_ENTRIES {
                let drop = st.outcomes.len() - MAX_OUTCOME_ENTRIES;
                st.outcomes.drain(0..drop);
            }
        }
        self.mark(DirtyKinds::OUTCOMES.with(DirtyKinds::ACTIVITY));
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

    pub fn token_matches(&self, token: &str) -> bool {
        let Ok(st) = self.st() else {
            return false;
        };
        match st.tokens.get(token) {
            Some(t) if t.expires_unix == 0 => true,
            Some(t) => self.clock.now_unix() < t.expires_unix,
            None => false,
        }
    }

    pub fn get_key(&self, token: &str) -> Option<OrganicToken> {
        self.st().ok()?.tokens.get(token).cloned()
    }

    pub fn list_keys(&self) -> Vec<OrganicToken> {
        let mut v: Vec<OrganicToken> = self
            .st()
            .map(|s| s.tokens.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub async fn create_key(
        &mut self,
        label: String,
        validity_seconds: Option<u64>,
    ) -> Result<OrganicToken, String> {
        let label = label.trim().to_string();
        if label.is_empty() {
            return Err("label_required".into());
        }
        if label.len() > 80 {
            return Err("label_too_long".into());
        }
        if let Some(0) = validity_seconds {
            return Err("validity_must_be_positive".into());
        }
        // The prefix used to be a hardcoded "RAJVANSH-" for every
        // account, not just the founder's -- which also meant nothing in
        // the token said which owner it belonged to. A machine presenting
        // a token (a knock) carries no session cookie, ever, so the
        // Worker-level router has no other way to find this owner's own
        // Durable Object. Embedding `owner_id` here is what makes that
        // routing possible without a second index to keep in sync -- see
        // `fetch()`'s handling of `/inorganic/*` in `workers/api/src/lib.rs`.
        let raw = random_id("tok").to_uppercase().replace('_', "-");
        let token = format!("{}.{raw}", self.owner_id);
        let now_unix = self.clock.now_unix();
        let expires_unix = match validity_seconds {
            Some(n) => now_unix + n as i64,
            None => 0,
        };
        let t = OrganicToken {
            token: token.clone(),
            owner_id: self.owner_id.clone(),
            label: label.clone(),
            created_at: self.clock.now_iso(),
            last_used_at: None,
            // Minted once, here, and never rotated -- see the field's own
            // doc comment in curatom-protocol for why a revoke+recreate is
            // deliberately a new workspace rather than the old one
            // continuing under a new token.
            workspace_id: format!("{}.{}", self.owner_id, random_id("ws")),
            validity_seconds,
            expires_unix,
        };
        {
            let st = self.st_mut()?;
            st.tokens.insert(t.token.clone(), t.clone());
        }
        self.mark(DirtyKinds::TOKENS.with(DirtyKinds::ACTIVITY));
        self.note(
            "key.created",
            format!("{label} {}", &token[..token.len().min(24)]),
            None,
            None,
        );
        self.persist().await?;
        Ok(t)
    }

    pub async fn revoke_key(&mut self, token: &str) -> Result<(), String> {
        {
            let st = self.st_mut()?;
            if st.tokens.remove(token).is_none() {
                return Err("unknown_token".into());
            }
        }
        self.mark(DirtyKinds::TOKENS.with(DirtyKinds::ACTIVITY));
        self.note(
            "key.revoked",
            token[..token.len().min(24)].to_string(),
            None,
            None,
        );
        self.persist().await
    }

    /// Reroll: mint a replacement for `token` with the same label and
    /// the same lifetime, and kill the old key in the same step. The
    /// design distinguishes this from revoke -- revoke says "stop using
    /// this key," reroll says "this key leaked or is being cycled; here
    /// is its successor." One is a stop, the other is a transition, and
    /// the audit trail has to say which happened.
    pub async fn reroll_key(&mut self, token: &str) -> Result<OrganicToken, String> {
        let old = self
            .get_key(token)
            .ok_or_else(|| "unknown_token".to_string())?;
        {
            let st = self.st_mut()?;
            st.tokens.remove(token);
        }
        self.mark(DirtyKinds::TOKENS.with(DirtyKinds::ACTIVITY));
        self.note(
            "key.rerolled",
            format!("{} ({} -> successor)", old.label, &token[..token.len().min(24)]),
            None,
            None,
        );
        self.persist().await?;
        self.create_key(old.label, old.validity_seconds).await
    }

    /// Delete: like revoke, but also drops this key's own scoped state
    /// (its checkpoint history). The knock history, ledger entries, and
    /// billboard rows are deliberately *not* touched -- the design says a
    /// key is "documented everywhere across your enterprise," and
    /// deleting the key should not delete the record of what it was used
    /// for.
    pub async fn delete_key(&mut self, token: &str) -> Result<(), String> {
        let label = {
            let st = self.st_mut()?;
            match st.tokens.remove(token) {
                Some(t) => t.label,
                None => return Err("unknown_token".into()),
            }
        };
        {
            let st = self.st_mut()?;
            st.checkpoints.remove(token);
        }
        self.mark(DirtyKinds::TOKENS.with(DirtyKinds::CHECKPOINTS).with(DirtyKinds::ACTIVITY));
        self.note(
            "key.deleted",
            format!("{label} {}", &token[..token.len().min(24)]),
            None,
            None,
        );
        self.persist().await
    }

    pub fn key_log(&self, token: &str) -> Vec<KeyLogEntry> {
        let Ok(st) = self.st() else {
            return Vec::new();
        };
        let mut out: Vec<KeyLogEntry> = Vec::new();
        for k in st.knocks.values().filter(|k| k.token == token) {
            out.push(KeyLogEntry {
                kind: "knock.created".into(),
                at: k.created_at.clone(),
                knock_id: Some(k.id.clone()),
                name: Some(k.name.clone()),
                reason: Some(k.reason.clone()),
            });
            match k.status {
                KnockStatus::Approved => out.push(KeyLogEntry {
                    kind: "knock.approved".into(),
                    at: k.decided_at.clone().unwrap_or_default(),
                    knock_id: Some(k.id.clone()),
                    name: Some(k.name.clone()),
                    reason: None,
                }),
                KnockStatus::Refused => out.push(KeyLogEntry {
                    kind: "knock.refused".into(),
                    at: k.decided_at.clone().unwrap_or_default(),
                    knock_id: Some(k.id.clone()),
                    name: Some(k.name.clone()),
                    reason: None,
                }),
                KnockStatus::Expired => out.push(KeyLogEntry {
                    kind: "knock.expired".into(),
                    at: k.decided_at.clone().unwrap_or_default(),
                    knock_id: Some(k.id.clone()),
                    name: Some(k.name.clone()),
                    reason: None,
                }),
                KnockStatus::Pending => {}
            }
        }
        out.sort_by(|a, b| b.at.cmp(&a.at));
        out
    }

    pub async fn create_knock(
        &mut self,
        token: String,
        name: String,
        reason: String,
        resources: Vec<String>,
        permissions: Vec<Permission>,
        duration: Duration,
    ) -> Result<Knock, String> {
        if !self.token_matches(&token) {
            return Err("token_not_recognized".into());
        }
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("name_required".into());
        }
        if name.len() > 200 {
            return Err("name_too_long".into());
        }
        if reason.len() > 2000 {
            return Err("reason_too_long".into());
        }
        if resources.is_empty() || permissions.is_empty() {
            return Err("empty_scope".into());
        }
        for r in &resources {
            if let Some(f) = self.is_frozen(r, None) {
                return Err(format!(
                    "resource_frozen:{}:freeze_id={}:reason={}",
                    r, f.id, f.reason
                ));
            }
        }
        let mut resources = resources;
        resources.sort();
        resources.dedup();
        let mut permissions = permissions;
        permissions.sort();
        permissions.dedup();

        let canonical = serde_json::json!({
            "owner_id": self.owner_id,
            "token": token,
            "name": name,
            "reason": reason,
            "resources": resources,
            "permissions": permissions.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "duration": duration,
        });
        let digest = sha256_hex(canonical.to_string().as_bytes());
        let created = self.clock.now_iso();
        let expires_unix = self.clock.now_unix() + KNOCK_TTL_SECS;
        let expires = iso_plus_secs(&created, KNOCK_TTL_SECS);
        let k = Knock {
            id: random_id("knock"),
            owner_id: self.owner_id.clone(),
            token,
            name,
            reason,
            resources,
            permissions,
            duration,
            created_at: created,
            expires_at: expires,
            expires_unix,
            status: KnockStatus::Pending,
            decided_at: None,
            request_digest: digest,
        };
        let kid = k.id.clone();
        let kname = k.name.clone();
        let tok = k.token.clone();
        let used_at = k.created_at.clone();
        {
            let st = self.st_mut()?;
            st.knocks.insert(kid.clone(), k.clone());
            if let Some(t) = st.tokens.get_mut(&tok) {
                t.last_used_at = Some(used_at);
            }
        }
        self.mark(DirtyKinds::KNOCKS.with(DirtyKinds::TOKENS).with(DirtyKinds::ACTIVITY));
        self.note("knock.created", kname, Some(kid), None);
        self.persist().await?;
        Ok(k)
    }

    pub fn list_pending_knocks(&self) -> Result<Vec<Knock>, String> {
        let now = self.clock.now_unix();
        let now_iso = self.clock.now_iso();
        let mut v: Vec<_> = self
            .st()?
            .knocks
            .values()
            .filter(|k| k.status == KnockStatus::Pending && !knock_expired(k, now, &now_iso))
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(v)
    }

    pub fn get_knock(&self, id: &str) -> Result<Option<Knock>, String> {
        Ok(self.st()?.knocks.get(id).cloned())
    }

    pub async fn sweep_expired_knocks(&mut self) -> Result<(), String> {
        let now = self.clock.now_unix();
        let now_iso = self.clock.now_iso();
        let expired: Vec<String> = self
            .st()?
            .knocks
            .values()
            .filter(|k| k.status == KnockStatus::Pending && knock_expired(k, now, &now_iso))
            .map(|k| k.id.clone())
            .collect();
        if expired.is_empty() {
            return Ok(());
        }
        {
            let st = self.st_mut()?;
            for id in &expired {
                if let Some(k) = st.knocks.get_mut(id) {
                    k.status = KnockStatus::Expired;
                    k.decided_at = Some(now_iso.clone());
                }
            }
        }
        self.mark(DirtyKinds::KNOCKS.with(DirtyKinds::ACTIVITY));
        for id in expired {
            self.note("knock.expired", id, None, None);
        }
        self.persist().await
    }

    pub async fn decide_knock(
        &mut self,
        knock_id: &str,
        decision: KnockStatus,
    ) -> Result<Knock, String> {
        if !matches!(decision, KnockStatus::Approved | KnockStatus::Refused) {
            return Err("bad_decision".into());
        }
        let now = self.clock.now_unix();
        let now_iso = self.clock.now_iso();
        let expired_now;
        {
            let st = self.st_mut()?;
            let k = st
                .knocks
                .get_mut(knock_id)
                .ok_or_else(|| "unknown_knock".to_string())?;
            if k.status != KnockStatus::Pending {
                return Err("knock_not_pending".into());
            }
            if knock_expired(k, now, &now_iso) {
                k.status = KnockStatus::Expired;
                k.decided_at = Some(now_iso.clone());
                expired_now = true;
            } else {
                k.status = decision;
                k.decided_at = Some(now_iso);
                expired_now = false;
            }
        }
        self.mark(DirtyKinds::KNOCKS.with(DirtyKinds::ACTIVITY));
        if expired_now {
            self.persist().await?;
            return Err("knock_expired".into());
        }
        let k = self.get_knock(knock_id)?.unwrap();
        let ev = match decision {
            KnockStatus::Approved => "knock.approved",
            KnockStatus::Refused => "knock.refused",
            _ => "knock.decided",
        };
        self.note(ev, &k.name, Some(k.id.clone()), None);
        self.persist().await?;
        Ok(k)
    }

    pub fn is_frozen(&self, scope: &str, caller_session: Option<&str>) -> Option<Freeze> {
        let Ok(st) = self.st() else {
            return None;
        };
        for f in st.freezes.values() {
            if f.released_at.is_some() {
                continue;
            }
            if f.scope != scope {
                continue;
            }
            if let (Some(owner), Some(caller)) = (f.session_id.as_deref(), caller_session) {
                if owner == caller {
                    continue;
                }
            }
            return Some(f.clone());
        }
        None
    }

    pub fn list_frozen(&self) -> Vec<Freeze> {
        self.st()
            .map(|s| {
                s.freezes
                    .values()
                    .filter(|f| f.released_at.is_none())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Token + approved knock whose resources cover every declared scope id.
    /// Stops a valid token from freezing live resources without a tap.
    pub fn authorize_valhalla_provision(
        &self,
        token: &str,
        knock_id: &str,
        scope: &[String],
    ) -> Result<(), String> {
        if !self.token_matches(token) {
            return Err("token_not_recognized".into());
        }
        if knock_id.is_empty() {
            return Err("missing_knock_id".into());
        }
        if scope.is_empty() {
            return Err("missing_scope".into());
        }
        let knock = self.get_knock(knock_id)?.ok_or_else(|| "unknown_knock".to_string())?;
        if knock.token != token {
            return Err("token_knock_mismatch".into());
        }
        if knock.status != KnockStatus::Approved {
            return Err("knock_not_approved".into());
        }
        for r in scope {
            if !knock.resources.iter().any(|x| x == r) {
                return Err(format!("scope_not_granted:{r}"));
            }
        }
        Ok(())
    }

    pub async fn freeze(
        &mut self,
        scope: String,
        reason: String,
        session_id: Option<String>,
    ) -> Result<Freeze, String> {
        let f = Freeze {
            id: random_id("frz"),
            scope: scope.clone(),
            reason: reason.clone(),
            session_id: session_id.clone(),
            created_at: self.clock.now_iso(),
            released_at: None,
            release_reason: None,
            release_parity_ok: None,
        };
        {
            let st = self.st_mut()?;
            st.freezes.insert(f.id.clone(), f.clone());
        }
        self.mark(DirtyKinds::FREEZES.with(DirtyKinds::ACTIVITY));
        self.note("freeze.created", format!("{scope} {reason}"), None, None);
        self.persist().await?;
        Ok(f)
    }

    pub async fn release_freeze(
        &mut self,
        freeze_id: &str,
        parity_ok: bool,
        reason: String,
    ) -> Result<Freeze, String> {
        let now = self.clock.now_iso();
        let result = {
            let st = self.st_mut()?;
            let f = st
                .freezes
                .get_mut(freeze_id)
                .ok_or_else(|| "unknown_freeze".to_string())?;
            if f.released_at.is_some() {
                return Err("already_released".into());
            }
            f.released_at = Some(now);
            f.release_reason = Some(reason.clone());
            f.release_parity_ok = Some(parity_ok);
            f.clone()
        };
        self.mark(DirtyKinds::FREEZES.with(DirtyKinds::ACTIVITY));
        self.note("freeze.released", freeze_id.to_string(), None, None);
        self.persist().await?;
        Ok(result)
    }

    // ---- compute connectors ----
    //
    // Curatom ships zero connectors pre-wired to any account, this
    // owner's included. hostos and hostos-mcp are one founder's own
    // Cloudflare Workers, reachable to nobody by default -- adding them
    // here is exactly the same action as any other user adding their
    // own MCP endpoint: this owner supplies the URL and whatever auth
    // it needs, same as everyone else, no special-cased identity check
    // anywhere in this file.

    pub fn list_connectors(&self) -> Vec<Connector> {
        let mut v: Vec<Connector> = self
            .st()
            .map(|s| s.connectors.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub fn get_connector(&self, id: &str) -> Option<Connector> {
        self.st().ok()?.connectors.get(id).cloned()
    }

    pub async fn create_connector(
        &mut self,
        name: String,
        endpoint: String,
        headers: Vec<ConnectorHeader>,
    ) -> Result<Connector, String> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("name_required".into());
        }
        if name.len() > 80 {
            return Err("name_too_long".into());
        }
        let endpoint = endpoint.trim().to_string();
        if !(endpoint.starts_with("https://") || endpoint.starts_with("http://")) {
            return Err("endpoint_must_be_a_url".into());
        }
        if endpoint.len() > 2048 {
            return Err("endpoint_too_long".into());
        }
        if headers.len() > 20 {
            return Err("too_many_headers".into());
        }
        for h in &headers {
            if h.name.trim().is_empty() || h.name.len() > 200 || h.value.len() > 4000 {
                return Err("bad_header".into());
            }
        }
        let c = Connector {
            id: random_id("conn"),
            owner_id: self.owner_id.clone(),
            name: name.clone(),
            endpoint,
            headers,
            created_at: self.clock.now_iso(),
        };
        {
            let st = self.st_mut()?;
            st.connectors.insert(c.id.clone(), c.clone());
        }
        self.mark(DirtyKinds::CONNECTORS.with(DirtyKinds::ACTIVITY));
        self.note("connector.created", name, None, None);
        self.persist().await?;
        Ok(c)
    }

    pub async fn delete_connector(&mut self, id: &str) -> Result<(), String> {
        let name = {
            let st = self.st_mut()?;
            match st.connectors.remove(id) {
                Some(c) => c.name,
                None => return Err("unknown_connector".into()),
            }
        };
        self.mark(DirtyKinds::CONNECTORS.with(DirtyKinds::ACTIVITY));
        self.note("connector.deleted", name, None, None);
        self.persist().await
    }

    // ---- data: repositories ----

    pub fn list_repositories(&self) -> Vec<Repository> {
        let mut v: Vec<Repository> = self
            .st()
            .map(|s| s.repositories.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub fn get_repository(&self, id: &str) -> Option<Repository> {
        self.st().ok()?.repositories.get(id).cloned()
    }

    pub async fn create_repository(
        &mut self,
        name: String,
        github_token: Option<String>,
    ) -> Result<Repository, String> {
        let name = name.trim().to_string();
        let valid_shape = {
            let parts: Vec<&str> = name.split('/').collect();
            parts.len() == 2
                && !parts[0].is_empty()
                && !parts[1].is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        };
        if !valid_shape {
            return Err("name_must_be_owner_slash_repo".into());
        }
        if self.st()?.repositories.values().any(|r| r.name == name) {
            return Err("repository_already_added".into());
        }
        let r = Repository {
            id: random_id("repo"),
            owner_id: self.owner_id.clone(),
            name: name.clone(),
            github_token,
            created_at: self.clock.now_iso(),
            last_synced_at: None,
            last_sync_file_count: None,
            manifest_ref: None,
        };
        {
            let st = self.st_mut()?;
            st.repositories.insert(r.id.clone(), r.clone());
        }
        self.mark(DirtyKinds::REPOSITORIES.with(DirtyKinds::ACTIVITY));
        self.note("repository.added", name, None, None);
        self.persist().await?;
        Ok(r)
    }

    pub async fn delete_repository(&mut self, id: &str) -> Result<(), String> {
        let name = {
            let st = self.st_mut()?;
            match st.repositories.remove(id) {
                Some(r) => r.name,
                None => return Err("unknown_repository".into()),
            }
        };
        self.mark(DirtyKinds::REPOSITORIES.with(DirtyKinds::ACTIVITY));
        self.note("repository.removed", name, None, None);
        self.persist().await
    }

    /// Called by the Worker after it actually fetches GitHub and writes
    /// the manifest to R2 -- this crate does no I/O, so the sync itself
    /// happens in `lib.rs`; this just records that it happened.
    pub async fn mark_repository_synced(
        &mut self,
        id: &str,
        file_count: usize,
        manifest_ref: String,
    ) -> Result<Repository, String> {
        let synced_at = self.clock.now_iso();
        let name = {
            let st = self.st_mut()?;
            let r = st.repositories.get_mut(id).ok_or_else(|| "unknown_repository".to_string())?;
            r.last_synced_at = Some(synced_at.clone());
            r.last_sync_file_count = Some(file_count);
            r.manifest_ref = Some(manifest_ref);
            r.name.clone()
        };
        self.mark(DirtyKinds::REPOSITORIES.with(DirtyKinds::ACTIVITY));
        self.note(
            "repository.synced",
            format!("{name}: {file_count} files"),
            None,
            None,
        );
        self.persist().await?;
        Ok(self.get_repository(id).unwrap())
    }

    // ---- checkpoints ----
    //
    // A named point in one key's own history. Bounded on purpose: what a
    // checkpoint snapshots is up to whoever creates one (by hand today,
    // a future sandbox action later) -- this crate only owns the record
    // that it happened, keyed to the token so it can never be listed or
    // created against a key this owner does not hold.

    pub fn list_checkpoints(&self, token: &str) -> Vec<Checkpoint> {
        self.st()
            .ok()
            .and_then(|s| s.checkpoints.get(token).cloned())
            .unwrap_or_default()
    }

    /// Marker-only checkpoint. Preserves the pre-Phase-4 signature so
    /// existing callers and tests keep working; a caller that has a
    /// snapshot (the Worker, after Valhalla has uploaded one) uses
    /// `create_checkpoint_with_snapshot` instead.
    pub async fn create_checkpoint(
        &mut self,
        token: &str,
        note: String,
    ) -> Result<Checkpoint, String> {
        self.create_checkpoint_with_snapshot(token, note, None, 0).await
    }

    /// Create a checkpoint, optionally with a snapshot behind it. The
    /// snapshot itself is created by the Worker calling out to Valhalla
    /// -- this crate does no I/O, so the only thing it does with the
    /// ref is record it. See `workers/valhalla/src/index.ts`'s
    /// `/snapshot` endpoint for the writer, and `/restore` for the
    /// reader.
    pub async fn create_checkpoint_with_snapshot(
        &mut self,
        token: &str,
        note: String,
        snapshot_ref: Option<String>,
        file_count: u64,
    ) -> Result<Checkpoint, String> {
        if !self.token_matches(token) {
            return Err("token_not_recognized".into());
        }
        let note = note.trim().to_string();
        if note.len() > 2000 {
            return Err("note_too_long".into());
        }
        if let Some(ref r) = snapshot_ref {
            // Path-traversal guard. The ref is opaque to the kernel --
            // it never reads it -- but it is echoed back to the
            // dashboard and passed to Valhalla on restore, so a
            // malformed one is worth rejecting at the boundary rather
            // than trusting whatever produced it.
            if r.len() > 512 || r.contains("..") {
                return Err("bad_snapshot_ref".into());
            }
        }
        let c = Checkpoint {
            id: random_id("chk"),
            note: note.clone(),
            created_at: self.clock.now_iso(),
            snapshot_ref,
            file_count,
        };
        {
            let st = self.st_mut()?;
            let list = st.checkpoints.entry(token.to_string()).or_default();
            list.push(c.clone());
            // Bounded history -- the current checkpoint is the last one;
            // older ones are logs, not something anyone pages through.
            if list.len() > 200 {
                let overflow = list.len() - 200;
                list.drain(0..overflow);
            }
        }
        self.mark(DirtyKinds::CHECKPOINTS.with(DirtyKinds::ACTIVITY));
        self.note("checkpoint.created", note, None, None);
        self.persist().await?;
        Ok(c)
    }

    /// Look up a single checkpoint by id, scoped to its token. Returns
    /// `None` if the id is unknown or belongs to a different key. Used
    /// by the Worker's `/internal/checkpoint-resolve`, which Valhalla
    /// calls during provision to fetch the manifest ref for a
    /// checkpoint it was told to restore from.
    pub fn get_checkpoint(
        &self,
        token: &str,
        checkpoint_id: &str,
    ) -> Result<Option<Checkpoint>, String> {
        Ok(self
            .st()?
            .checkpoints
            .get(token)
            .and_then(|list| list.iter().find(|c| c.id == checkpoint_id).cloned()))
    }

    // ---- whitepaper ----
    //
    // This owner's own company context. Per-account, never shared: no
    // key belonging to any *other* owner can read into it, and it is
    // never the founder's document leaking into someone else's account.
    // `None` until the owner writes one -- there is no default text.

    const WHITEPAPER_MAX_LEN: usize = 20_000;

    pub fn get_whitepaper(&self) -> Option<String> {
        self.st().ok().and_then(|s| s.whitepaper.clone())
    }

    pub async fn set_whitepaper(&mut self, text: String) -> Result<(), String> {
        if text.len() > Self::WHITEPAPER_MAX_LEN {
            return Err("whitepaper_too_long".into());
        }
        let is_clear = text.trim().is_empty();
        {
            let st = self.st_mut()?;
            st.whitepaper = if is_clear { None } else { Some(text) };
        }
        self.mark(DirtyKinds::WHITEPAPER.with(DirtyKinds::ACTIVITY));
        self.note(
            if is_clear { "whitepaper.cleared" } else { "whitepaper.updated" },
            String::new(),
            None,
            None,
        );
        self.persist().await
    }

    pub async fn issue_knock_grant(&mut self, knock: &Knock) -> Result<Option<IssuedGrant>, String> {
        if self.st()?.issued.contains(&knock.id) {
            return Ok(None);
        }
        if knock.status != KnockStatus::Approved {
            return Ok(None);
        }
        let now = self.clock.now_unix();
        let (expires_unix, expires_at) = match &knock.duration {
            Duration::SingleUse => (0, None),
            Duration::Ttl { seconds } => {
                let exp = now + *seconds as i64;
                (exp, Some(exp.to_string()))
            }
        };
        let secret = CapabilitySecret {
            token: random_id("tok"),
            grant_id: random_id("grt"),
            requester_id: format!("knock:{}", knock.id),
            resources: knock.resources.clone(),
            permissions: knock.permissions.clone(),
            request_digest: knock.request_digest.clone(),
            intent_id: knock.id.clone(),
            expires_at,
            expires_unix,
        };
        {
            let st = self.st_mut()?;
            st.grants.insert(secret.grant_id.clone(), secret.clone());
            st.issued.insert(knock.id.clone());
        }
        self.mark(DirtyKinds::GRANTS.with(DirtyKinds::ISSUED).with(DirtyKinds::ACTIVITY));
        self.note(
            "grant.issued",
            "A grant was issued.",
            Some(knock.id.clone()),
            None,
        );
        self.persist().await?;
        Ok(Some(IssuedGrant {
            grant_id: secret.grant_id.clone(),
            approval_id: knock.id.clone(),
            handle: GrantHandle { secret },
        }))
    }
}

/// Replace an oversized `Outcome::data` with a truncation marker before
/// the outcome lands in DO state. See `MAX_OUTCOME_DATA_BYTES` for the
/// budget reasoning.
///
/// Serializing to check the size is the only reliable way to answer
/// "is this too big?" for an arbitrary JSON value -- a recursive
/// size estimator would be more code for no benefit, and would have to
/// be kept in sync with the serializer anyway. The cost is one
/// `to_string()` per recorded outcome; outcomes arrive at human action
/// speed, not request speed, so this is invisible.
///
/// Truncation is byte-bounded but character-safe: the preview is cut at
/// the largest byte index that is also a character boundary, so a
/// multi-byte UTF-8 sequence is never split in half. `original_bytes`
/// reports the pre-truncation serialized size, so a consumer can tell
/// it was cut and by how much.
fn cap_outcome_data(mut o: Outcome) -> Outcome {
    if let Some(data) = &o.data {
        let serialized = data.to_string();
        if serialized.len() > MAX_OUTCOME_DATA_BYTES {
            let mut end = MAX_OUTCOME_DATA_BYTES;
            while end > 0 && !serialized.is_char_boundary(end) {
                end -= 1;
            }
            o.data = Some(serde_json::json!({
                "truncated": true,
                "original_bytes": serialized.len(),
                "preview": &serialized[..end],
            }));
        }
    }
    o
}

fn knock_expired(k: &Knock, now_unix: i64, now_iso: &str) -> bool {
    if k.expires_unix != 0 {
        now_unix > k.expires_unix
    } else {
        now_iso > k.expires_at.as_str()
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

    #[test]
    fn token_rotate_kills_old() {
        let mut k = k(1_700_000_000);
        let t1 = pollster::block_on(k.create_key("one".into(), None)).unwrap();
        assert!(t1.token.starts_with("org_owner."));
        assert!(k.token_matches(&t1.token));
        pollster::block_on(k.revoke_key(&t1.token)).unwrap();
        let t2 = pollster::block_on(k.create_key("two".into(), None)).unwrap();
        assert_ne!(t1.token, t2.token);
        assert!(!k.token_matches(&t1.token));
        assert!(k.token_matches(&t2.token));
    }

    #[test]
    fn token_carries_its_owner_for_unauthenticated_machine_routing() {
        // The whole point of the prefix: a machine presenting this token
        // has no session cookie, so `fetch()` in the Worker must be able
        // to recover this Durable Object's own name (`owner_id`) from the
        // token text alone. If this regresses back to a fixed prefix,
        // every non-founder account's keys silently stop being reachable
        // by any knock again -- see workers/api/src/lib.rs's `/inorganic/*`
        // handling.
        let mut k = Kernel::new(
            "user_e659792109db4885a21923e3042ab112".into(),
            MemoryStateStore::new(),
            MemoryLedger::new(),
            MemoryArtifacts::new(),
            FrozenClock::new(1_700_000_000),
        );
        pollster::block_on(k.load()).unwrap();
        let t = pollster::block_on(k.create_key("mine".into(), None)).unwrap();
        let recovered_owner = t.token.split_once('.').map(|(prefix, _)| prefix);
        assert_eq!(recovered_owner, Some("user_e659792109db4885a21923e3042ab112"));
    }

    #[test]
    fn knock_expires_after_ttl() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("t".into(), None)).unwrap();
        let kn = pollster::block_on(k.create_knock(
            t.token.clone(),
            "Claude".into(),
            "read inventory".into(),
            vec!["hostos.inventory".into()],
            vec![Permission::Read],
            Duration::SingleUse,
        ))
        .unwrap();
        assert_eq!(k.list_pending_knocks().unwrap().len(), 1);
        k.clock().set(1_700_000_000 + 89);
        pollster::block_on(k.sweep_expired_knocks()).unwrap();
        assert!(k.list_pending_knocks().unwrap().is_empty());
        let got = k.get_knock(&kn.id).unwrap().unwrap();
        assert_eq!(got.status, KnockStatus::Expired);
    }

    #[test]
    fn unknown_token_cannot_knock() {
        let mut k = k(1_700_000_000);
        let err = pollster::block_on(k.create_knock(
            "RAJVANSH-NOPE".into(),
            "X".into(),
            "r".into(),
            vec!["hostos.inventory".into()],
            vec![Permission::Read],
            Duration::SingleUse,
        ))
        .unwrap_err();
        assert_eq!(err, "token_not_recognized");
    }

    #[test]
    fn freeze_blocks_new_knock() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("t".into(), None)).unwrap();
        pollster::block_on(k.freeze(
            "hostos.inventory".into(),
            "valhalla_session".into(),
            Some("vh-1".into()),
        ))
        .unwrap();
        let err = pollster::block_on(k.create_knock(
            t.token.clone(),
            "Intruder".into(),
            "r".into(),
            vec!["hostos.inventory".into()],
            vec![Permission::Read],
            Duration::SingleUse,
        ))
        .unwrap_err();
        assert!(err.starts_with("resource_frozen:hostos.inventory:"));
        assert_eq!(k.list_frozen().len(), 1);
        let fid = k.list_frozen()[0].id.clone();
        pollster::block_on(k.release_freeze(&fid, true, "done".into())).unwrap();
        assert!(k.list_frozen().is_empty());
        pollster::block_on(k.create_knock(
            t.token,
            "After".into(),
            "r".into(),
            vec!["hostos.inventory".into()],
            vec![Permission::Read],
            Duration::SingleUse,
        ))
        .unwrap();
    }

    #[test]
    fn provision_requires_approved_knock() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("t".into(), None)).unwrap();
        let scope = vec!["hostos.inventory".to_string()];
        assert_eq!(
            k.authorize_valhalla_provision(&t.token, "knock_nope", &scope)
                .unwrap_err(),
            "unknown_knock"
        );
        let kn = pollster::block_on(k.create_knock(
            t.token.clone(),
            "V".into(),
            "r".into(),
            vec!["hostos.inventory".into()],
            vec![Permission::Read],
            Duration::SingleUse,
        ))
        .unwrap();
        assert_eq!(
            k.authorize_valhalla_provision(&t.token, &kn.id, &scope)
                .unwrap_err(),
            "knock_not_approved"
        );
        pollster::block_on(k.decide_knock(&kn.id, KnockStatus::Approved)).unwrap();
        k.authorize_valhalla_provision(&t.token, &kn.id, &scope)
            .unwrap();
        assert_eq!(
            k.authorize_valhalla_provision(
                &t.token,
                &kn.id,
                &["valhalla.workspace".into()]
            )
            .unwrap_err(),
            "scope_not_granted:valhalla.workspace"
        );
    }

    #[test]
    fn connector_header_values_never_round_trip_by_accident() {
        // Not a real assertion of secrecy (JSON serialization of Connector
        // does include the value -- the dashboard's own view redacts it,
        // see plate_compute.rs). This is the create/list/delete contract:
        // a header survives storage, and deleting a connector is real.
        let mut k = k(1_700_000_000);
        let c = pollster::block_on(k.create_connector(
            "my mcp".into(),
            "https://example.workers.dev/mcp".into(),
            vec![ConnectorHeader { name: "Authorization".into(), value: "Bearer x".into() }],
        ))
        .unwrap();
        assert_eq!(k.list_connectors().len(), 1);
        assert_eq!(k.list_connectors()[0].headers[0].value, "Bearer x");
        pollster::block_on(k.delete_connector(&c.id)).unwrap();
        assert!(k.list_connectors().is_empty());
    }

    #[test]
    fn connector_rejects_a_non_url_endpoint() {
        let mut k = k(1_700_000_000);
        let err = pollster::block_on(k.create_connector(
            "bad".into(),
            "not-a-url".into(),
            vec![],
        ))
        .unwrap_err();
        assert_eq!(err, "endpoint_must_be_a_url");
    }

    #[test]
    fn deleting_an_unknown_connector_fails_closed() {
        let mut k = k(1_700_000_000);
        let err = pollster::block_on(k.delete_connector("conn_nope")).unwrap_err();
        assert_eq!(err, "unknown_connector");
    }

    #[test]
    fn repository_name_must_be_owner_slash_repo() {
        let mut k = k(1_700_000_000);
        let err = pollster::block_on(k.create_repository("just-a-name".into(), None))
            .unwrap_err();
        assert_eq!(err, "name_must_be_owner_slash_repo");
        let ok = pollster::block_on(k.create_repository("comfortcurators/curator".into(), None));
        assert!(ok.is_ok());
    }

    #[test]
    fn repository_cannot_be_added_twice() {
        let mut k = k(1_700_000_000);
        pollster::block_on(k.create_repository("comfortcurators/curator".into(), None)).unwrap();
        let err = pollster::block_on(k.create_repository("comfortcurators/curator".into(), None))
            .unwrap_err();
        assert_eq!(err, "repository_already_added");
    }

    #[test]
    fn sync_records_count_and_manifest_ref_not_before() {
        let mut k = k(1_700_000_000);
        let r = pollster::block_on(k.create_repository("comfortcurators/curator".into(), None))
            .unwrap();
        assert!(r.last_synced_at.is_none());
        let synced = pollster::block_on(k.mark_repository_synced(&r.id, 42, "repos/x/manifest.json".into()))
            .unwrap();
        assert_eq!(synced.last_sync_file_count, Some(42));
        assert!(synced.last_synced_at.is_some());
    }

    #[test]
    fn every_key_gets_its_own_workspace_id() {
        let mut k = k(1_700_000_000);
        let a = pollster::block_on(k.create_key("a".into(), None)).unwrap();
        let b = pollster::block_on(k.create_key("b".into(), None)).unwrap();
        assert!(!a.workspace_id.is_empty());
        assert!(!b.workspace_id.is_empty());
        assert_ne!(a.workspace_id, b.workspace_id);
    }

    #[test]
    fn checkpoints_are_scoped_to_their_own_token() {
        let mut k = k(1_700_000_000);
        let a = pollster::block_on(k.create_key("a".into(), None)).unwrap();
        let b = pollster::block_on(k.create_key("b".into(), None)).unwrap();
        pollster::block_on(k.create_checkpoint(&a.token, "before refactor".into())).unwrap();
        pollster::block_on(k.create_checkpoint(&a.token, "after refactor".into())).unwrap();
        assert_eq!(k.list_checkpoints(&a.token).len(), 2);
        assert!(k.list_checkpoints(&b.token).is_empty());
        let err = pollster::block_on(k.create_checkpoint("not-a-real-token", "x".into()))
            .unwrap_err();
        assert_eq!(err, "token_not_recognized");
    }

    #[test]
    fn whitepaper_is_none_until_written_and_clears_on_empty_text() {
        let mut k = k(1_700_000_000);
        assert_eq!(k.get_whitepaper(), None);
        pollster::block_on(k.set_whitepaper("We govern compute by cost.".into())).unwrap();
        assert_eq!(k.get_whitepaper().as_deref(), Some("We govern compute by cost."));
        pollster::block_on(k.set_whitepaper("   ".into())).unwrap();
        assert_eq!(k.get_whitepaper(), None);
    }

    #[test]
    fn whitepaper_never_crosses_owners() {
        // Same discipline as `token_carries_its_owner_...` above, pointed
        // at the other new per-account field: two owners, two kernels,
        // two independent stores -- one owner's whitepaper must not be
        // readable through the other's kernel.
        let mut mine = k(1_700_000_000);
        pollster::block_on(mine.set_whitepaper("Mine, not shared.".into())).unwrap();
        let mut theirs = Kernel::new(
            "org_other".into(),
            MemoryStateStore::new(),
            MemoryLedger::new(),
            MemoryArtifacts::new(),
            FrozenClock::new(1_700_000_000),
        );
        pollster::block_on(theirs.load()).unwrap();
        assert_eq!(theirs.get_whitepaper(), None);
        assert_eq!(mine.get_whitepaper().as_deref(), Some("Mine, not shared."));
    }

    #[test]
    fn key_with_validity_expires_and_stops_matching() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("short".into(), Some(60))).unwrap();
        assert_eq!(t.expires_unix, 1_700_000_060);
        assert_eq!(t.validity_seconds, Some(60));
        assert!(k.token_matches(&t.token));
        k.clock().set(1_700_000_060);
        assert!(!k.token_matches(&t.token), "expired key must stop matching");
        // It is still listed -- the dashboard shows expired keys with a
        // "expired" label rather than silently hiding them.
        assert!(k.list_keys().iter().any(|x| x.token == t.token));
    }

    #[test]
    fn key_without_validity_never_expires() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("forever".into(), None)).unwrap();
        assert_eq!(t.expires_unix, 0);
        assert_eq!(t.validity_seconds, None);
        k.clock().set(2_000_000_000);
        assert!(k.token_matches(&t.token));
    }

    #[test]
    fn reroll_kills_the_old_key_and_preserves_label_and_validity() {
        let mut k = k(1_700_000_000);
        let old = pollster::block_on(k.create_key("mine".into(), Some(3600))).unwrap();
        let new = pollster::block_on(k.reroll_key(&old.token)).unwrap();
        assert_ne!(old.token, new.token);
        assert_eq!(new.label, "mine");
        assert_eq!(new.validity_seconds, Some(3600));
        assert!(!new.workspace_id.is_empty());
        assert!(!k.token_matches(&old.token), "reroll must kill the old key");
        assert!(k.token_matches(&new.token));
    }

    #[test]
    fn delete_drops_checkpoints_but_keeps_knock_history() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("doomed".into(), None)).unwrap();
        pollster::block_on(k.create_checkpoint(&t.token, "before".into())).unwrap();
        pollster::block_on(k.create_knock(
            t.token.clone(),
            "Agent".into(),
            "r".into(),
            vec!["hostos.inventory".into()],
            vec![Permission::Read],
            Duration::SingleUse,
        ))
        .unwrap();
        assert_eq!(k.list_checkpoints(&t.token).len(), 1);

        pollster::block_on(k.delete_key(&t.token)).unwrap();
        assert!(!k.token_matches(&t.token));
        assert!(k.list_checkpoints(&t.token).is_empty());
        // The knock survives -- deleting a key does not erase what it did.
        assert_eq!(k.key_log(&t.token).len(), 1);
    }

    #[test]
    fn marker_only_checkpoint_has_no_snapshot() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("t".into(), None)).unwrap();
        let c = pollster::block_on(k.create_checkpoint(&t.token, "before".into())).unwrap();
        assert!(c.snapshot_ref.is_none());
        assert_eq!(c.file_count, 0);
    }

    #[test]
    fn snapshot_checkpoint_records_ref_and_count() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("t".into(), None)).unwrap();
        let c = pollster::block_on(k.create_checkpoint_with_snapshot(
            &t.token,
            "with content".into(),
            Some("checkpoints/manifests/org_owner/abc/chk_1.json".into()),
            17,
        ))
        .unwrap();
        assert_eq!(c.file_count, 17);
        assert_eq!(
            c.snapshot_ref.as_deref(),
            Some("checkpoints/manifests/org_owner/abc/chk_1.json")
        );
    }

    #[test]
    fn bad_snapshot_ref_is_rejected() {
        let mut k = k(1_700_000_000);
        let t = pollster::block_on(k.create_key("t".into(), None)).unwrap();
        let err = pollster::block_on(k.create_checkpoint_with_snapshot(
            &t.token,
            "x".into(),
            Some("../../etc/passwd".into()),
            0,
        ))
        .unwrap_err();
        assert_eq!(err, "bad_snapshot_ref");
    }

    #[test]
    fn get_checkpoint_scopes_to_its_token() {
        let mut k = k(1_700_000_000);
        let a = pollster::block_on(k.create_key("a".into(), None)).unwrap();
        let b = pollster::block_on(k.create_key("b".into(), None)).unwrap();
        let c = pollster::block_on(k.create_checkpoint(&a.token, "a's".into())).unwrap();
        assert!(k.get_checkpoint(&a.token, &c.id).unwrap().is_some());
        assert!(k.get_checkpoint(&b.token, &c.id).unwrap().is_none());
    }

    #[test]
    fn activity_is_capped_and_keeps_the_most_recent() {
        let mut k = k(1_700_000_000);
        // Drive past the cap. Calling `note` directly is what these
        // tests are for -- it is the exact function that owns the cap,
        // and using it skips the intent/approval/digest machinery that
        // would add nothing but runtime to a test about a Vec bound.
        for i in 0..(MAX_ACTIVITY_ENTRIES + 50) {
            k.note(
                &format!("test.{i}"),
                format!("event {i}"),
                None,
                None,
            );
        }
        let activity = k.activity().unwrap();
        assert_eq!(activity.len(), MAX_ACTIVITY_ENTRIES);
        // The most recent event is the last one -- `kind` on the last
        // entry is `test.<MAX+49>`.
        assert_eq!(
            activity.last().unwrap().kind,
            format!("test.{}", MAX_ACTIVITY_ENTRIES + 49)
        );
        // The oldest retained entry is NOT `test.0`; the first 50 were
        // dropped to make room.
        assert_eq!(activity.first().unwrap().kind, format!("test.{}", 50));
    }

    #[test]
    fn outcome_data_over_cap_is_truncated_with_a_marker() {
        let mut k = k(1_700_000_000);
        let huge = "x".repeat(MAX_OUTCOME_DATA_BYTES * 4);
        let outcome = Outcome {
            id: "out_1".into(),
            intent_id: "int_1".into(),
            execution_id: "exec_1".into(),
            grant_id: "grt_1".into(),
            resource: "hostos.inventory".into(),
            operation: Permission::Read,
            ok: true,
            data: Some(serde_json::json!({ "big": huge })),
            error: None,
            provider: "test".into(),
            mock: Some(true),
            at: "2026-09-18T00:00:00Z".into(),
        };
        pollster::block_on(k.record_outcome(outcome)).unwrap();
        let stored = k.outcomes_for_intent("int_1").unwrap();
        assert_eq!(stored.len(), 1);
        let data = stored[0].data.as_ref().unwrap();
        assert_eq!(
            data.get("truncated").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(
            data.get("original_bytes")
                .and_then(|v| v.as_u64())
                .unwrap()
                > MAX_OUTCOME_DATA_BYTES as u64
        );
        let preview = data.get("preview").and_then(|v| v.as_str()).unwrap();
        assert!(preview.len() <= MAX_OUTCOME_DATA_BYTES);
    }

    #[test]
    fn outcome_data_under_cap_is_untouched() {
        let mut k = k(1_700_000_000);
        let small = serde_json::json!({ "workers": 1, "buckets": 1 });
        let outcome = Outcome {
            id: "out_2".into(),
            intent_id: "int_2".into(),
            execution_id: "exec_2".into(),
            grant_id: "grt_2".into(),
            resource: "cloudflare.inventory".into(),
            operation: Permission::Read,
            ok: true,
            data: Some(small.clone()),
            error: None,
            provider: "test".into(),
            mock: Some(true),
            at: "2026-09-18T00:00:00Z".into(),
        };
        pollster::block_on(k.record_outcome(outcome)).unwrap();
        let stored = k.outcomes_for_intent("int_2").unwrap();
        assert_eq!(stored[0].data.as_ref().unwrap(), &small);
    }
}
