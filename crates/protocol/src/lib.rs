//! Shared types. No I/O, no Cloudflare, no time.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Read,
    Write,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::Read => "read",
            Permission::Write => "write",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "write" => Permission::Write,
            _ => Permission::Read,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Duration {
    SingleUse,
    Ttl { seconds: u64 },
}

impl Duration {
    pub fn label(&self) -> String {
        match self {
            Duration::SingleUse => "single-use".into(),
            Duration::Ttl { seconds } => format!("ttl:{seconds}s"),
        }
    }
}

/// Live capability. Never crosses Contract 1. Never given to Elixir.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilitySecret {
    pub token: String,
    pub grant_id: String,
    pub requester_id: String,
    pub resources: Vec<String>,
    pub permissions: Vec<Permission>,
    pub request_digest: String,
    /// Set at issue_grant from the approval's intent. Not a digest.
    pub intent_id: String,
    /// For humans / logs. None on single-use.
    pub expires_at: Option<String>,
    /// For the kernel. 0 = single-use. Compare integers. Do not parse dates.
    pub expires_unix: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub text: String,
    pub requester_id: String,
    pub resources: Vec<String>,
    pub permissions: Vec<Permission>,
    pub duration: Duration,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Refuse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approval {
    pub id: String,
    pub intent_id: String,
    pub requester: String,
    pub reason: String,
    pub resources: Vec<String>,
    pub permissions: Vec<Permission>,
    pub duration: Duration,
    pub digest: String,
    pub status: ApprovalStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssuedGrant {
    pub grant_id: String,
    pub approval_id: String,
    pub handle: GrantHandle,
}

/// The secret lives here. Organic views must not serialize this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrantHandle {
    pub secret: CapabilitySecret,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub id: String,
    pub intent_id: String,
    pub execution_id: String,
    pub grant_id: String,
    pub resource: String,
    pub operation: Permission,
    pub ok: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub provider: String,
    pub mock: Option<bool>,
    pub at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    pub kind: String,
    pub summary: String,
    pub at: String,
    pub intent_id: Option<String>,
    pub approval_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KernelState {
    pub owner_id: String,
    pub intents: HashMap<String, Intent>,
    pub approvals: HashMap<String, Approval>,
    pub grants: HashMap<String, CapabilitySecret>,
    /// Spent (grant_id, resource, permission-as-str) triples.
    pub consumed: HashSet<String>,
    /// Approval ids that have already minted a grant. One approval, one grant.
    /// `serde(default)` so state written before this field still loads.
    #[serde(default)]
    pub issued: HashSet<String>,
    pub outcomes: Vec<Outcome>,
    pub activity: Vec<Activity>,
}

impl KernelState {
    pub fn consume_key(grant_id: &str, resource: &str, op: Permission) -> String {
        format!("{grant_id}|{resource}|{}", op.as_str())
    }
}

/// Transport DTO used by both routers and the Durable Object.
#[derive(Clone, Debug, Default)]
pub struct HttpRequestDto {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpRequestDto {
    pub fn header(&self, name: &str) -> Option<&str> {
        let needle = name.to_ascii_lowercase();
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(&needle)).map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct Identity {
    pub id: String,
    pub kind: IdentityKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityKind {
    Organic,
    Inorganic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateIntentRequest {
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InorganicRequest {
    pub requester_id: String,
    pub reason: String,
    pub resources: Vec<String>,
    pub permissions: Vec<Permission>,
    pub duration: Duration,
}

/// Organic approval card. No tokens, no grants, no Cloudflare.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalView {
    pub id: String,
    pub requester: String,
    pub reason: String,
    pub resources: Vec<String>,
    pub permissions: Vec<String>,
    pub duration: String,
}

/// Organic activity line.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityView {
    pub kind: String,
    pub summary: String,
}

pub fn assert_organic_safe(value: &serde_json::Value) -> Result<(), String> {
    let dumped = value.to_string();
    for needle in ["\"token\"", "grt_", "cap_", "OAuth", "mcp", "CURATOM_ARTIFACTS"] {
        if dumped.contains(needle) {
            return Err(format!("organic leak: {needle}"));
        }
    }
    Ok(())
}
