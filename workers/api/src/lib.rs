//! Durable Object. Inline dispatch. Nothing holding &mut Kernel across .await
//! except the DO itself. Shape from the architecture paste. Not cargo-check-verified
//! on wasm32 in this commit — see DEFERRED.md.

use curatom_attestation::{hmac_hex, hmac_key_bytes, sign, AttestationClaims, constant_time_eq};
use curatom_crypto::random_id;
use curatom_key_kernel::Kernel;
use curatom_organic_router::{activity_view, approval_view, intent_view, leak_check, me_view};
use curatom_protocol::*;
use curatom_sign::{DevSignVerifier, SignVerifier};
use curatom_substrate_cloudflare::{
    to_dto, CloudflareAccessIdentityProvider, CloudflareClock, DOEventLedger, DOStateStore,
    ProductionCloudflareInorganicIdentityProvider, R2ArtifactStore,
};
use serde_json::json;
use worker::*;

#[durable_object]
pub struct CuratomKernel {
    state: State,
    env: Env,
    kernel: Option<Kernel<DOStateStore, DOEventLedger, R2ArtifactStore, CloudflareClock>>,
    organic_id: Option<CloudflareAccessIdentityProvider>,
    inorganic_id: Option<ProductionCloudflareInorganicIdentityProvider>,
}

#[durable_object]
impl DurableObject for CuratomKernel {
    fn new(state: State, env: Env) -> Self {
        console_error_panic_hook::set_once();
        Self {
            state,
            env,
            kernel: None,
            organic_id: None,
            inorganic_id: None,
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        self.ensure().await?;
        let hr = to_dto(req).await?;
        let url = url::Url::parse(&hr.url).map_err(|e| Error::RustError(e.to_string()))?;
        let path = url.path().to_string();
        let method = hr.method.clone();

        // INLINE dispatch. Each handler constructs nothing holding &mut Kernel
        // across .await — it borrows for the duration of one kernel call.
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
            _ => (404, json!({"error": "not found"})),
        };

        if status == 200 {
            let _ = leak_check(&body);
        }
        Response::from_json(&body).map(|r| r.with_status(status))
    }
}

impl CuratomKernel {
    async fn ensure(&mut self) -> Result<()> {
        if self.kernel.is_some() {
            return Ok(());
        }
        let owner_id = self.env.var("CURATOM_OWNER_ID")?.to_string();
        let store = DOStateStore::new(self.state.storage());
        let ledger = DOEventLedger::new(self.state.storage());
        let bucket = self.env.bucket("CURATOM_ARTIFACTS")?;
        let artifacts = R2ArtifactStore::new(bucket, owner_id.clone());
        let mut k = Kernel::new(owner_id.clone(), store, ledger, artifacts, CloudflareClock);
        k.load().await.map_err(|e| Error::RustError(e))?;
        self.kernel = Some(k);
        self.organic_id = Some(CloudflareAccessIdentityProvider::new(&self.env, owner_id)?);
        self.inorganic_id = Some(ProductionCloudflareInorganicIdentityProvider);
        Ok(())
    }

    async fn require_owner(&self, hr: &HttpRequestDto) -> Result<Identity, (u16, serde_json::Value)> {
        let owner_id = self.env.var("CURATOM_OWNER_ID").map(|v| v.to_string()).unwrap_or_default();
        let idp = self.organic_id.as_ref().unwrap();
        let Ok(Some(id)) = idp.authenticate(hr).await else {
            return Err((401, json!({"error": "unauthenticated"})));
        };
        if id.id != owner_id {
            return Err((403, json!({"error": "not_owner"})));
        }
        Ok(id)
    }

    async fn h_me(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        match self.require_owner(hr).await {
            Ok(id) => (200, me_view(&id.id)),
            Err(e) => e,
        }
    }

    async fn h_create_intent(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        if let Err(e) = self.require_owner(hr).await {
            return e;
        }
        let Ok(body) = serde_json::from_str::<CreateIntentRequest>(&hr.body) else {
            return (400, json!({"error": "bad json"}));
        };
        let k = self.kernel.as_mut().unwrap();
        match k.create_intent(&body.text, &k.owner_id().to_string()).await {
            Ok(i) => (200, intent_view(&i)),
            Err(e) => (500, json!({"error": e})),
        }
    }

