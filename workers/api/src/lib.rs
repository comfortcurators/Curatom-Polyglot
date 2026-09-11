/*
Intent : Carry Contract 1 and Contract 2 over the kernel, adding no authority of its own.
Pattern: cargo check --target wasm32-unknown-unknown. Every kernel call here is on Kernel today.
Signed. Claude / 2026-09-11 UTC

This file is the integration layer and nothing else. It authenticates, it
translates HTTP into kernel calls, it renders organic views, and it hands a
spent grant's attestation to Elixir. Every rule it appears to enforce is
enforced in `curatom-key-kernel`; if a rule looks like it lives here, that is
a bug.

The previous version of this file was written against a kernel that existed
only in a conversation -- `StateError`, `RequestRecord`, `human_label`,
`list_events`. It did not compile. Nothing below calls a method that is not
on `Kernel` in crates/key-kernel/src/lib.rs.
*/

use worker::*;

use std::cell::RefCell;

use curatom_attestation::{hmac_hex, hmac_key_bytes, AttestationClaims};
use curatom_key_kernel::Kernel;
use curatom_organic_router::{activity_view, approval_view, intent_view, me_view};
use curatom_protocol::{
    ApprovalDecision, Duration as CuratomDuration, HttpRequestDto, InorganicRequest, Outcome,
    Permission,
};
use curatom_sign::{DevSignVerifier, SignVerifier};
use curatom_substrate_cloudflare::{
    to_dto, CloudflareAccessIdentityProvider, CloudflareClock, DOEventLedger, DOStateStore,
    ProductionCloudflareInorganicIdentityProvider, R2ArtifactStore,
};
use curatom_ports::Clock;

use serde_json::{json, Value};

type K = Kernel<DOStateStore, DOEventLedger, R2ArtifactStore, CloudflareClock>;

/// A status and a body. `worker::Result` is single-parameter, so the
/// handlers return this pair rather than a `Result` that `use worker::*`
/// would shadow.
type Reply = (u16, Value);

/// How long an attestation is good for once handed to Elixir.
const ATTESTATION_TTL_SECONDS: i64 = 300;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let ns = env.durable_object("CURATOM_KERNEL")?;
    let id = ns.id_from_name(&env.var("CURATOM_OWNER_ID")?.to_string())?;
    id.get_stub()?.fetch_with_request(req).await
}

#[durable_object]
pub struct CuratomKernel {
    state: State,
    env: Env,
    kernel: RefCell<Option<K>>,
    owner_id: String,
    hmac_key: Vec<u8>,
    organic_idp: Option<CloudflareAccessIdentityProvider>,
    inorganic_idp: ProductionCloudflareInorganicIdentityProvider,
    bucket: Option<Bucket>,
}

impl DurableObject for CuratomKernel {
    fn new(state: State, env: Env) -> Self {
        console_error_panic_hook::set_once();

        let owner_id = env
            .var("CURATOM_OWNER_ID")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "organic_rajvansh".into());

        let hmac_key = env
            .secret("CURATOM_HMAC_KEY")
            .map(|s| hmac_key_bytes(&s.to_string()))
            .unwrap_or_default();

        let organic_idp = CloudflareAccessIdentityProvider::new(&env, owner_id.clone()).ok();
        let bucket = env.bucket("CURATOM_ARTIFACTS").ok();

        Self {
            state,
            env,
            kernel: RefCell::new(None),
            owner_id,
            hmac_key,
            organic_idp,
            inorganic_idp: ProductionCloudflareInorganicIdentityProvider,
            bucket,
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        self.ensure().await?;

        let hr = to_dto(req).await?;
        let path = Url::parse(&hr.url)?.path().to_string();
        let method = hr.method.to_ascii_uppercase();

        let (status, body) = match (method.as_str(), path.as_str()) {
            ("GET", "/organic/me") => self.h_me(&hr).await,
            ("POST", "/organic/intents") => self.h_create_intent(&hr).await,
            ("GET", p) if p.starts_with("/organic/intents/") => {
                let id = p.trim_start_matches("/organic/intents/").to_string();
                self.h_get_intent(&hr, &id).await
            }
            ("GET", "/organic/approvals") => self.h_list_approvals(&hr).await,
            ("POST", p) if p.starts_with("/organic/approvals/") && p.ends_with("/approve") => {
                let id = p
                    .trim_start_matches("/organic/approvals/")
                    .trim_end_matches("/approve")
                    .to_string();
                self.h_approve(&hr, &id).await
            }
            ("POST", p) if p.starts_with("/organic/approvals/") && p.ends_with("/refuse") => {
                let id = p
                    .trim_start_matches("/organic/approvals/")
                    .trim_end_matches("/refuse")
                    .to_string();
                self.h_refuse(&hr, &id).await
            }
            ("GET", "/organic/activity") => self.h_activity(&hr).await,
            ("POST", "/inorganic/requests") => self.h_inorganic_submit(&hr).await,
            ("GET", p) if p.starts_with("/inorganic/requests/") => {
                let id = p.trim_start_matches("/inorganic/requests/").to_string();
                self.h_inorganic_status(&hr, &id).await
            }
            ("POST", "/internal/outcome") => self.h_internal_outcome(&hr).await,
            _ => (404, json!({ "error": "not_found" })),
        };

        Ok(Response::from_json(&body)?.with_status(status))
    }
}

