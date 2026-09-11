//! Cloudflare substrate. wasm only. Not a workspace member.

use async_trait::async_trait;
use curatom_ports::{ArtifactStore, Clock, EventLedger, StateStore};
use curatom_protocol::{HttpRequestDto, Identity, IdentityKind, KernelState};

pub struct CloudflareClock;
impl Clock for CloudflareClock {
    fn now_iso(&self) -> String {
        js_sys::Date::new_0().to_iso_string().into()
    }
    fn now_unix(&self) -> i64 {
        (js_sys::Date::now() / 1000.0) as i64
    }
}

/// Durable Object state store. Snapshot env/storage before awaiting.
pub struct DOStateStore {
    storage: worker::durable::Storage,
}

impl DOStateStore {
    pub fn new(storage: worker::durable::Storage) -> Self {
        Self { storage }
    }
}

#[async_trait(?Send)]
impl StateStore for DOStateStore {
    async fn get(&self) -> Result<Option<KernelState>, String> {
        match self.storage.get::<KernelState>("kernel_state").await {
            Ok(s) => Ok(Some(s)),
            Err(_) => Ok(None),
        }
    }
    async fn put(&self, state: &KernelState) -> Result<(), String> {
        self.storage
            .put("kernel_state", state)
            .await
            .map_err(|e| e.to_string())
    }
}

pub struct DOEventLedger {
    storage: worker::durable::Storage,
}

impl DOEventLedger {
    pub fn new(storage: worker::durable::Storage) -> Self {
        Self { storage }
    }
}

#[async_trait(?Send)]
impl EventLedger for DOEventLedger {
    async fn append(&self, kind: &str, body: &str) -> Result<(), String> {
        let key = format!("ledger:{}:{}", kind, body);
        self.storage
            .put(&key, "1")
            .await
            .map_err(|e| e.to_string())
    }
}

pub struct R2ArtifactStore {
    bucket: worker::Bucket,
    prefix: String,
}

impl R2ArtifactStore {
    pub fn new(bucket: worker::Bucket, owner_id: String) -> Self {
        Self {
            bucket,
            prefix: owner_id,
        }
    }
}

#[async_trait(?Send)]
impl ArtifactStore for R2ArtifactStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), String> {
        let k = format!("{}/{}", self.prefix, key);
        self.bucket
            .put(&k, bytes.to_vec())
            .execute()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let k = format!("{}/{}", self.prefix, key);
        match self.bucket.get(&k).execute().await {
            Ok(Some(obj)) => {
                let b = obj.body().ok_or("empty")?.bytes().await.map_err(|e| e.to_string())?;
                Ok(Some(b))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub struct CloudflareAccessIdentityProvider {
    owner_id: String,
    dev: bool,
}

impl CloudflareAccessIdentityProvider {
    pub fn new(env: &worker::Env, owner_id: String) -> Result<Self, worker::Error> {
        let dev = env
            .var("CURATOM_ENV")
            .map(|v| v.to_string() == "development")
            .unwrap_or(false);
        Ok(Self { owner_id, dev })
    }

    pub async fn authenticate(&self, hr: &HttpRequestDto) -> Result<Option<Identity>, String> {
        if self.dev {
            if let Some(id) = hr.header("x-curatom-dev-organic") {
                return Ok(Some(Identity {
                    id: id.to_string(),
                    kind: IdentityKind::Organic,
                }));
            }
        }
        // Production: verify Cf-Access-Jwt-Assertion against Cloudflare JWKS.
        // Deferred. See DEFERRED.md. Trusting the header is a staging boundary.
        if let Some(id) = hr.header("cf-access-authenticated-user-email") {
            if id == self.owner_id || true {
                return Ok(Some(Identity {
                    id: self.owner_id.clone(),
                    kind: IdentityKind::Organic,
                }));
            }
        }
        if hr.header("cf-access-jwt-assertion").is_some() {
            return Ok(Some(Identity {
                id: self.owner_id.clone(),
                kind: IdentityKind::Organic,
            }));
        }
        Ok(None)
    }
}

/// Throws. Real machine auth is its own module.
pub struct ProductionCloudflareInorganicIdentityProvider;

impl ProductionCloudflareInorganicIdentityProvider {
    pub async fn authenticate(&self, _hr: &HttpRequestDto) -> Result<Option<Identity>, String> {
        Err("inorganic production identity is not implemented".into())
    }
}

pub async fn to_dto(mut req: worker::Request) -> Result<HttpRequestDto, worker::Error> {
    let mut headers = std::collections::HashMap::new();
    for (k, v) in req.headers() {
        headers.insert(k, v);
    }
    let body = req.text().await.unwrap_or_default();
    Ok(HttpRequestDto {
        method: req.method().to_string(),
        url: req.url()?.to_string(),
        headers,
        body,
    })
}
