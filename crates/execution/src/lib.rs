//! Coordinator trait only. Real execution lives in Elixir.

use async_trait::async_trait;
use curatom_protocol::Outcome;

#[async_trait]
pub trait ExecutionCoordinator: Send + Sync {
    async fn dispatch(&self, job_json: serde_json::Value) -> Result<(), String>;
    async fn record_remote_outcome(&self, outcome: Outcome) -> Result<(), String>;
}
