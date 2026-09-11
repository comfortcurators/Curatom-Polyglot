//! HMAC-signed job envelope. Contract 2.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AttestationClaims {
    pub v: u8,
    pub job_id: String,
    pub grant_id: String,
    pub requester_id: String,
    pub resource: String,
    pub operation: String,
    pub issued_unix: i64,
    pub expires_unix: i64,
    pub nonce: String,
}

/// Decode CURATOM_HMAC_KEY. Try standard base64, else raw UTF-8.
pub fn hmac_key_bytes(secret: &str) -> Vec<u8> {
    use base64::engine::general_purpose::STANDARD;
    STANDARD
        .decode(secret.trim())
        .unwrap_or_else(|_| secret.as_bytes().to_vec())
}

pub fn sign(claims: &AttestationClaims, key: &[u8]) -> Result<String, String> {
    let payload = serde_json::to_vec(claims).map_err(|e| e.to_string())?;
    let mut mac = HmacSha256::new_from_slice(key).map_err(|e| e.to_string())?;
    mac.update(&payload);
    let sig = mac.finalize().into_bytes();
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&payload),
        URL_SAFE_NO_PAD.encode(sig)
    ))
}

pub fn verify(token: &str, key: &[u8]) -> Result<AttestationClaims, String> {
    let (p, s) = token.split_once('.').ok_or("malformed")?;
    let payload = URL_SAFE_NO_PAD.decode(p).map_err(|_| "bad payload")?;
    let sig = URL_SAFE_NO_PAD.decode(s).map_err(|_| "bad sig")?;
    let mut mac = HmacSha256::new_from_slice(key).map_err(|e| e.to_string())?;
    mac.update(&payload);
    mac.verify_slice(&sig).map_err(|_| "bad signature")?;
    serde_json::from_slice(&payload).map_err(|e| e.to_string())
}

pub fn hmac_hex(msg: &[u8], key: &[u8]) -> String {
    let mut m = HmacSha256::new_from_slice(key).expect("hmac key");
    m.update(msg);
    hex_encode(&m.finalize().into_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut x = 0u8;
    for i in 0..a.len() {
        x |= a[i] ^ b[i];
    }
    x == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims() -> AttestationClaims {
        AttestationClaims {
            v: 1,
            job_id: "job_1".into(),
            grant_id: "grt_1".into(),
            requester_id: "fleet.curatom".into(),
            resource: "hostos.inventory".into(),
            operation: "read".into(),
            issued_unix: 1000,
            expires_unix: 1300,
            nonce: "nonce_1".into(),
        }
    }

    #[test]
    fn roundtrip() {
        let key = hmac_key_bytes("dGVzdC1obWFjLWtleS1jaGFuZ2UtbWUtaW4tcHJvZC0xMjM0NTY=");
        let tok = sign(&claims(), &key).unwrap();
        let back = verify(&tok, &key).unwrap();
        assert_eq!(back, claims());
    }

    #[test]
    fn bad_sig_fails() {
        let key = b"abc";
        let tok = sign(&claims(), key).unwrap();
        assert!(verify(&tok, b"xyz").is_err());
    }

    #[test]
    fn key_bytes_accepts_raw_or_b64() {
        assert_eq!(hmac_key_bytes("not-b64!!"), b"not-b64!!");
        assert_eq!(hmac_key_bytes("YWI="), b"ab");
    }
}
