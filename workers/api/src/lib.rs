use worker::*;
use curatom_protocol::{
    Approval, Duration, Intent, Outcome, Permission, StateError,
};
use curatom_key_kernel::{ApprovalDecision, Kernel};
use curatom_ports::{Clock, HttpRequestDto, OrganicIdentityProvider};
use curatom_sign::{DevSignVerifier, RealSignVerifier, SignVerifier};
use curatom_substrate_cloudflare::{
    CloudflareAccessIdentityProvider, CloudflareClock,
    DOEventLedger, DOStateStore, R2ArtifactStore,
    ProductionCloudflareInorganicIdentityProvider,
};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;
type K = Kernel<DOStateStore, DOEventLedger, R2ArtifactStore, CloudflareClock>;

#[durable_object]
pub struct CuratomKernel {
    state: State,
    kernel: Option<K>,
    owner_id: String,
    hmac_key: Vec<u8>,
    orchestrator_url: String,
    organic_idp: Option<CloudflareAccessIdentityProvider>,
    inorganic_idp: Option<ProductionCloudflareInorganicIdentityProvider>,
    bucket: Option<worker::Bucket>,
}

#[durable_object]
impl DurableObject for CuratomKernel {
    fn new(state: State, env: Env) -> Self {
        console_error_panic_hook::set_once();

        let owner_id = env
            .var("CURATOM_OWNER_ID")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "organic_rajvansh".into());

        let hmac_raw = env
            .secret("CURATOM_HMAC_KEY")
            .map(|s| s.to_string())
            .unwrap_or_default();
        let hmac_key = B64.decode(hmac_raw.as_bytes()).unwrap_or_default();

        let orchestrator_url = env
            .var("CURATOM_ORCHESTRATOR_URL")
            .map(|v| v.to_string())
            .unwrap_or_default();

        let bucket = env.bucket("CURATOM_ARTIFACTS").ok();

        Self {
            state,
            kernel: None,
            owner_id,
            hmac_key,
            orchestrator_url,
            organic_idp: None,
            inorganic_idp: None,
            bucket,
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        self.ensure().await?;

        let hr = to_dto(req).await?;
        let url = Url::parse(&hr.url)?;
        let path = url.path().to_string();
        let method = hr.method.clone();

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
            _ => (404, serde_json::json!({"error": "not found"})),
        };

        let resp = Response::from_json(&body)?;
        Ok(resp.with_status(status))
    }
}

impl CuratomKernel {
    async fn ensure(&mut self) -> Result<()> {
        if self.kernel.is_some() {
            return Ok(());
        }

        if self.owner_id.is_empty() {
            return Err(Error::RustError("CURATOM_OWNER_ID not configured".into()));
        }
        if self.hmac_key.len() != 32 {
            return Err(Error::RustError(format!(
                "CURATOM_HMAC_KEY must decode to 32 bytes, got {}",
                self.hmac_key.len()
            )));
        }

        let store = DOStateStore::new(self.state.storage());
        let ledger = DOEventLedger::new(self.state.storage());
        let bucket = self
            .bucket
            .clone()
            .ok_or_else(|| Error::RustError("CURATOM_ARTIFACTS bucket missing".into()))?;
        let artifacts = R2ArtifactStore::new(bucket, self.owner_id.clone());

        let mut k = Kernel::new(
            self.owner_id.clone(),
            store,
            ledger,
            artifacts,
            CloudflareClock,
        );
        k.load().await.map_err(Error::RustError)?;

        self.kernel = Some(k);
        self.organic_idp = Some(CloudflareAccessIdentityProvider::new(
            self.owner_id.clone(),
        ));
        self.inorganic_idp = Some(ProductionCloudflareInorganicIdentityProvider);
        Ok(())
    }

    // ---- ORGANIC ----

    async fn h_me(&self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let idp = match &self.organic_idp {
            Some(i) => i,
            None => return (500, serde_json::json!({"error": "idp uninitialized"})),
        };
        let Ok(Some(id)) = idp.authenticate(hr).await else {
            return (401, serde_json::json!({"error": "unauthenticated"}));
        };
        if id.id != self.owner_id {
            return (403, serde_json::json!({"error": "not_owner"}));
        }
        (
            200,
            serde_json::json!({
                "id": id.id,
                "displayName": id.display_name,
                "mode": "owner",
            }),
        )
    }

