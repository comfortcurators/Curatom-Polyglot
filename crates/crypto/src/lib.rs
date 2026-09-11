//! Tiny crypto helpers. No Cloudflare.

use sha2::{Digest, Sha256};

pub fn random_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
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
}
