use serde::{Deserialize, Serialize};

/// The monkey's identity, created exactly once, at enrollment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerRecord {
    pub owner_id: String,
    pub display_name: String,
    pub sign1_digest: String,
    pub sign1_ref: String,
    pub sign2_digest: String,
    pub sign2_ref: String,
    pub enrolled_at: String,
}

/// In-flight enrollment, held in DO memory only. Never persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollmentSession {
    pub session_id: String,
    pub owner_id: String,
    pub sign1_digest: Option<String>,
    pub sign1_ref: Option<String>,
    pub sign2_digest: Option<String>,
    pub sign2_ref: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

impl EnrollmentSession {
    pub fn is_complete(&self) -> bool {
        self.sign1_digest.is_some() && self.sign2_digest.is_some()
    }
    pub fn is_expired(&self, now_iso: &str) -> bool {
        now_iso > self.expires_at.as_str()
    }
}

/// What the monkey sees at /organic/me.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganicMeView {
    pub id: String,
    pub display_name: String,
    pub mode: String,
    pub enrolled: bool,
    pub enrolled_at: Option<String>,
}
