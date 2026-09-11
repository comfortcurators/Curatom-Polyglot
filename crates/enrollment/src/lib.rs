use curatom_crypto::{random_id, sha256_hex};
use curatom_ports::{ArtifactStore, Clock};
use curatom_protocol::{ArtifactKind, EnrollmentSession, OwnerRecord};

const ENROLLMENT_TTL_SECS: i64 = 15 * 60;

pub struct EnrollmentFlow<C: Clock> {
    clock: C,
}

impl<C: Clock> EnrollmentFlow<C> {
    pub fn new(clock: C) -> Self {
        Self { clock }
    }

    pub fn start(&self, owner_id: &str) -> EnrollmentSession {
        let now = self.clock.now_iso();
        let expires = iso_plus_secs(&now, ENROLLMENT_TTL_SECS);
        EnrollmentSession {
            session_id: random_id("enr"),
            owner_id: owner_id.to_string(),
            sign1_digest: None,
            sign1_ref: None,
            sign2_digest: None,
            sign2_ref: None,
            created_at: now,
            expires_at: expires,
        }
    }

    /// Store a signature image. Returns (digest, artifact_ref_id).
    pub async fn capture_signature<A: ArtifactStore>(
        &self,
        artifacts: &A,
        owner_id: &str,
        kind: ArtifactKind,
        bytes: &[u8],
    ) -> Result<(String, String), String> {
        if bytes.is_empty() {
            return Err("empty_signature".into());
        }
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("signature_too_large".into());
        }
        let digest = sha256_hex(bytes);
        let ref_id = format!("{}:{}:{}", owner_id, kind_str(kind), digest);
        artifacts.put(&ref_id, bytes).await?;
        Ok((digest, ref_id))
    }

    pub fn complete(
        &self,
        session: &EnrollmentSession,
        owner_id: &str,
        display_name: &str,
    ) -> Result<OwnerRecord, String> {
        if session.owner_id != owner_id {
            return Err("owner_mismatch".into());
        }
        let now = self.clock.now_iso();
        if session.is_expired(&now) {
            return Err("enrollment_expired".into());
        }
        let s1d = session.sign1_digest.clone().ok_or("sign1_missing")?;
        let s1r = session.sign1_ref.clone().ok_or("sign1_missing")?;
        let s2d = session.sign2_digest.clone().ok_or("sign2_missing")?;
        let s2r = session.sign2_ref.clone().ok_or("sign2_missing")?;
        Ok(OwnerRecord {
            owner_id: owner_id.to_string(),
            display_name: display_name.to_string(),
            sign1_digest: s1d,
            sign1_ref: s1r,
            sign2_digest: s2d,
            sign2_ref: s2r,
            enrolled_at: now,
        })
    }
}

fn kind_str(k: ArtifactKind) -> &'static str {
    match k {
        ArtifactKind::Organic => "organic",
        ArtifactKind::Inorganic => "inorganic",
        ArtifactKind::Signature1 => "signature1",
        ArtifactKind::Signature2 => "signature2",
        ArtifactKind::ApprovalSignature => "approval_signature",
    }
}

/// ISO-8601 arithmetic for `YYYY-MM-DDTHH:MM:SS(.ffffff)?Z`.
fn iso_plus_secs(iso: &str, secs: i64) -> String {
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

pub fn decode_image_b64(b64: &str) -> Result<Vec<u8>, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.decode(b64.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use curatom_protocol::EnrollmentSession;

    #[test]
    fn iso_plus_one_minute() {
        assert_eq!(
            iso_plus_secs("2026-09-11T12:00:00Z", 60),
            "2026-09-11T12:01:00Z"
        );
    }

    #[test]
    fn iso_plus_one_day_wraps() {
        assert_eq!(
            iso_plus_secs("2026-09-11T23:59:30Z", 60),
            "2026-09-12T00:00:30Z"
        );
    }

    #[test]
    fn expired_check_lexicographic() {
        let s = EnrollmentSession {
            session_id: "x".into(),
            owner_id: "o".into(),
            sign1_digest: None,
            sign1_ref: None,
            sign2_digest: None,
            sign2_ref: None,
            created_at: "2026-09-11T12:00:00Z".into(),
            expires_at: "2026-09-11T12:15:00Z".into(),
        };
        assert!(!s.is_expired("2026-09-11T12:14:59Z"));
        assert!(s.is_expired("2026-09-11T12:15:01Z"));
    }
}