impl CuratomKernel {
    async fn ensure(&self) -> Result<()> {
        if self.kernel.borrow().is_some() {
            return Ok(());
        }
        if self.owner_id.is_empty() {
            return Err(Error::RustError("CURATOM_OWNER_ID not configured".into()));
        }
        if self.hmac_key.is_empty() {
            return Err(Error::RustError("CURATOM_HMAC_KEY not configured".into()));
        }
        if self.organic_idp.is_none() {
            return Err(Error::RustError("organic identity provider unavailable".into()));
        }
        let bucket = self
            .bucket
            .clone()
            .ok_or_else(|| Error::RustError("CURATOM_ARTIFACTS bucket missing".into()))?;

        let mut k = Kernel::new(
            self.owner_id.clone(),
            DOStateStore::new(self.state.storage()),
            DOEventLedger::new(self.state.storage()),
            R2ArtifactStore::new(bucket, self.owner_id.clone()),
            CloudflareClock,
        );
        k.load().await.map_err(Error::RustError)?;
        *self.kernel.borrow_mut() = Some(k);
        Ok(())
    }

    /// Contract 1 is owner-only. Anything short of the owner is a refusal,
    /// never a downgrade.
    async fn owner(&self, hr: &HttpRequestDto) -> std::result::Result<(), Reply> {
        let idp = self
            .organic_idp
            .as_ref()
            .ok_or((500, json!({ "error": "idp_uninitialized" })))?;
        match idp.authenticate(hr).await {
            Ok(Some(id)) if id.id == self.owner_id => Ok(()),
            Ok(Some(_)) => Err((403, json!({ "error": "not_owner" }))),
            Ok(None) => Err((401, json!({ "error": "unauthenticated" }))),
            Err(e) => Err((500, json!({ "error": e }))),
        }
    }

    // ---- organic ----

