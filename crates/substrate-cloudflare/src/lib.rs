//! Cloudflare substrate. wasm only. Not a workspace member.

use async_trait::async_trait;
use curatom_ports::{ArtifactStore, Clock, EventLedger, StateStore};
use curatom_protocol::{HttpRequestDto, Identity, IdentityKind, KernelState};
use std::sync::Mutex;

pub struct CloudflareClock;
impl Clock for CloudflareClock {
    fn now_iso(&self) -> String {
        js_sys::Date::new_0().to_iso_string().into()
    }
    fn now_unix(&self) -> i64 {
        (js_sys::Date::now() / 1000.0) as i64
    }
}

/// Durable Object state store.
///
/// worker 0.8 `Storage::put` takes `&self`. The Mutex is still the right
/// holder: two stores share one Storage from `state.storage()`.
pub struct DOStateStore {
    storage: Mutex<worker::durable::Storage>,
}

impl DOStateStore {
    pub fn new(storage: worker::durable::Storage) -> Self {
        Self {
            storage: Mutex::new(storage),
        }
    }
}

#[async_trait(?Send)]
impl StateStore for DOStateStore {
    async fn get(&self) -> Result<Option<KernelState>, String> {
        let storage = self.storage.lock().map_err(|e| e.to_string())?;
        match storage.get::<KernelState>("kernel_state").await {
            Ok(s) => Ok(s),
            Err(_) => Ok(None),
        }
    }
    async fn put(&self, state: &KernelState) -> Result<(), String> {
        let storage = self.storage.lock().map_err(|e| e.to_string())?;
        storage
            .put("kernel_state", state)
            .await
            .map_err(|e| e.to_string())
    }
}

pub struct DOEventLedger {
    storage: Mutex<worker::durable::Storage>,
}

impl DOEventLedger {
    pub fn new(storage: worker::durable::Storage) -> Self {
        Self {
            storage: Mutex::new(storage),
        }
    }
}

#[async_trait(?Send)]
impl EventLedger for DOEventLedger {
    async fn append(&self, kind: &str, body: &str) -> Result<(), String> {
        let key = format!("ledger:{}:{}", kind, body);
        let storage = self.storage.lock().map_err(|e| e.to_string())?;
        storage.put(&key, "1").await.map_err(|e| e.to_string())
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
    admin_token: Option<String>,
}

/// Byte-length-equal-input constant-time comparison. Deliberately not
/// `==`: a naive comparison short-circuits on the first mismatched
/// byte, so its timing leaks how many leading bytes of a guess were
/// correct. RAJ_TOKEN is a real bearer credential, not a debug header
/// -- same discipline this org already applies to CURA_ORIGIN_SECRET
/// and reek's auth guard.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn bearer_token(hr: &HttpRequestDto) -> Option<&str> {
    hr.header("authorization")?.strip_prefix("Bearer ")
}

/// `?token=` query-param fallback, so RAJ_TOKEN works by pasting a URL
/// into a plain browser tab -- Safari has no way to set an Authorization
/// header, and that's the actual entry path this exists for. No percent-
/// decoding: RAJ_TOKEN is generated, not user-typed, so it never needs
/// characters a URL requires encoding.
fn query_token(hr: &HttpRequestDto) -> Option<&str> {
    let (_, query) = hr.url.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == "token").then_some(v)
    })
}

impl CloudflareAccessIdentityProvider {
    pub fn new(env: &worker::Env, owner_id: String) -> Result<Self, worker::Error> {
        let dev = env
            .var("CURATOM_ENV")
            .map(|v| v.to_string() == "development")
            .unwrap_or(false);
        // Absent until the founder sets it (`wrangler secret put RAJ_TOKEN`).
        // Not present is not an error -- this path is simply unavailable
        // until the secret exists, same as any other optional Worker secret.
        let admin_token = env.secret("RAJ_TOKEN").ok().map(|s| s.to_string());
        Ok(Self { owner_id, dev, admin_token })
    }

    pub async fn authenticate(&self, hr: &HttpRequestDto) -> Result<Option<Identity>, String> {
        // Admin break-glass: a bearer token matching the RAJ_TOKEN Worker
        // secret authenticates as owner, independent of Cloudflare Access
        // entirely -- the whole point is a path in that doesn't depend on
        // the Access/JWKS verification this file already defers (see
        // DEFERRED.md). Checked first because it's the strongest, most
        // explicit credential of the three checked here.
        if let Some(expected) = &self.admin_token {
            let presented = bearer_token(hr).or_else(|| query_token(hr));
            if let Some(presented) = presented {
                if constant_time_eq(presented.as_bytes(), expected.as_bytes()) {
                    return Ok(Some(Identity {
                        id: self.owner_id.clone(),
                        kind: IdentityKind::Organic,
                    }));
                }
            }
        }

        if self.dev {
            if let Some(id) = hr.header("x-curatom-dev-organic") {
                return Ok(Some(Identity {
                    id: id.to_string(),
                    kind: IdentityKind::Organic,
                }));
            }
        }
        // The two Cf-Access-* header checks that used to live here are
        // removed, not merely disabled. They never verified anything --
        // `cf-access-authenticated-user-email` was trusted unconditionally
        // (`|| true`, dead code masquerading as a real check) and
        // `cf-access-jwt-assertion` was trusted for mere presence, not a
        // verified JWT (deferred in DEFERRED.md, real verification needs
        // Cloudflare's JWKS). That was only ever safe because Access's own
        // edge gate stopped unauthenticated traffic before the Worker ran
        // -- true on the custom domain, false the moment this Worker is
        // reachable anywhere Access doesn't cover (workers_dev, a direct
        // service binding, ...), where either header is just a value an
        // attacker sets themselves. RAJ_TOKEN and the CURATOM_ENV=development
        // header above are the only two paths now: both are real secrets,
        // neither is spoofable by setting a header Cloudflare happens to
        // pass through unmodified.
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

