//! Tiny crypto helpers. No Cloudflare.

use hmac::Hmac;
use pbkdf2::pbkdf2;
use sha2::{Digest, Sha256};

pub fn random_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Constant-time comparison. A naive `==` short-circuits on the first
/// mismatched byte, leaking via timing how many leading bytes of a guess
/// were right -- wrong for anything that compares a real credential
/// (password hash, session token) against a presented one.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 16 random bytes, hex-encoded. `getrandom` (wasm: the `js` feature,
/// backed by the platform's real CSPRNG; native: the OS's) is the same
/// randomness source `random_id`'s UUIDs already draw from -- not a new
/// trust assumption for this codebase.
pub fn random_salt_hex() -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("getrandom failed");
    hex::encode(buf)
}

/// A 32-byte random token, hex-encoded -- used as both a session token
/// and (via `sha256_hex` of it) the row key stored server-side. Longer
/// than `random_salt_hex`'s 16 bytes on purpose: this one is a bearer
/// credential by itself, not a salt that only needs to be unique.
pub fn random_token_hex() -> String {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).expect("getrandom failed");
    hex::encode(buf)
}

const PBKDF2_ITERATIONS: u32 = 100_000;

/// PBKDF2-HMAC-SHA256, 100k iterations -- the RustCrypto implementation,
/// not hand-rolled. `salt_hex` must be `random_salt_hex()`'s output (or
/// equivalent); a malformed salt hashes as all-zero bytes rather than
/// panicking, so a corrupt stored row fails verification instead of
/// crashing the request.
pub fn hash_password(password: &str, salt_hex: &str) -> String {
    let salt = hex::decode(salt_hex).unwrap_or_default();
    let mut out = [0u8; 32];
    let _ = pbkdf2::<Hmac<Sha256>>(password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut out);
    hex::encode(out)
}

/// Re-hashes `password` under `salt_hex` and compares to
/// `expected_hash_hex` in constant time.
pub fn verify_password(password: &str, salt_hex: &str, expected_hash_hex: &str) -> bool {
    let computed = hash_password(password, salt_hex);
    constant_time_eq(computed.as_bytes(), expected_hash_hex.as_bytes())
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

/// ISO-8601 `YYYY-MM-DDTHH:MM:SS(.fff)?Z` plus whole seconds.
/// Inverse is on civil days, not on the seconds total.
pub fn iso_plus_secs(iso: &str, secs: i64) -> String {
    let bytes = iso.as_bytes();
    if bytes.len() < 20 || bytes[10] != b'T' || !iso.ends_with('Z') {
        return iso.to_string();
    }
    let y: i64 = iso[0..4].parse().unwrap_or(1970);
    let mo: i64 = iso[5..7].parse().unwrap_or(1);
    let d: i64 = iso[8..10].parse().unwrap_or(1);
    let h: i64 = iso[11..13].parse().unwrap_or(0);
    let mi: i64 = iso[14..16].parse().unwrap_or(0);
    let s: i64 = iso[17..19].parse().unwrap_or(0);

    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;

    let total = days * 86400 + h * 3600 + mi * 60 + s + secs;
    let days2 = total.div_euclid(86400);
    let tod = total.rem_euclid(86400);

    let z = days2 + 719468;
    let era2 = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe2 = z - era2 * 146097;
    let yoe2 = (doe2 - doe2 / 1460 + doe2 / 36524 - doe2 / 146096) / 365;
    let y3 = yoe2 + era2 * 400;
    let doy2 = doe2 - (365 * yoe2 + yoe2 / 4 - yoe2 / 100);
    let mp = (5 * doy2 + 2) / 153;
    let d2 = doy2 - (153 * mp + 2) / 5 + 1;
    let m2 = if mp < 10 { mp + 3 } else { mp - 9 };
    let y4 = if m2 <= 2 { y3 + 1 } else { y3 };

    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y4, m2, d2, hh, mm, ss)
}

pub fn iso_to_unix(iso: &str) -> i64 {
    if iso.len() < 20 || !iso.ends_with('Z') {
        return iso.parse().unwrap_or(0);
    }
    let y: i64 = iso[0..4].parse().unwrap_or(1970);
    let mo: i64 = iso[5..7].parse().unwrap_or(1);
    let d: i64 = iso[8..10].parse().unwrap_or(1);
    let h: i64 = iso[11..13].parse().unwrap_or(0);
    let mi: i64 = iso[14..16].parse().unwrap_or(0);
    let s: i64 = iso[17..19].parse().unwrap_or(0);
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + h * 3600 + mi * 60 + s
}

pub fn iso_diff_secs(a: &str, b: &str) -> i64 {
    iso_to_unix(b) - iso_to_unix(a)
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

    #[test]
    fn iso_plus_one_minute() {
        assert_eq!(
            iso_plus_secs("2026-09-11T12:00:00Z", 60),
            "2026-09-11T12:01:00Z"
        );
    }

    #[test]
    fn iso_plus_88_seconds() {
        assert_eq!(
            iso_plus_secs("2026-09-11T12:00:00Z", 88),
            "2026-09-11T12:01:28Z"
        );
    }

    #[test]
    fn iso_diff_positive() {
        assert_eq!(
            iso_diff_secs("2026-09-11T12:00:00Z", "2026-09-11T12:01:28Z"),
            88
        );
    }

    #[test]
    fn correct_password_verifies() {
        let salt = random_salt_hex();
        let hash = hash_password("correct horse battery staple", &salt);
        assert!(verify_password("correct horse battery staple", &salt, &hash));
    }

    #[test]
    fn wrong_password_fails() {
        let salt = random_salt_hex();
        let hash = hash_password("correct horse battery staple", &salt);
        assert!(!verify_password("wrong password entirely", &salt, &hash));
    }

    #[test]
    fn same_password_different_salt_different_hash() {
        let salt_a = random_salt_hex();
        let salt_b = random_salt_hex();
        assert_ne!(salt_a, salt_b, "two calls must not draw the same salt");
        let hash_a = hash_password("same password", &salt_a);
        let hash_b = hash_password("same password", &salt_b);
        assert_ne!(hash_a, hash_b, "a shared salt would make identical passwords detectable");
    }

    #[test]
    fn random_tokens_are_unique_and_long_enough() {
        let a = random_token_hex();
        let b = random_token_hex();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64, "32 bytes hex-encoded");
    }
}
