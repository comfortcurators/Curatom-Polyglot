//! SIGN2. Dev verifier only. Production signing is deferred.

use async_trait::async_trait;

#[async_trait]
pub trait SignVerifier: Send + Sync {
    async fn verify(&self, kind: &str, owner_id: &str, signature: &str, payload: &str) -> Result<bool, String>;
}

pub struct DevSignVerifier;

impl DevSignVerifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DevSignVerifier {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl SignVerifier for DevSignVerifier {
    async fn verify(&self, kind: &str, _owner_id: &str, signature: &str, _payload: &str) -> Result<bool, String> {
        // Dev SIGN2. Production: hardware-backed / passkey. Not this.
        Ok(kind == "sign2" && signature == "sign2:ok")
    }
}
