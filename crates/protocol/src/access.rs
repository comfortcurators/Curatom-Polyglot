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
    /// This key's own identity, separate from the token itself -- minted
    /// once at `create_key` and never rotated (a revoke+recreate mints a
    /// new one, on purpose: a rotated key is a new key, not the same
    /// workspace continuing). `serde(default)` so a key created before
    /// this field existed still loads; it reads as empty, not missing.
    #[serde(default)]
    pub workspace_id: String,
    /// How long this key was minted to live, or `None` for "no expiry."
    /// Stored explicitly (not derived) so a reroll can mint the new key
    /// with the same lifetime as the key it replaces -- deriving it from
    /// `expires_unix - created_at` would slowly drift on every reroll.
    #[serde(default)]
    pub validity_seconds: Option<u64>,
    /// `0` = never expires. Otherwise a unix-seconds deadline compared as
    /// an integer by `Kernel::token_matches`, the same discipline as
    /// `CapabilitySecret::expires_unix` -- never parse a date to decide
    /// whether a credential is live.
    #[serde(default)]
    pub expires_unix: i64,
}

/// A named point in a key's own history -- "what state was I working
/// from."
///
/// Two kinds, distinguished by `snapshot_ref`:
///   * marker-only -- `snapshot_ref: None`. Existed before Phase 4 and
///     still legal: the operator is writing a note-to-self about a
///     point in time without capturing anything behind it.
///   * snapshot -- `snapshot_ref: Some("checkpoints/manifests/...")`.
///     An R2 manifest listing every file under `/workspace` at the
///     moment the checkpoint was created, plus content-addressed
///     blobs for the file contents. Restore from a snapshot is
///     `Valhalla`'s `/restore` endpoint reading that manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub note: String,
    pub created_at: String,
    /// R2 key of the snapshot manifest, or `None` for a marker-only
    /// checkpoint. `serde(default)` so a checkpoint created before
    /// this field existed still loads; it reads as marker-only, which
    /// is what it was.
    #[serde(default)]
    pub snapshot_ref: Option<String>,
    /// Number of files captured in the snapshot. `0` for marker-only.
    /// Reported to the dashboard so the operator can see whether a
    /// checkpoint has content behind it before trying to restore one.
    #[serde(default)]
    pub file_count: u64,
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

/// One auth header a connector sends on every call -- `name` alone is
/// safe to hand back to the dashboard after creation; `value` is not,
/// the same discipline as a minted password or a capability token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorHeader {
    pub name: String,
    pub value: String,
}

/// A user-defined MCP (or any HTTP-reachable tool) endpoint this owner
/// has told Curatom about. Curatom never ships one pre-wired -- see the
/// module doc on `Kernel::create_connector` for why that line matters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub headers: Vec<ConnectorHeader>,
    pub created_at: String,
}

/// A repository this owner has pointed Curatom at, for `Kernel::sync_repository`
/// (executed in the Worker, since fetching GitHub is I/O this crate never
/// does) to pull a structured, LLM-readable snapshot of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: String,
    pub owner_id: String,
    /// `owner/repo`, GitHub's own shorthand.
    pub name: String,
    /// A PAT for a private repo. Never serialized back to the dashboard
    /// after creation -- same discipline as a connector header's value.
    #[serde(default)]
    pub github_token: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub last_synced_at: Option<String>,
    #[serde(default)]
    pub last_sync_file_count: Option<usize>,
    /// R2 key of the most recent manifest `sync_repository` wrote, if any.
    #[serde(default)]
    pub manifest_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Freeze {
    pub id: String,
    pub scope: String,
    pub reason: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub released_at: Option<String>,
    pub release_reason: Option<String>,
    pub release_parity_ok: Option<bool>,
}
