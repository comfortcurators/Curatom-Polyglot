//! Tiny crypto helpers. No Cloudflare.

use sha2::{Digest, Sha256};

pub fn random_id(prefix: &str) -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("entropy");
    format!("{prefix}_{}", hex::encode(buf))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub fn request_digest(requester: &str, resources: &[String], reason: &str) -> String {
    sha256_hex(format!("{requester}|{}|{reason}", resources.join(",")).as_bytes())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_iso() -> String {
    // RFC3339-ish, second precision. Kernel compares unix integers, not this.
    let secs = now_unix();
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_prefixed() {
        let id = random_id("job");
        assert!(id.starts_with("job_"));
        assert_eq!(id.len(), 4 + 32);
    }
}