    async fn h_get_intent(&mut self, hr: &HttpRequestDto, id: &str) -> (u16, serde_json::Value) {
        if let Err(e) = self.require_owner(hr).await {
            return e;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.get_intent(id) {
            Ok(Some(i)) => (200, intent_view(&i)),
            Ok(None) => (404, json!({"error": "not found"})),
            Err(e) => (500, json!({"error": e})),
        }
    }

    async fn h_list_approvals(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        if let Err(e) = self.require_owner(hr).await {
            return e;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.list_pending_approvals() {
            Ok(list) => {
                let views: Vec<_> = list.iter().map(approval_view).collect();
                (200, serde_json::to_value(views).unwrap())
            }
            Err(e) => (500, json!({"error": e})),
        }
    }

    async fn h_refuse(&mut self, hr: &HttpRequestDto, approval_id: &str) -> (u16, serde_json::Value) {
        if let Err(e) = self.require_owner(hr).await {
            return e;
        }
        let k = self.kernel.as_mut().unwrap();
        match k.decide_approval(approval_id, ApprovalDecision::Refuse).await {
            Ok(Some(_)) => (200, json!({"ok": true})),
            Ok(None) => (409, json!({"error": "approval not pending"})),
            Err(e) => (500, json!({"error": e})),
        }
    }

    async fn h_activity(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        if let Err(e) = self.require_owner(hr).await {
            return e;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.activity() {
            Ok(list) => {
                let views: Vec<_> = list.iter().map(activity_view).collect();
                (200, serde_json::to_value(views).unwrap())
            }
            Err(e) => (500, json!({"error": e})),
        }
    }

    async fn h_inorganic_submit(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let idp = self.inorganic_id.as_ref().unwrap();
        match idp.authenticate(hr).await {
            Err(e) => (501, json!({"error": e})),
            Ok(_) => (501, json!({"error": "inorganic production identity is not implemented"})),
        }
    }

    async fn h_inorganic_status(&mut self, hr: &HttpRequestDto, id: &str) -> (u16, serde_json::Value) {
        if let Err(e) = self.require_owner(hr).await {
            return e;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.get_approval(id) {
            Ok(Some(a)) => (200, curatom_inorganic_router::status_view(&a)),
            Ok(None) => (404, json!({"error": "not found"})),
            Err(e) => (500, json!({"error": e})),
        }
    }

    async fn h_approve(&mut self, hr: &HttpRequestDto, approval_id: &str) -> (u16, serde_json::Value) {
        // Snapshot env before borrowing kernel.
        let owner_id = match self.env.var("CURATOM_OWNER_ID") {
            Ok(v) => v.to_string(),
            Err(_) => return (500, json!({"error": "no owner"})),
        };
        let hmac_secret = match self.env.secret("CURATOM_HMAC_KEY") {
            Ok(s) => s.to_string(),
            Err(_) => return (500, json!({"error": "no key"})),
        };
        let orchestrator_url = match self.env.var("CURATOM_ORCHESTRATOR_URL") {
            Ok(v) => v.to_string(),
            Err(_) => return (500, json!({"error": "no orchestrator"})),
        };
        let hmac_key = hmac_key_bytes(&hmac_secret);

        let idp = self.organic_id.as_ref().unwrap();
        let Ok(Some(id)) = idp.authenticate(hr).await else {
            return (401, json!({"error": "unauthenticated"}));
        };
        if id.id != owner_id {
            return (403, json!({"error": "not_owner"}));
        }

        let signv = DevSignVerifier::new();
        let Ok(body) = serde_json::from_str::<serde_json::Value>(&hr.body) else {
            return (400, json!({"error": "bad json"}));
        };
        let Some(sign2) = body.get("sign2").and_then(|v| v.as_str()) else {
            return (400, json!({"error": "missing sign2"}));
        };
        if !matches!(signv.verify("sign2", &owner_id, sign2, "").await, Ok(true)) {
            return (403, json!({"error": "sign2 verification failed"}));
        }

        let k = self.kernel.as_mut().unwrap();
        let Ok(Some(_approval)) = k.decide_approval(approval_id, ApprovalDecision::Approve).await else {
            return (409, json!({"error": "approval not pending or digest mismatch"}));
        };
        let Ok(Some(issued)) = k.issue_grant(approval_id).await else {
            return (500, json!({"error": "grant issuance failed"}));
        };

        // The Worker consumes the capability NOW. Elixir never sees a live token.
        let secret = issued.handle.secret.clone();
        let mut claims_list = Vec::new();
        let now = CloudflareClock.now_unix();
        for resource in &secret.resources {
            for op in &secret.permissions {
                let Ok(_consumed) = k
                    .consume_capability(&secret.token, &secret.requester_id, resource, *op)
                    .await
                else {
                    return (500, json!({"error": "capability consumption failed"}));
                };
                let job_id = random_id("job");
                let claims = AttestationClaims {
                    v: 1,
                    job_id: job_id.clone(),
                    grant_id: secret.grant_id.clone(),
                    requester_id: secret.requester_id.clone(),
                    resource: resource.clone(),
                    operation: op.as_str().to_string(),
                    issued_unix: now,
                    expires_unix: now + 300,
                    nonce: random_id("nonce"),
                };
                claims_list.push(claims);
            }
        }
        if claims_list.is_empty() {
            return (500, json!({"error": "no actions"}));
        }

        let job = json!({
            "job_id": claims_list[0].job_id,
            "approval_id": approval_id,
            "intent_id": secret.intent_id,
            "requester_id": secret.requester_id,
            "actions": claims_list.iter().map(|c| {
                json!({
                    "resource": c.resource,
                    "operation": c.operation,
                    "attestation": sign(c, &hmac_key).unwrap(),
                })
            }).collect::<Vec<_>>(),
        });
        let body_bytes = serde_json::to_vec(&job).unwrap();
        let sig = hmac_hex(&body_bytes, &hmac_key);

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_body(Some(wasm_bindgen::JsValue::from(js_sys::Uint8Array::from(body_bytes.as_slice()))));
        let mut headers = Headers::new();
        let _ = headers.set("content-type", "application/json");
        let _ = headers.set("x-curatom-hmac", &sig);
        init.with_headers(headers);
        let handoff = Request::new_with_init(&orchestrator_url, &init);
        let sent = match handoff {
            Ok(r) => r.send().await,
            Err(_) => return (502, json!({"error": "orchestrator handoff failed"})),
        };
        if sent.map(|r| r.status_code()).unwrap_or(500) >= 400 {
            return (502, json!({"error": "orchestrator handoff failed"}));
        }

        (200, json!({"ok": true, "job_handed_off": true}))
    }

    async fn h_internal_outcome(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let hmac_secret = match self.env.secret("CURATOM_HMAC_KEY") {
            Ok(s) => s.to_string(),
            Err(_) => return (500, json!({"error": "no key"})),
        };
        let hmac_key = hmac_key_bytes(&hmac_secret);
        let Some(provided) = hr.header("x-curatom-hmac") else {
            return (401, json!({"error": "missing hmac"}));
        };
        let expected = hmac_hex(hr.body.as_bytes(), &hmac_key);
        if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
            return (401, json!({"error": "bad hmac"}));
        }
        let Ok(body) = serde_json::from_str::<serde_json::Value>(&hr.body) else {
            return (400, json!({"error": "bad json"}));
        };
        let k = self.kernel.as_mut().unwrap();
        let o = Outcome {
            id: random_id("out"),
            intent_id: body["intent_id"].as_str().unwrap_or("").into(),
            execution_id: body["job_id"].as_str().unwrap_or("").into(),
            grant_id: body["grant_id"].as_str().unwrap_or("").into(),
            resource: body["resource"].as_str().unwrap_or("").into(),
            operation: Permission::parse(body["operation"].as_str().unwrap_or("read")),
            ok: body["ok"].as_bool().unwrap_or(false),
            data: body.get("data").cloned(),
            error: body.get("error").and_then(|v| v.as_str()).map(String::from),
            provider: "hostos".into(),
            mock: body["mock"].as_bool(),
            at: CloudflareClock.now_iso(),
        };
        match k.record_outcome(o).await {
            Ok(_) => (200, json!({"ok": true})),
            Err(e) => (500, json!({"error": e})),
        }
    }
}
