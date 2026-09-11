use serde::{Deserialize, Serialize};

/// The monkey's identity. Created once, on first claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerRecord {
    pub owner_id: String,
    pub display_name: String,
    #[serde(default, alias = "enrolled_at")]
    pub claimed_at: String,
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
