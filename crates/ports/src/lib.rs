//! Ports the kernel talks to. No Cloudflare imports.

use async_trait::async_trait;
use curatom_protocol::KernelState;

pub trait Clock: Send + Sync {
    fn now_iso(&self) -> String;
    fn now_unix(&self) -> i64;
}

/// Which top-level fields of `KernelState` changed since the last
/// successful persist. The kernel accumulates these as it mutates and
/// clears them once the store has written. A store that ignores the set
/// (like the memory store, which just keeps the whole struct) is still
/// correct -- the set is an optimization, not a correctness requirement.
///
/// A single `u8` per entity kind, not a `HashSet<KernelKind>`: this is
/// touched on every mutation and read once per persist, so it stays a
/// cheap copyable value the kernel can hold inline.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct DirtyKinds(u16);

impl DirtyKinds {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u16::MAX);

    pub const META: Self = Self(1 << 0);
    pub const TOKENS: Self = Self(1 << 1);
    pub const KNOCKS: Self = Self(1 << 2);
    pub const INTENTS: Self = Self(1 << 3);
    pub const APPROVALS: Self = Self(1 << 4);
    pub const GRANTS: Self = Self(1 << 5);
    pub const CONSUMED: Self = Self(1 << 6);
    pub const ISSUED: Self = Self(1 << 7);
    pub const OUTCOMES: Self = Self(1 << 8);
    pub const ACTIVITY: Self = Self(1 << 9);
    pub const FREEZES: Self = Self(1 << 10);
    pub const CONNECTORS: Self = Self(1 << 11);
    pub const REPOSITORIES: Self = Self(1 << 12);
    pub const CHECKPOINTS: Self = Self(1 << 13);
    pub const WHITEPAPER: Self = Self(1 << 14);

    pub fn new() -> Self {
        Self::NONE
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn bits(self) -> u16 {
        self.0
    }
}

// ?Send because Cloudflare Workers' JsFuture is !Send. See substrate-cloudflare.
#[async_trait(?Send)]
pub trait StateStore: Send + Sync {
    /// Read the full state, or `None` if this store has never held one.
    /// The kernel applies its own defaults on `None`; the store does not
    /// invent one.
    async fn load(&self) -> Result<Option<KernelState>, String>;

    /// Write only the fields marked in `dirty`. A store is free to write
    /// everything regardless (the memory store does), but the DO store
    /// uses this to touch one storage key per marked entity kind instead
    /// of rewriting the whole blob.
    async fn persist(&self, state: &KernelState, dirty: &DirtyKinds) -> Result<(), String>;
}

// ?Send because Cloudflare Workers' JsFuture is !Send. See substrate-cloudflare.
#[async_trait(?Send)]
pub trait EventLedger: Send + Sync {
    async fn append(&self, kind: &str, body: &str) -> Result<(), String>;
}

// ?Send because Cloudflare Workers' JsFuture is !Send. See substrate-cloudflare.
#[async_trait(?Send)]
pub trait ArtifactStore: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), String>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
}
