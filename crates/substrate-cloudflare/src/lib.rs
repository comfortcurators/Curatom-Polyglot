//! Cloudflare substrate. wasm only. Not a workspace member.

use async_trait::async_trait;
use curatom_ports::{ArtifactStore, Clock, DirtyKinds, EventLedger, StateStore};
use curatom_protocol::{
    Activity, Approval, Checkpoint, Connector, Freeze, HttpRequestDto, Identity, IdentityKind,
    Intent, KernelState, Knock, OrganicToken, Outcome, Repository,
};
use curatom_protocol::CapabilitySecret;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
/// Storage key names, one per entity kind. Prefixed so a stray key
/// written by something else in this DO (the ledger writes `ledger:*`
/// keys, for instance) can never collide with kernel state.
const KEY_LEGACY: &str = "kernel_state";
const KEY_META: &str = "kernel:meta";
const KEY_TOKENS: &str = "kernel:tokens";
const KEY_KNOCKS: &str = "kernel:knocks";
const KEY_INTENTS: &str = "kernel:intents";
const KEY_APPROVALS: &str = "kernel:approvals";
const KEY_GRANTS: &str = "kernel:grants";
const KEY_CONSUMED: &str = "kernel:consumed";
const KEY_ISSUED: &str = "kernel:issued";
const KEY_OUTCOMES: &str = "kernel:outcomes";
const KEY_ACTIVITY: &str = "kernel:activity";
const KEY_FREEZES: &str = "kernel:freezes";
const KEY_CONNECTORS: &str = "kernel:connectors";
const KEY_REPOSITORIES: &str = "kernel:repositories";
const KEY_CHECKPOINTS: &str = "kernel:checkpoints";
const KEY_WHITEPAPER: &str = "kernel:whitepaper";

/// The two fields of `KernelState` that are not a collection, stored
/// together because they are both small and both change together on
/// enrollment.
#[derive(Serialize, Deserialize, Clone, Default)]
struct MetaBlob {
    owner_id: String,
    #[serde(default)]
    owner: Option<curatom_protocol::OwnerRecord>,
}

/// `whitepaper` is the one `KernelState` field that is a bare
/// `Option<String>` with nothing else beside it. Found the hard way,
/// against real production data on 19 Sep 2026: `serde_wasm_bindgen`
/// serializes a top-level `Option::None` to JS `undefined`, not `null`,
/// and Cloudflare's real Durable Object storage throws
/// `TypeError: put() called with undefined value` on that -- while
/// Miniflare's local emulation (used to verify this migration before
/// deploy) tolerated it silently. `owner: Option<OwnerRecord>` above
/// never hit this because it is a *field inside* `MetaBlob`, not the
/// top-level value handed to `storage.put`; nested `None` serializes to
/// `null` as expected. This wrapper gives `whitepaper` the same shape,
/// so the value `storage.put` ever sees is always an object, never a
/// bare `Option`.
#[derive(Serialize, Deserialize, Clone, Default)]
struct WhitepaperBlob {
    #[serde(default)]
    text: Option<String>,
}

/// `storage.put` wrapped to name the key on the specific failure mode
/// that took production down on 19 Sep 2026: a value that serializes to
/// JS `undefined` rather than `null`. `worker::durable::Storage::put`
/// calls `serde_wasm_bindgen::to_value` internally and hands the result
/// straight to the underlying JS `put`; a top-level `Option::None`
/// (or anything else that happens to serialize the same way) throws
/// `TypeError: put() called with undefined value` -- an error that
/// names neither the key nor the caller. Checking here first turns
/// that into `"refusing to write undefined to <key>"`, so the next
/// occurrence of this bug class is diagnosable from the error message
/// alone rather than from a `wrangler tail` session against production.
/// `WhitepaperBlob` already makes this specific case impossible; this
/// is the backstop for whichever field is next.
async fn guarded_put<T: Serialize>(
    storage: &worker::durable::Storage,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let js = serde_wasm_bindgen::to_value(value).map_err(|e| e.to_string())?;
    if js.is_undefined() {
        return Err(format!("refusing to write undefined to {key}"));
    }
    storage.put(key, value).await.map_err(|e| e.to_string())
}

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

