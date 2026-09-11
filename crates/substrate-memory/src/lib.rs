//! In-memory substrate. This is what `cargo test --workspace` runs against.

use async_trait::async_trait;
use curatom_ports::{ArtifactStore, Clock, EventLedger, StateStore};
use curatom_protocol::KernelState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI64, Ordering};

#[derive(Clone, Default)]
pub struct MemoryStateStore {
    inner: Arc<Mutex<Option<KernelState>>>,
}

impl MemoryStateStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait(?Send)]
impl StateStore for MemoryStateStore {
    async fn get(&self) -> Result<Option<KernelState>, String> {
        Ok(self.inner.lock().unwrap().clone())
    }
    async fn put(&self, state: &KernelState) -> Result<(), String> {
        *self.inner.lock().unwrap() = Some(state.clone());
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct MemoryLedger {
    inner: Arc<Mutex<Vec<(String, String)>>>,
}

impl MemoryLedger {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait(?Send)]
impl EventLedger for MemoryLedger {
    async fn append(&self, kind: &str, body: &str) -> Result<(), String> {
        self.inner.lock().unwrap().push((kind.into(), body.into()));
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct MemoryArtifacts {
    inner: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MemoryArtifacts {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait(?Send)]
impl ArtifactStore for MemoryArtifacts {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), String> {
        self.inner.lock().unwrap().insert(key.into(), bytes.to_vec());
        Ok(())
    }
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.inner.lock().unwrap().get(key).cloned())
    }
}

pub struct FrozenClock {
    pub unix: AtomicI64,
    pub iso: Mutex<String>,
}

impl FrozenClock {
    pub fn new(unix: i64) -> Self {
        Self {
            unix: AtomicI64::new(unix),
            iso: Mutex::new(unix.to_string()),
        }
    }

    pub fn set(&self, unix: i64) {
        self.unix.store(unix, Ordering::SeqCst);
        *self.iso.lock().unwrap() = unix.to_string();
    }
}

impl Clock for FrozenClock {
    fn now_iso(&self) -> String {
        self.iso.lock().unwrap().clone()
    }
    fn now_unix(&self) -> i64 {
        self.unix.load(Ordering::SeqCst)
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_iso(&self) -> String {
        #[cfg(not(target_arch = "wasm32"))]
        {
            curatom_crypto_now()
        }
        #[cfg(target_arch = "wasm32")]
        {
            String::new()
        }
    }
    fn now_unix(&self) -> i64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
        }
        #[cfg(target_arch = "wasm32")]
        {
            0
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn curatom_crypto_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}
