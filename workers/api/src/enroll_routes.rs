//! Enrollment routes. Sessions live in DO memory only.

use curatom_enrollment::{decode_image_b64, EnrollmentFlow};
use curatom_ports::Clock;
use curatom_protocol::{ArtifactKind, EnrollmentSession, HttpRequestDto};
use curatom_substrate_cloudflare::{CloudflareClock, R2ArtifactStore};
use serde_json::{json, Value};

use crate::{CuratomKernel, Reply};

impl CuratomKernel {
    pub(crate) async fn h_enroll_start(&self, hr: &HttpRequestDto) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        if self.kernel.borrow().as_ref().unwrap().has_owner() {
            return (409, json!({ "error": "owner_already_enrolled" }));
        }
        let session = EnrollmentFlow::new(CloudflareClock).start(&self.owner_id);
        let body = json!({
            "session_id": session.session_id,
            "expires_at": session.expires_at,
        });
        self.sessions
            .borrow_mut()
            .insert(session.session_id.clone(), session);
        (201, body)
    }

    pub(crate) async fn h_enroll_sign(
        &self,
        hr: &HttpRequestDto,
        which: ArtifactKind,
    ) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        let Ok(body) = serde_json::from_str::<Value>(&hr.body) else {
            return (400, json!({ "error": "invalid_json" }));
        };
        let Some(session_id) = body.get("session_id").and_then(|v| v.as_str()) else {
            return (400, json!({ "error": "missing_session_id" }));
        };
        let Some(image_b64) = body.get("image_b64").and_then(|v| v.as_str()) else {
            return (400, json!({ "error": "missing_image_b64" }));
        };
        let bytes = match decode_image_b64(image_b64) {
            Ok(b) => b,
            Err(e) => return (400, json!({ "error": e })),
        };
        let bucket = match self.bucket.clone() {
            Some(b) => b,
            None => return (500, json!({ "error": "CURATOM_ARTIFACTS bucket missing" })),
        };
        let artifacts = R2ArtifactStore::new(bucket, self.owner_id.clone());
        let flow = EnrollmentFlow::new(CloudflareClock);
        let now = CloudflareClock.now_iso();

        let mut sessions = self.sessions.borrow_mut();
        let Some(session) = sessions.get_mut(session_id) else {
            return (404, json!({ "error": "unknown_session" }));
        };
        if session.owner_id != self.owner_id {
            return (403, json!({ "error": "owner_mismatch" }));
        }
        if session.is_expired(&now) {
            return (410, json!({ "error": "enrollment_expired" }));
        }
        drop(sessions);

        let (digest, ref_id) = match flow
            .capture_signature(&artifacts, &self.owner_id, which, &bytes)
            .await
        {
            Ok(x) => x,
            Err(e) => return (400, json!({ "error": e })),
        };

        let mut sessions = self.sessions.borrow_mut();
        let Some(session) = sessions.get_mut(session_id) else {
            return (404, json!({ "error": "unknown_session" }));
        };
        match which {
            ArtifactKind::Signature1 => {
                session.sign1_digest = Some(digest.clone());
                session.sign1_ref = Some(ref_id.clone());
            }
            ArtifactKind::Signature2 => {
                session.sign2_digest = Some(digest.clone());
                session.sign2_ref = Some(ref_id.clone());
            }
            _ => return (400, json!({ "error": "bad_kind" })),
        }
        let complete = session.is_complete();
        (200, json!({ "digest": digest, "ref": ref_id, "complete": complete }))
    }

    pub(crate) async fn h_enroll_complete(&self, hr: &HttpRequestDto) -> Reply {
        if let Err(r) = self.owner(hr).await {
            return r;
        }
        let Ok(body) = serde_json::from_str::<Value>(&hr.body) else {
            return (400, json!({ "error": "invalid_json" }));
        };
        let Some(session_id) = body.get("session_id").and_then(|v| v.as_str()) else {
            return (400, json!({ "error": "missing_session_id" }));
        };
        let display_name = body
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Rajvansh");

        let session: EnrollmentSession = {
            let sessions = self.sessions.borrow();
            match sessions.get(session_id) {
                Some(s) => s.clone(),
                None => return (404, json!({ "error": "unknown_session" })),
            }
        };

        let flow = EnrollmentFlow::new(CloudflareClock);
        let record = match flow.complete(&session, &self.owner_id, display_name) {
            Ok(r) => r,
            Err(e) => return (400, json!({ "error": e })),
        };

        let mut k = self.kernel.borrow_mut();
        match k.as_mut().unwrap().enroll_owner(record.clone()).await {
            Ok(r) => {
                self.sessions.borrow_mut().remove(session_id);
                (
                    201,
                    json!({
                        "owner_id": r.owner_id,
                        "enrolled_at": r.enrolled_at,
                    }),
                )
            }
            Err(e) => (409, json!({ "error": e })),
        }
    }
}