/// Per-entity-key DO storage with a one-way migration from the old
/// single-blob format.
///
/// Layout after migration, all under the same DO instance:
///
///   kernel:meta         { owner_id, owner }        small, written every persist
///   kernel:tokens       HashMap<token, OrganicToken>
///   kernel:knocks       HashMap<id, Knock>
///   kernel:intents      HashMap<id, Intent>
///   kernel:approvals    HashMap<id, Approval>
///   kernel:grants       HashMap<grant_id, CapabilitySecret>
///   kernel:consumed     HashSet<consume_key>
///   kernel:issued       HashSet<approval_id>
///   kernel:outcomes     Vec<Outcome>
///   kernel:activity     Vec<Activity>
///   kernel:freezes      HashMap<id, Freeze>
///   kernel:connectors   HashMap<id, Connector>
///   kernel:repositories HashMap<id, Repository>
///   kernel:checkpoints  HashMap<token, Vec<Checkpoint>>
///   kernel:whitepaper   Option<String>
///
/// A mutation that touches one entity kind writes exactly one storage
/// key. The whole point of 5b: creating a key no longer rewrites every
/// knock, and recording a knock no longer rewrites every token.
///
/// `kernel:meta` is written unconditionally on every persist. It is
/// small (two short strings plus an optional OwnerRecord) and its
/// presence is what `load` uses to distinguish "new format" from
/// "fresh DO with no format yet". If it were only written when META
/// was dirty, a DO whose first mutation was, say, `submit_inorganic`
/// (INTENTS, APPROVALS, ACTIVITY -- no META) would write those three
/// keys and then be indistinguishable from a fresh DO on the next
/// load, losing them silently. Writing meta every time is the honest
/// fix for that hole.
///
/// # Rollback note for this migration
///
/// The migration is one-way: it deletes the legacy `kernel_state` blob
/// after writing every entity key and `kernel:meta`. `git revert` of this
/// commit therefore is NOT a rollback in production. The reverted 5a code
/// reads only `kernel_state`; a migrated DO has no such key, so `load()`
/// returns `None`, the kernel falls back to `KernelState::default()`, and
/// every token, knock, connector, repository, checkpoint, and activity
/// entry the owner had is gone from the kernel's perspective -- while
/// still physically present under `kernel:*` keys that 5a code never
/// looks at.
///
/// If this turns out to be wrong after real migrations have run, the
/// correct response is a forward-fix commit that reads and repairs the
/// `kernel:*` keys, not a revert. A true code-level rollback would
/// require keeping this `load()` path specifically (which understands
/// both formats) while reverting only the `persist()` side -- more
/// surgical than a `git revert`, and worth doing deliberately rather
/// than in an incident.
///
/// Verified by a local `wrangler dev` run (Miniflare's SQLite-backed DO
/// storage, not production): the migration path, idempotency, and
/// dirty-set write isolation across all fourteen entity kinds all held,
/// byte-for-byte against a legacy-format blob seeded through the real
/// HTTP API. Not verified: that production Durable Object storage
/// behaves identically to Miniflare's on the exact `get`/`put`/`delete`
/// calls this migration makes. Both go through the same `worker-rs`
/// signatures and the same serde-json serialization, so this is very
/// likely true -- but it was not checked against a real production DO,
/// only inferred from the shared implementation surface.
#[async_trait(?Send)]
impl StateStore for DOStateStore {
    async fn load(&self) -> Result<Option<KernelState>, String> {
        let storage = self.storage.lock().map_err(|e| e.to_string())?;

        // 1. New format exists. `kernel:meta` is the commit marker: its
        //    presence means the migration (or a fresh 5b-first boot)
        //    completed, and every other key is either present or was
        //    never written. Missing keys default to empty.
        if let Ok(Some(meta)) = storage.get::<MetaBlob>(KEY_META).await {
            let mut state = KernelState {
                owner_id: meta.owner_id,
                owner: meta.owner,
                ..KernelState::default()
            };

            if let Ok(Some(v)) = storage.get::<HashMap<String, OrganicToken>>(KEY_TOKENS).await {
                state.tokens = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, Knock>>(KEY_KNOCKS).await {
                state.knocks = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, Intent>>(KEY_INTENTS).await {
                state.intents = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, Approval>>(KEY_APPROVALS).await {
                state.approvals = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, CapabilitySecret>>(KEY_GRANTS).await {
                state.grants = v;
            }
            if let Ok(Some(v)) = storage.get::<HashSet<String>>(KEY_CONSUMED).await {
                state.consumed = v;
            }
            if let Ok(Some(v)) = storage.get::<HashSet<String>>(KEY_ISSUED).await {
                state.issued = v;
            }
            if let Ok(Some(v)) = storage.get::<Vec<Outcome>>(KEY_OUTCOMES).await {
                state.outcomes = v;
            }
            if let Ok(Some(v)) = storage.get::<Vec<Activity>>(KEY_ACTIVITY).await {
                state.activity = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, Freeze>>(KEY_FREEZES).await {
                state.freezes = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, Connector>>(KEY_CONNECTORS).await {
                state.connectors = v;
            }
            if let Ok(Some(v)) = storage.get::<HashMap<String, Repository>>(KEY_REPOSITORIES).await {
                state.repositories = v;
            }
            if let Ok(Some(v)) = storage
                .get::<HashMap<String, Vec<Checkpoint>>>(KEY_CHECKPOINTS)
                .await
            {
                state.checkpoints = v;
            }
            // Try the current shape first. Fall back to a bare `String`
            // for a DO that migrated under the pre-fix build of 5b,
            // where `put(KEY_WHITEPAPER, &legacy.whitepaper)` wrote a
            // bare string directly (only reachable if that DO's
            // whitepaper was `Some` at the time -- `None` is exactly
            // what crashed the write before it landed). Silently
            // reading such a value as `WhitepaperBlob` fails to
            // deserialize, `if let Ok(Some(..))` swallows the error, and
            // the whitepaper reads back as `None` -- data loss with no
            // error surfaced anywhere. The fallback exists so that
            // silent case cannot happen, whether or not any DO actually
            // hit it.
            match storage.get::<WhitepaperBlob>(KEY_WHITEPAPER).await {
                Ok(Some(w)) => state.whitepaper = w.text,
                Ok(None) => {}
                Err(_) => {
                    if let Ok(Some(s)) = storage.get::<String>(KEY_WHITEPAPER).await {
                        state.whitepaper = Some(s);
                    }
                }
            }

            // Best-effort cleanup of an orphaned legacy blob: the crash
            // case where meta landed but the delete did not. Failure here
            // is fine -- the legacy key is unreachable once meta exists.
            let _ = storage.delete(KEY_LEGACY).await;
            return Ok(Some(state));
        }

        // 2. Legacy blob exists, no meta yet. Migrate.
        //
        //    Order matters. Every entity key is written first; meta is
        //    written last, as the commit marker. A crash after writing
        //    three entity keys but before meta leaves the legacy blob
        //    intact, and the next load reruns this whole block from the
        //    top. `put` overwrites, so the three already-written keys are
        //    rewritten with the same data. Idempotent.
        //
        //    A crash *after* meta but before deleting legacy leaves an
        //    orphaned kernel_state, which the branch above already cleans
        //    up on the next load.
        if let Ok(Some(legacy)) = storage.get::<KernelState>(KEY_LEGACY).await {
            guarded_put(&storage, KEY_TOKENS, &legacy.tokens).await?;
            guarded_put(&storage, KEY_KNOCKS, &legacy.knocks).await?;
            guarded_put(&storage, KEY_INTENTS, &legacy.intents).await?;
            guarded_put(&storage, KEY_APPROVALS, &legacy.approvals).await?;
            guarded_put(&storage, KEY_GRANTS, &legacy.grants).await?;
            guarded_put(&storage, KEY_CONSUMED, &legacy.consumed).await?;
            guarded_put(&storage, KEY_ISSUED, &legacy.issued).await?;
            guarded_put(&storage, KEY_OUTCOMES, &legacy.outcomes).await?;
            guarded_put(&storage, KEY_ACTIVITY, &legacy.activity).await?;
            guarded_put(&storage, KEY_FREEZES, &legacy.freezes).await?;
            guarded_put(&storage, KEY_CONNECTORS, &legacy.connectors).await?;
            guarded_put(&storage, KEY_REPOSITORIES, &legacy.repositories).await?;
            guarded_put(&storage, KEY_CHECKPOINTS, &legacy.checkpoints).await?;
            guarded_put(
                &storage,
                KEY_WHITEPAPER,
                &WhitepaperBlob { text: legacy.whitepaper.clone() },
            )
            .await?;

            let meta = MetaBlob {
                owner_id: legacy.owner_id.clone(),
                owner: legacy.owner.clone(),
            };
            guarded_put(&storage, KEY_META, &meta).await?;

            let _ = storage.delete(KEY_LEGACY).await;
            return Ok(Some(legacy));
        }

        // 3. Fresh DO. Write a placeholder meta so the very next persist
        //    cannot be the "wrote entity keys but not meta" case, then
        //    return None. The kernel applies its own defaults on None;
        //    the placeholder's empty owner_id is indistinguishable from
        //    "fresh" for the kernel's purposes, because `load()` in the
        //    kernel already does `if state.owner_id.is_empty() { ... }`.
        let placeholder = MetaBlob::default();
        guarded_put(&storage, KEY_META, &placeholder).await?;
        Ok(None)
    }

    async fn persist(&self, state: &KernelState, dirty: &DirtyKinds) -> Result<(), String> {
        let storage = self.storage.lock().map_err(|e| e.to_string())?;

        // Every conditional key below is guarded by its own dirty bit.
        // A mutation that touched one entity kind writes one key. This is
        // the whole point of the split.
        if dirty.contains(DirtyKinds::TOKENS) {
            guarded_put(&storage, KEY_TOKENS, &state.tokens).await?;
        }
        if dirty.contains(DirtyKinds::KNOCKS) {
            guarded_put(&storage, KEY_KNOCKS, &state.knocks).await?;
        }
        if dirty.contains(DirtyKinds::INTENTS) {
            guarded_put(&storage, KEY_INTENTS, &state.intents).await?;
        }
        if dirty.contains(DirtyKinds::APPROVALS) {
            guarded_put(&storage, KEY_APPROVALS, &state.approvals).await?;
        }
        if dirty.contains(DirtyKinds::GRANTS) {
            guarded_put(&storage, KEY_GRANTS, &state.grants).await?;
        }
        if dirty.contains(DirtyKinds::CONSUMED) {
            guarded_put(&storage, KEY_CONSUMED, &state.consumed).await?;
        }
        if dirty.contains(DirtyKinds::ISSUED) {
            guarded_put(&storage, KEY_ISSUED, &state.issued).await?;
        }
        if dirty.contains(DirtyKinds::OUTCOMES) {
            guarded_put(&storage, KEY_OUTCOMES, &state.outcomes).await?;
        }
        if dirty.contains(DirtyKinds::ACTIVITY) {
            guarded_put(&storage, KEY_ACTIVITY, &state.activity).await?;
        }
        if dirty.contains(DirtyKinds::FREEZES) {
            guarded_put(&storage, KEY_FREEZES, &state.freezes).await?;
        }
        if dirty.contains(DirtyKinds::CONNECTORS) {
            guarded_put(&storage, KEY_CONNECTORS, &state.connectors).await?;
        }
        if dirty.contains(DirtyKinds::REPOSITORIES) {
            guarded_put(&storage, KEY_REPOSITORIES, &state.repositories).await?;
        }
        if dirty.contains(DirtyKinds::CHECKPOINTS) {
            guarded_put(&storage, KEY_CHECKPOINTS, &state.checkpoints).await?;
        }
        if dirty.contains(DirtyKinds::WHITEPAPER) {
            guarded_put(
                &storage,
                KEY_WHITEPAPER,
                &WhitepaperBlob { text: state.whitepaper.clone() },
            )
            .await?;
        }

        // Meta unconditionally, per the struct doc above. Two short
        // strings plus an optional OwnerRecord: a few dozen bytes, well
        // under the DO storage value limit, and its presence is what
        // `load` keys on to distinguish new format from fresh DO.
        let meta = MetaBlob {
            owner_id: state.owner_id.clone(),
            owner: state.owner.clone(),
        };
        guarded_put(&storage, KEY_META, &meta).await?;

        Ok(())
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

pub const SESSION_COOKIE_NAME: &str = "raj_session";

/// The session cookie the top-level fetch handler sets after a
/// successful ?token= visit, so every visit after the first doesn't
/// need the query string -- same secret, just found in a second place.
fn session_cookie(hr: &HttpRequestDto) -> Option<&str> {
    hr.header("cookie")?.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k == SESSION_COOKIE_NAME).then_some(v)
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

    /// True when this request's `?token=` (not the cookie, not the
    /// header) is the live RAJ_TOKEN -- the signal the top-level fetch
    /// handler uses to decide whether to set the session cookie, so a
    /// visit that already succeeded via the cookie doesn't re-set an
    /// identical one on every request.
    pub fn admin_token_matches_query(&self, hr: &HttpRequestDto) -> bool {
        match (&self.admin_token, query_token(hr)) {
            (Some(expected), Some(presented)) => {
                constant_time_eq(presented.as_bytes(), expected.as_bytes())
            }
            _ => false,
        }
    }

    pub async fn authenticate(&self, hr: &HttpRequestDto) -> Result<Option<Identity>, String> {
        // Admin break-glass: a bearer token matching the RAJ_TOKEN Worker
        // secret authenticates as owner, independent of Cloudflare Access
        // entirely -- the whole point is a path in that doesn't depend on
        // the Access/JWKS verification this file already defers (see
        // DEFERRED.md). Checked first because it's the strongest, most
        // explicit credential of the three checked here.
        if let Some(expected) = &self.admin_token {
            let presented = bearer_token(hr)
                .or_else(|| query_token(hr))
                .or_else(|| session_cookie(hr));
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