    async fn h_me(&self, hr: &HttpRequestDto) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        (200, me_view(&self.owner_id))
    }

    async fn h_create_intent(&self, hr: &HttpRequestDto) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        let Ok(body) = serde_json::from_str::<Value>(&hr.body) else {
            return (400, json!({ "error": "invalid_json" }));
        };
        let Some(text) = body.get("text").and_then(|v| v.as_str()) else {
            return (400, json!({ "error": "missing_text" }));
        };
        let owner = self.owner_id.clone();
        let mut k = self.kernel.borrow_mut();
        match k.as_mut().unwrap().create_intent(text, &owner).await {
            Ok(intent) => (201, intent_view(&intent)),
            Err(e) => (500, json!({ "error": e })),
        }
    }

    async fn h_get_intent(&self, hr: &HttpRequestDto, intent_id: &str) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        match self.kernel.borrow().as_ref().unwrap().get_intent(intent_id) {
            Ok(Some(i)) => (200, intent_view(&i)),
            Ok(None) => (404, json!({ "error": "unknown_intent" })),
            Err(e) => (500, json!({ "error": e })),
        }
    }

    async fn h_list_approvals(&self, hr: &HttpRequestDto) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        match self.kernel.borrow().as_ref().unwrap().list_pending_approvals() {
            Ok(list) => {
                let out: Vec<Value> = list
                    .iter()
                    .map(|a| serde_json::to_value(approval_view(a)).unwrap_or(Value::Null))
                    .collect();
                (200, Value::Array(out))
            }
            Err(e) => (500, json!({ "error": e })),
        }
    }

    /// Approve: SIGN2, decide, issue, consume every pair, attest, hand off.
    ///
    /// The consume happens before the POST on purpose. Elixir receives proof
    /// that a capability was spent, never a capability it could spend itself.
    async fn h_approve(&self, hr: &HttpRequestDto, approval_id: &str) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        let Ok(body) = serde_json::from_str::<Value>(&hr.body) else {
            return (400, json!({ "error": "invalid_json" }));
        };
        let Some(sign2) = body.get("sign2").and_then(|v| v.as_str()) else {
            return (400, json!({ "error": "missing_sign2" }));
        };
        if !matches!(
            DevSignVerifier::new()
                .verify("sign2", &self.owner_id, sign2, approval_id)
                .await,
            Ok(true)
        ) {
            return (403, json!({ "error": "sign2_failed" }));
        }

        // Snapshot before borrowing the kernel mutably across awaits.
        let hmac_key = self.hmac_key.clone();
        let approval_id = approval_id.to_string();
        let fetcher = match self.env.service("CURATOM_ORCHESTRATOR") {
            Ok(f) => f,
            Err(e) => {
                return (
                    502,
                    json!({ "error": format!("service binding: {e}") }),
                )
            }
        };

        let mut k = self.kernel.borrow_mut();
        let k = k.as_mut().unwrap();

        match k.decide_approval(&approval_id, ApprovalDecision::Approve).await {
            Ok(Some(_)) => {}
            Ok(None) => return (409, json!({ "error": "approval_not_pending" })),
            // digest_mismatch: the card moved under the owner. Not a server fault.
            Err(e) => return (409, json!({ "error": e })),
        }

        let secret = match k.issue_grant(&approval_id).await {
            Ok(Some(issued)) => issued.handle.secret,
            Ok(None) => return (409, json!({ "error": "grant_already_issued" })),
            Err(e) => return (500, json!({ "error": e })),
        };

        let now = CloudflareClock.now_unix();
        let job_id = curatom_crypto::random_id("job");
        let mut claims = Vec::new();
        for resource in &secret.resources {
            for op in &secret.permissions {
                claims.push(AttestationClaims {
                    v: 1,
                    job_id: job_id.clone(),
                    grant_id: secret.grant_id.clone(),
                    requester_id: secret.requester_id.clone(),
                    resource: resource.clone(),
                    operation: op.as_str().to_string(),
                    issued_unix: now,
                    expires_unix: now + ATTESTATION_TTL_SECONDS,
                    nonce: curatom_crypto::random_id("nonce"),
                });
            }
        }
        if claims.is_empty() {
            return (400, json!({ "error": "no_actions_authorized" }));
        }

        let mut actions = Vec::with_capacity(claims.len());
        for c in &claims {
            let op = Permission::parse(&c.operation);
            if let Err(e) = k
                .consume_capability(&secret.token, &secret.requester_id, &c.resource, op)
                .await
            {
                return (500, json!({ "error": e }));
            }
            match curatom_attestation::sign(c, &hmac_key) {
                Ok(token) => actions.push(json!({
                    "resource": c.resource,
                    "operation": c.operation,
                    "attestation": token,
                })),
                Err(e) => return (500, json!({ "error": e })),
            }
        }

        let envelope = json!({
            "job_id": job_id,
            "approval_id": approval_id,
            "intent_id": secret.intent_id,
            "requester_id": secret.requester_id,
            "actions": actions,
        });
        let raw = match serde_json::to_string(&envelope) {
            Ok(s) => s,
            Err(e) => return (500, json!({ "error": e.to_string() })),
        };

        match post_signed(&fetcher, &raw, &hmac_key).await {
            Ok(code) if code < 400 => (200, json!({ "ok": true, "job_handed_off": true })),
            Ok(code) => (502, json!({ "error": format!("orchestrator status {code}") })),
            Err(e) => (502, json!({ "error": format!("service binding send: {e}") })),
        }
    }

    async fn h_refuse(&self, hr: &HttpRequestDto, approval_id: &str) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        let mut k = self.kernel.borrow_mut();
        match k.as_mut().unwrap().decide_approval(approval_id, ApprovalDecision::Refuse).await {
            Ok(Some(_)) => (200, json!({ "ok": true })),
            Ok(None) => (409, json!({ "error": "approval_not_pending" })),
            Err(e) => (500, json!({ "error": e })),
        }
    }

    async fn h_activity(&self, hr: &HttpRequestDto) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        match self.kernel.borrow().as_ref().unwrap().activity() {
            Ok(items) => {
                let out: Vec<Value> = items
                    .iter()
                    .map(|a| serde_json::to_value(activity_view(a)).unwrap_or(Value::Null))
                    .collect();
                (200, Value::Array(out))
            }
            Err(e) => (500, json!({ "error": e })),
        }
    }

    // ---- inorganic ----
    //
    // Machine identity is deferred and the provider throws. That is the whole
    // behaviour: a request from a machine is refused because nothing here can
    // say who the machine is. The kernel call below runs only if that ever
    // changes.

    async fn h_inorganic_submit(&self, hr: &HttpRequestDto) -> Reply {
        let identity = match self.inorganic_idp.authenticate(hr).await {
            Ok(Some(id)) => id,
            Ok(None) => return (401, json!({ "error": "unauthenticated" })),
            Err(e) => return (503, json!({ "error": e })),
        };
        let Ok(body) = serde_json::from_str::<Value>(&hr.body) else {
            return (400, json!({ "error": "invalid_json" }));
        };
        let req = InorganicRequest {
            requester_id: identity.id,
            reason: body["reason"].as_str().unwrap_or_default().to_string(),
            resources: body["resources"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).map(String::from).collect())
                .unwrap_or_default(),
            permissions: body["permissions"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).map(Permission::parse).collect())
                .unwrap_or_else(|| vec![Permission::Read]),
            duration: match body["duration"]["seconds"].as_u64() {
                Some(seconds) => CuratomDuration::Ttl { seconds },
                None => CuratomDuration::SingleUse,
            },
        };
        let mut k = self.kernel.borrow_mut();
        match k.as_mut().unwrap().submit_inorganic(req).await {
            Ok(a) => (202, curatom_inorganic_router::submitted_view(&a)),
            Err(e) => (400, json!({ "error": e })),
        }
    }

    async fn h_inorganic_status(&self, hr: &HttpRequestDto, approval_id: &str) -> Reply {
        match self.inorganic_idp.authenticate(hr).await {
            Ok(None) => return (401, json!({ "error": "unauthenticated" })),
            Err(e) => return (503, json!({ "error": e })),
            Ok(Some(_)) => {}
        }
        match self.kernel.borrow().as_ref().unwrap().get_approval(approval_id) {
            Ok(Some(a)) => (200, curatom_inorganic_router::status_view(&a)),
            Ok(None) => (404, json!({ "error": "unknown_request" })),
            Err(e) => (500, json!({ "error": e })),
        }
    }

    // ---- internal (Contract 3 callback) ----

    async fn h_internal_outcome(&self, hr: &HttpRequestDto) -> Reply {
        let Some(provided) = hr.header("x-curatom-hmac") else {
            return (401, json!({ "error": "missing_hmac" }));
        };
        let expected = hmac_hex(hr.body.as_bytes(), &self.hmac_key);
        if !curatom_attestation::constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
            return (401, json!({ "error": "bad_hmac" }));
        }
        let Ok(body) = serde_json::from_str::<Value>(&hr.body) else {
            return (400, json!({ "error": "invalid_json" }));
        };

        let outcome = Outcome {
            id: curatom_crypto::random_id("out"),
            intent_id: body["intent_id"].as_str().unwrap_or_default().into(),
            execution_id: body["job_id"].as_str().unwrap_or_default().into(),
            grant_id: body["grant_id"].as_str().unwrap_or_default().into(),
            resource: body["resource"].as_str().unwrap_or_default().into(),
            operation: Permission::parse(body["operation"].as_str().unwrap_or("read")),
            ok: body["ok"].as_bool().unwrap_or(false),
            data: body.get("data").cloned(),
            error: body.get("error").and_then(|v| v.as_str()).map(String::from),
            provider: "hostos".into(),
            mock: body["mock"].as_bool(),
            at: CloudflareClock.now_iso(),
        };

        let mut k = self.kernel.borrow_mut();
        match k.as_mut().unwrap().record_outcome(outcome).await {
            Ok(()) => (200, json!({ "ok": true })),
            Err(e) => (500, json!({ "error": e })),
        }
    }
}

/// Contract 2's one POST. Service binding, not a public URL.
/// `Fetcher::fetch_request` takes `worker::Request` (TryInto<Request>).
/// Response status is `status_code()`, not `status()`.
async fn post_signed(fetcher: &Fetcher, raw: &str, key: &[u8]) -> Result<u16> {
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    headers.set("x-curatom-hmac", &hmac_hex(raw.as_bytes(), key))?;
    headers.set("x-curatom-probe", "kernel-handoff-v1")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(raw)));

    let req = Request::new_with_init("https://curatom-orchestrator/v1/jobs", &init)?;
    let resp = fetcher.fetch_request(req).await?;
    Ok(resp.status_code())
}
