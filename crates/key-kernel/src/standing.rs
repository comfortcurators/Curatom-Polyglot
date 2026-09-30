//! Operator-issued standing grants and audit projection.
//!
//! Standing skips the 88s knock for eligible resources only.
//! Sensitive scopes (`valhalla.*`, `hostos.*`) stay on the knock path.

use curatom_ports::DirtyKinds;
use curatom_protocol::*;
use curatom_crypto::random_id;
use curatom_resource_registry::standing_eligible;

use super::Kernel;
use curatom_ports::{ArtifactStore, Clock, EventLedger, StateStore};

impl<S, L, A, C> Kernel<S, L, A, C>
where
    S: StateStore,
    L: EventLedger,
    A: ArtifactStore,
    C: Clock,
{
    // ---- standing grants (operator-issued; skip 88s knock for eligible scopes) ----

    /// Issue a standing grant. Operator-only path — the Worker must have
    /// already authenticated the owner. Resources must all be
    /// `standing_eligible`; sensitive scopes (`valhalla.*`, `hostos.*`)
    /// are refused here so they stay on the knock path.
    pub async fn issue_standing_grant(
        &mut self,
        label: String,
        resources: Vec<String>,
        permissions: Vec<Permission>,
        token: Option<String>,
    ) -> Result<StandingGrant, String> {
        let label = label.trim().to_string();
        if label.is_empty() {
            return Err("label_required".into());
        }
        if resources.is_empty() || permissions.is_empty() {
            return Err("empty_scope".into());
        }
        for r in &resources {
            if !standing_eligible(r) {
                return Err(format!("resource_not_standing_eligible:{r}"));
            }
        }
        if let Some(ref tok) = token {
            if !self.token_matches(tok) {
                return Err("token_not_recognized".into());
            }
        }
        let mut resources = resources;
        resources.sort();
        resources.dedup();
        let mut permissions = permissions;
        permissions.sort();
        permissions.dedup();
        let g = StandingGrant {
            id: random_id("stg"),
            owner_id: self.owner_id.clone(),
            token,
            label: label.clone(),
            resources,
            permissions,
            created_at: self.clock.now_iso(),
            revoked_at: None,
            use_count: 0,
            last_used_at: None,
        };
        {
            let st = self.st_mut()?;
            st.standing_grants.insert(g.id.clone(), g.clone());
        }
        self.mark(DirtyKinds::STANDING.with(DirtyKinds::ACTIVITY));
        self.note(
            "standing.issued",
            format!("{label} {}", g.id),
            None,
            None,
        );
        self.persist().await?;
        Ok(g)
    }

    pub fn list_standing_grants(&self) -> Vec<StandingGrant> {
        self.st()
            .map(|s| {
                let mut v: Vec<_> = s.standing_grants.values().cloned().collect();
                v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                v
            })
            .unwrap_or_default()
    }

    pub async fn revoke_standing_grant(&mut self, id: &str) -> Result<StandingGrant, String> {
        let now = self.clock.now_iso();
        let g = {
            let st = self.st_mut()?;
            let g = st
                .standing_grants
                .get_mut(id)
                .ok_or_else(|| "unknown_standing_grant".to_string())?;
            if g.revoked_at.is_some() {
                return Err("already_revoked".into());
            }
            g.revoked_at = Some(now);
            g.clone()
        };
        self.mark(DirtyKinds::STANDING.with(DirtyKinds::ACTIVITY));
        self.note("standing.revoked", g.label.clone(), None, None);
        self.persist().await?;
        Ok(g)
    }

    /// Find an active standing grant that covers every requested resource
    /// and permission for this token. Returns the first match.
    pub fn find_standing(
        &self,
        token: &str,
        resources: &[String],
        permissions: &[Permission],
    ) -> Option<StandingGrant> {
        if !self.token_matches(token) {
            return None;
        }
        if resources.is_empty() || permissions.is_empty() {
            return None;
        }
        if !resources.iter().all(|r| standing_eligible(r)) {
            return None;
        }
        let st = self.st().ok()?;
        st.standing_grants.values().find(|g| {
            if g.revoked_at.is_some() {
                return false;
            }
            if let Some(ref bound) = g.token {
                if !bound.is_empty() && bound != token {
                    return false;
                }
            }
            resources.iter().all(|r| g.resources.iter().any(|x| x == r))
                && permissions.iter().all(|p| g.permissions.iter().any(|x| x == p))
        }).cloned()
    }

    /// Record that a standing grant was used to skip a knock. Bumps
    /// use_count and writes `standing.used` to the activity feed.
    pub async fn record_standing_use(&mut self, id: &str, knock_id: &str) -> Result<(), String> {
        let now = self.clock.now_iso();
        let label = {
            let st = self.st_mut()?;
            let g = st
                .standing_grants
                .get_mut(id)
                .ok_or_else(|| "unknown_standing_grant".to_string())?;
            g.use_count = g.use_count.saturating_add(1);
            g.last_used_at = Some(now);
            g.label.clone()
        };
        self.mark(DirtyKinds::STANDING.with(DirtyKinds::ACTIVITY));
        self.note(
            "standing.used",
            format!("{label} knock={knock_id}"),
            Some(knock_id.to_string()),
            None,
        );
        self.persist().await
    }

    /// Auto-approve a pending knock under a matching standing grant.
    /// Sets duration to Standing so the issued capability is reusable.
    pub async fn apply_standing_to_knock(
        &mut self,
        knock_id: &str,
        standing_id: &str,
    ) -> Result<Knock, String> {
        let now_iso = self.clock.now_iso();
        {
            let st = self.st_mut()?;
            let k = st
                .knocks
                .get_mut(knock_id)
                .ok_or_else(|| "unknown_knock".to_string())?;
            if k.status != KnockStatus::Pending {
                return Err("knock_not_pending".into());
            }
            k.status = KnockStatus::Approved;
            k.decided_at = Some(now_iso);
            k.duration = Duration::Standing;
        }
        self.mark(DirtyKinds::KNOCKS.with(DirtyKinds::ACTIVITY));
        self.note("knock.approved", format!("standing:{standing_id}"), Some(knock_id.to_string()), None);
        self.record_standing_use(standing_id, knock_id).await?;
        self.get_knock(knock_id)?.ok_or_else(|| "unknown_knock".into())
    }

    /// Operator-initiated Valhalla bootstrap on key create. No knock —
    /// the operator just minted the key and wants its workspace ready.
    /// Still fails closed for unrecognized tokens.
    pub fn authorize_valhalla_bootstrap(&self, token: &str) -> Result<(), String> {
        if !self.token_matches(token) {
            return Err("token_not_recognized".into());
        }
        Ok(())
    }

    /// Unified audit projection: activity + knocks + standing grants.
    /// Organic-safe (no raw tokens / grant secrets).
    pub fn audit_projection(&self, limit: usize) -> Result<serde_json::Value, String> {
        let st = self.st()?;
        let limit = limit.clamp(1, 500);
        let mut events: Vec<serde_json::Value> = Vec::new();
        for a in st.activity.iter().rev().take(limit) {
            events.push(serde_json::json!({
                "source": "activity",
                "kind": a.kind,
                "summary": a.summary,
                "at": a.at,
                "intent_id": a.intent_id,
                "approval_id": a.approval_id,
            }));
        }
        let mut knocks: Vec<_> = st.knocks.values().cloned().collect();
        knocks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let knock_views: Vec<_> = knocks.into_iter().take(limit.min(100)).map(|k| {
            serde_json::json!({
                "id": k.id,
                "name": k.name,
                "reason": k.reason,
                "resources": k.resources,
                "status": format!("{:?}", k.status).to_lowercase(),
                "duration": k.duration.label(),
                "created_at": k.created_at,
                "decided_at": k.decided_at,
            })
        }).collect();
        let standing: Vec<_> = self.list_standing_grants().into_iter().map(|g| {
            serde_json::json!({
                "id": g.id,
                "label": g.label,
                "resources": g.resources,
                "permissions": g.permissions.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
                "token_bound": g.token.as_ref().map(|t| !t.is_empty()).unwrap_or(false),
                "created_at": g.created_at,
                "revoked_at": g.revoked_at,
                "use_count": g.use_count,
                "last_used_at": g.last_used_at,
            })
        }).collect();
        Ok(serde_json::json!({
            "events": events,
            "knocks": knock_views,
            "standing_grants": standing,
            "generated_at": self.clock.now_iso(),
        }))
    }
}