    async fn h_create_intent(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let auth = self.require_owner(hr).await;
        if let Err(r) = auth {
            return r;
        }
        let body: serde_json::Value = match serde_json::from_str(&hr.body) {
            Ok(v) => v,
            Err(_) => return (400, serde_json::json!({"error": "invalid_json"})),
        };
        let Some(text) = body.get("text").and_then(|v| v.as_str()) else {
            return (400, serde_json::json!({"error": "missing text"}));
        };
        let k = self.kernel.as_mut().unwrap();
        match k.create_intent(text.to_string()).await {
            Ok(intent) => (201, serde_json::to_value(intent).unwrap()),
            Err(e) => (500, serde_json::json!({"error": e})),
        }
    }

    async fn h_get_intent(&self, hr: &HttpRequestDto, intent_id: &str) -> (u16, serde_json::Value) {
        let auth = self.require_owner(hr).await;
        if let Err(r) = auth {
            return r;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.get_intent(intent_id) {
            Some(i) => (200, serde_json::to_value(i).unwrap()),
            None => (404, serde_json::json!({"error": "unknown intent"})),
        }
    }

    async fn h_list_approvals(&self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let auth = self.require_owner(hr).await;
        if let Err(r) = auth {
            return r;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.list_pending_approvals(&self.owner_id) {
            Ok(list) => {
                let out: Vec<serde_json::Value> = list
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "id": a.id,
                            "requester": a.human_label.requester,
                            "reason": a.human_label.reason,
                            "resources": a.human_label.resources,
                            "permissions": a.human_label.permissions,
                            "duration": human_duration(&a.human_label.duration),
                        })
                    })
                    .collect();
                (200, serde_json::Value::Array(out))
            }
            Err(e) => (500, serde_json::json!({"error": e})),
        }
    }

    async fn h_approve(&mut self, hr: &HttpRequestDto, approval_id: &str) -> (u16, serde_json::Value) {
        let auth = self.require_owner(hr).await;
        if let Err(r) = auth {
            return r;
        }

        let body: serde_json::Value = match serde_json::from_str(&hr.body) {
            Ok(v) => v,
            Err(_) => return (400, serde_json::json!({"error": "invalid_json"})),
        };
        let Some(sign2) = body.get("sign2").and_then(|v| v.as_str()) else {
            return (400, serde_json::json!({"error": "missing sign2"}));
        };

        let sign = DevSignVerifier::new();
        if !matches!(sign.verify("sign2", &self.owner_id, sign2, "").await, Ok(true)) {
            return (403, serde_json::json!({"error": "sign2 verification failed"}));
        }

        // Snapshot everything we need from `self` BEFORE borrowing the kernel.
        let hmac_key = self.hmac_key.clone();
        let orchestrator_url = self.orchestrator_url.clone();
        let approval_id_owned = approval_id.to_string();

        let k = self.kernel.as_mut().unwrap();

        let Ok(Some(_approval)) = k
            .decide_approval(&approval_id_owned, ApprovalDecision::Approve)
            .await
        else {
            return (409, serde_json::json!({"error": "approval not pending or digest mismatch"}));
        };

        let Ok(Some(issued)) = k.issue_grant(&approval_id_owned).await else {
            return (500, serde_json::json!({"error": "grant issuance failed"}));
        };

        let secret = issued.handle.secret.clone();

        // Build claims first; consume after we know we have a non-empty action set.
        let now = CloudflareClock.now_unix();
        let mut claims_list = Vec::new();
        for resource in &secret.resources {
            for op in &secret.permissions {
                claims_list.push(curatom_attestation::AttestationClaims {
                    v: 1,
                    job_id: curatom_crypto::random_id("job"),
                    grant_id: secret.grant_id.clone(),
                    requester_id: secret.requester_id.clone(),
                    resource: resource.clone(),
                    operation: op_string(*op),
                    issued_unix: now,
                    expires_unix: now + 300,
                    nonce: curatom_crypto::random_id("nonce"),
                });
            }
        }
        if claims_list.is_empty() {
            return (400, serde_json::json!({"error": "no actions authorized by grant"}));
        }

        for c in &claims_list {
            let perm = parse_op(&c.operation);
            if k.consume_capability(&secret.token, &secret.requester_id, &c.resource, perm)
                .await
                .is_err()
            {
                return (500, serde_json::json!({"error": "capability consumption failed"}));
            }
        }

        let mut actions = Vec::with_capacity(claims_list.len());
        for c in &claims_list {
            let token = match curatom_attestation::sign(c, &hmac_key) {
                Ok(t) => t,
                Err(e) => return (500, serde_json::json!({"error": e})),
            };
            actions.push(serde_json::json!({
                "resource": c.resource,
                "operation": c.operation,
                "attestation": token,
            }));
        }

        let envelope = serde_json::json!({
            "job_id": claims_list[0].job_id,
            "approval_id": approval_id_owned,
            "intent_id": secret.intent_id,
            "requester_id": secret.requester_id,
            "actions": actions,
        });

        let envelope_bytes = match serde_json::to_vec(&envelope) {
            Ok(b) => b,
            Err(e) => return (500, serde_json::json!({"error": e.to_string()})),
        };

        let envelope_sig = hmac_hex(&envelope_bytes, &hmac_key);

        let mut init = RequestInit::new();
        init = init.with_method(Method::Post);
        let body_str = match std::str::from_utf8(&envelope_bytes) {
            Ok(s) => s,
            Err(e) => return (500, serde_json::json!({"error": e.to_string()})),
        };
        init = init.with_body(Some(wasm_bindgen::JsValue::from_str(body_str)));

        let headers = Headers::new();
        let _ = headers.set("content-type", "application/json");
        let _ = headers.set("x-curatom-hmac", &envelope_sig);
        init = init.with_headers(headers);

        let req = match Request::new_with_init(&orchestrator_url, &init) {
            Ok(r) => r,
            Err(e) => return (500, serde_json::json!({"error": format!("build: {e}")})),
        };
        match req.send().await {
            Ok(resp) if resp.status_code() < 400 => {
                (200, serde_json::json!({"ok": true, "job_handed_off": true}))
            }
            Ok(resp) => (
                502,
                serde_json::json!({"error": format!("orchestrator status {}", resp.status_code())}),
            ),
            Err(e) => (502, serde_json::json!({"error": format!("send: {e}")})),
        }
    }

    async fn h_refuse(&mut self, hr: &HttpRequestDto, approval_id: &str) -> (u16, serde_json::Value) {
        let auth = self.require_owner(hr).await;
        if let Err(r) = auth {
            return r;
        }
        let k = self.kernel.as_mut().unwrap();
        match k.decide_approval(approval_id, ApprovalDecision::Refuse).await {
            Ok(Some(_)) => (200, serde_json::json!({"ok": true})),
            Ok(None) => (409, serde_json::json!({"error": "approval not pending"})),
            Err(e) => (500, serde_json::json!({"error": e})),
        }
    }

    async fn h_activity(&self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let auth = self.require_owner(hr).await;
        if let Err(r) = auth {
            return r;
        }
        let k = self.kernel.as_ref().unwrap();
        match k.list_events().await {
            Ok(events) => {
                let out: Vec<serde_json::Value> = events
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "seq": e.seq,
                            "at": e.at,
                            "kind": e.kind,
                            "actor": e.actor,
                        })
                    })
                    .collect();
                (200, serde_json::Value::Array(out))
            }
            Err(e) => (500, serde_json::json!({"error": e})),
        }
    }

    // ---- INORGANIC ----

    async fn h_inorganic_submit(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        // Machine auth in v0 fails closed by design. Production must configure a real provider.
        let idp = match &self.inorganic_idp {
            Some(i) => i,
            None => return (500, serde_json::json!({"error": "idp uninitialized"})),
        };
        let _identity = match idp.authenticate(hr).await {
            Ok(Some(id)) => id,
            Ok(None) => return (401, serde_json::json!({"error": "unauthenticated"})),
            Err(e) => return (503, serde_json::json!({"error": e})),
        };
        // If we reach here in a non-production environment with a dev provider,
        // continue. Otherwise fail closed. For v0, fail closed:
        (503, serde_json::json!({"error": "no machine-auth configured"}))
    }

    async fn h_inorganic_status(
        &self,
        hr: &HttpRequestDto,
        _request_id: &str,
    ) -> (u16, serde_json::Value) {
        let idp = match &self.inorganic_idp {
            Some(i) => i,
            None => return (500, serde_json::json!({"error": "idp uninitialized"})),
        };
        match idp.authenticate(hr).await {
            Ok(None) => (401, serde_json::json!({"error": "unauthenticated"})),
            Err(e) => (503, serde_json::json!({"error": e})),
            Ok(Some(_)) => (503, serde_json::json!({"error": "no machine-auth configured"})),
        }
    }

    // ---- INTERNAL (Elixir callback) ----

    async fn h_internal_outcome(&mut self, hr: &HttpRequestDto) -> (u16, serde_json::Value) {
        let hmac_key = self.hmac_key.clone();
        let Some(provided) = hr.headers.get("x-curatom-hmac") else {
            return (401, serde_json::json!({"error": "missing hmac"}));
        };
        let expected = hmac_hex(hr.body.as_bytes(), &hmac_key);
        if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
            return (401, serde_json::json!({"error": "bad hmac"}));
        }

        let body: serde_json::Value = match serde_json::from_str(&hr.body) {
            Ok(v) => v,
            Err(_) => return (400, serde_json::json!({"error": "invalid_json"})),
        };

        let outcome = Outcome {
            id: curatom_crypto::random_id("out"),
            intent_id: body["intent_id"].as_str().unwrap_or("").into(),
            execution_id: body["job_id"].as_str().unwrap_or("").into(),
            grant_id: body["grant_id"].as_str().unwrap_or("").into(),
            resource: body["resource"].as_str().unwrap_or("").into(),
            operation: parse_op(body["operation"].as_str().unwrap_or("read")),
            ok: body["ok"].as_bool().unwrap_or(false),
            data: body.get("data").cloned(),
            error: body.get("error").and_then(|v| v.as_str()).map(String::from),
            human_summary: body
                .get("human_summary")
                .and_then(|v| v.as_str())
                .map(String::from),
            provider: "hostos".into(),
            mock: body["mock"].as_bool(),
            at: CloudflareClock.now_iso(),
        };

        let k = self.kernel.as_mut().unwrap();
        match k.record_outcome(outcome).await {
            Ok(_) => (200, serde_json::json!({"ok": true})),
            Err(e) => (500, serde_json::json!({"error": e})),
        }
    }

    // ---- helpers ----

    async fn require_owner(
        &self,
        hr: &HttpRequestDto,
    ) -> Result<(), (u16, serde_json::Value)> {
        let idp = self
            .organic_idp
            .as_ref()
            .ok_or((500, serde_json::json!({"error": "idp uninitialized"})))?;
        match idp.authenticate(hr).await {
            Ok(Some(id)) if id.id == self.owner_id => Ok(()),
            Ok(Some(_)) => Err((403, serde_json::json!({"error": "not_owner"}))),
            Ok(None) => Err((401, serde_json::json!({"error": "unauthenticated"}))),
            Err(e) => Err((500, serde_json::json!({"error": e}))),
        }
    }
}

