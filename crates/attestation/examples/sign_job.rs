//! Sign a Contract-2 job envelope the way the Worker does.
//!
//! stdout:
//!   HMAC <hex>
//!   <json body>
//!
//!   CURATOM_HMAC_KEY=... cargo run -p curatom-attestation --example sign_job

use curatom_attestation::{hmac_hex, hmac_key_bytes, sign, AttestationClaims};

fn main() {
    let secret = std::env::var("CURATOM_HMAC_KEY").expect("CURATOM_HMAC_KEY");
    let key = hmac_key_bytes(&secret);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let nonce = std::env::var("SEAM_NONCE").unwrap_or_else(|_| format!("nonce_seam_{now}"));
    let job_id = std::env::var("SEAM_JOB_ID").unwrap_or_else(|_| format!("job_seam_{now}"));

    let claims = AttestationClaims {
        v: 1,
        job_id: job_id.clone(),
        grant_id: "grt_seam".into(),
        requester_id: "fleet.curatom".into(),
        resource: "hostos.inventory".into(),
        operation: "read".into(),
        issued_unix: now,
        expires_unix: now + 300,
        nonce,
    };
    let attestation = sign(&claims, &key).expect("sign");

    let envelope = serde_json::json!({
        "job_id": job_id,
        "approval_id": "appr_seam",
        "intent_id": "int_seam",
        "requester_id": "fleet.curatom",
        "actions": [{
            "resource": "hostos.inventory",
            "operation": "read",
            "attestation": attestation,
        }],
    });
    let body = serde_json::to_vec(&envelope).expect("json");
    let mac = hmac_hex(&body, &key);
    println!("HMAC {mac}");
    print!("{}", String::from_utf8(body).expect("utf8"));
}
