use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerRecord {
    pub owner_id: String,
    pub display_name: String,
    #[serde(default, alias = "enrolled_at")]
    pub claimed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganicMeView {
    pub id: String,
    pub display_name: String,
    pub mode: String,
    pub enrolled: bool,
    pub enrolled_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganicToken {
    pub token: String,
    pub owner_id: String,
    #[serde(default)]
    pub label: String,
    pub created_at: String,
    #[serde(default)]
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyLogEntry {
    pub kind: String,
    pub at: String,
    pub knock_id: Option<String>,
    pub name: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnockStatus {
    Pending,
    Approved,
    Refused,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knock {
    pub id: String,
    pub owner_id: String,
    pub token: String,
    pub name: String,
    pub reason: String,
    pub resources: Vec<String>,
    pub permissions: Vec<crate::Permission>,
    pub duration: crate::Duration,
    pub created_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub expires_unix: i64,
    pub status: KnockStatus,
    pub decided_at: Option<String>,
    pub request_digest: String,
}

pub const KNOCK_TTL_SECS: i64 = 88;