// ---- module-level helpers ----

async fn to_dto(mut req: Request) -> Result<HttpRequestDto> {
    let method = req.method().to_string();
    let url = req.url()?.to_string();

    let mut headers = std::collections::BTreeMap::new();
    for (k, v) in req.headers().entries() {
        headers.insert(k.to_ascii_lowercase(), v);
    }

    let body = if method == "GET" {
        String::new()
    } else {
        req.text().await.unwrap_or_default()
    };

    Ok(HttpRequestDto {
        method,
        url,
        headers,
        body,
    })
}

fn hmac_hex(msg: &[u8], key: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("32-byte key");
    mac.update(msg);
    hex::encode(mac.finalize().into_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut x = 0u8;
    for i in 0..a.len() {
        x |= a[i] ^ b[i];
    }
    x == 0
}

fn op_string(p: Permission) -> String {
    match p {
        Permission::Read => "read".into(),
        Permission::Write => "write".into(),
    }
}

fn parse_op(s: &str) -> Permission {
    match s {
        "write" => Permission::Write,
        _ => Permission::Read,
    }
}

fn human_duration(d: &Duration) -> String {
    match d {
        Duration::SingleUse => "one request".into(),
        Duration::Ttl { seconds } => format!("{seconds} seconds"),
    }
}

// Unused import silencer for symbols only used in match arms above.
#[allow(dead_code)]
fn _keep_imports(_: Approval, _: Intent, _: StateError, _: RealSignVerifier) {}
