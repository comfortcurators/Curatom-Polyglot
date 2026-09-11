//! Ports the kernel talks to. No Cloudflare imports.

use async_trait::async_trait;
use curatom_protocol::KernelState;

pub trait Clock: Send + Sync {
    fn now_iso(&self) -> String;
    fn now_unix(&self) -> i64;
}

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get(&self) -> Result<Option<KernelState>, String>;
    async fn put(&self, state: &KernelState) -> Result<(), String>;
}

#[async_trait]
pub trait EventLedger: Send + Sync {
    async fn append(&self, kind: &str, body: &str) -> Result<(), String>;
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), String>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
}
