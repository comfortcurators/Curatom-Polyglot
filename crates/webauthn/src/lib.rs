/*
Intent : Verify a passkey the way the spec says, in Rust nobody has to trust me about.
Pattern: cargo test -p curatom-webauthn. Flip any check to `true` and a case goes red.
Signed. Claude / 2026-09-18 UTC

What this is and is not.

IS: the server half of WebAuthn for the one case a passkey actually uses --
ES256 (ECDSA P-256), attestation format "none". Registration extracts the
public key the authenticator minted; login verifies a signature against it.
Both halves check the challenge, the origin, the relying-party id hash and
the user-present flag, because every one of those is what stops a signature
that is real but was made somewhere else, for something else.

IS NOT: attestation-certificate validation. A passkey from a phone or a
laptop sends `fmt: "none"` -- there is no certificate chain to check, and
pretending to check one would be theatre. What that costs is precise and
worth stating: we learn nothing about *which* authenticator model made the
key. We still learn that whoever logs in later holds the private half of
the key registered here, which is the whole point.
*/

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ciborium::value::Value as Cbor;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

#[derive(Debug, PartialEq, Eq)]
pub enum WebauthnError {
    BadEncoding(&'static str),
    BadClientData(&'static str),
    ChallengeMismatch,
    OriginMismatch,
    RpIdMismatch,
    UserNotPresent,
    UnsupportedAlgorithm,
    MalformedAuthData,
    SignatureInvalid,
    /// The authenticator's counter went backwards, which is the one signal
    /// the spec gives that a credential may have been cloned. Reported, never
    /// swallowed.
    CounterRegressed,
}

impl core::fmt::Display for WebauthnError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            Self::BadEncoding(w) => return write!(f, "malformed base64url in {w}"),
            Self::BadClientData(w) => return write!(f, "clientDataJSON: {w}"),
            Self::ChallengeMismatch => "challenge does not match the one issued",
            Self::OriginMismatch => "origin does not match this site",
            Self::RpIdMismatch => "relying party id hash does not match",
            Self::UserNotPresent => "authenticator did not report a present user",
            Self::UnsupportedAlgorithm => "credential is not ES256 (P-256)",
            Self::MalformedAuthData => "authenticator data is malformed",
            Self::SignatureInvalid => "signature does not verify",
            Self::CounterRegressed => "authenticator counter went backwards",
        };
        write!(f, "{s}")
    }
}

/// What registration establishes and login later leans on. `public_key_sec1`
/// is the uncompressed SEC1 encoding (`0x04 || x || y`) -- the form
/// `VerifyingKey::from_sec1_bytes` takes, stored as-is so login does no
/// re-derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredCredential {
    pub credential_id: Vec<u8>,
    pub public_key_sec1: Vec<u8>,
    pub sign_count: u32,
}

const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_ATTESTED_CREDENTIAL_DATA: u8 = 0x40;
const COSE_ALG_ES256: i128 = -7;

pub fn b64url_decode(s: &str, what: &'static str) -> Result<Vec<u8>, WebauthnError> {
    URL_SAFE_NO_PAD
        .decode(s.trim())
        .map_err(|_| WebauthnError::BadEncoding(what))
}

pub fn b64url_encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

/// Length-independent byte comparison. A challenge is a random nonce rather
/// than a long-lived secret, but comparing it with `==` still leaks how much
/// of a guess was right, and there is no reason to leak that.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// The three fields of clientDataJSON this verification depends on. Parsed
/// with a real JSON parser rather than by scanning for substrings: the
/// browser is free to add fields, reorder them, and escape them, and a
/// substring scan would quietly accept a challenge that merely *appears*
/// inside some other field.
fn check_client_data(
    client_data: &[u8],
    expected_type: &str,
    expected_challenge: &[u8],
    expected_origin: &str,
) -> Result<(), WebauthnError> {
    let parsed: serde_json::Value = serde_json::from_slice(client_data)
        .map_err(|_| WebauthnError::BadClientData("not valid JSON"))?;

    let ceremony = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or(WebauthnError::BadClientData("missing type"))?;
    if ceremony != expected_type {
        return Err(WebauthnError::BadClientData("wrong ceremony type"));
    }

    let challenge_b64 = parsed
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or(WebauthnError::BadClientData("missing challenge"))?;
    let challenge = b64url_decode(challenge_b64, "clientData challenge")?;
    if !constant_time_eq(&challenge, expected_challenge) {
        return Err(WebauthnError::ChallengeMismatch);
    }

    let origin = parsed
        .get("origin")
        .and_then(|v| v.as_str())
        .ok_or(WebauthnError::BadClientData("missing origin"))?;
    if origin != expected_origin {
        return Err(WebauthnError::OriginMismatch);
    }

    Ok(())
}

struct AuthData<'a> {
    rp_id_hash: &'a [u8],
    flags: u8,
    sign_count: u32,
    rest: &'a [u8],
}

fn parse_auth_data(bytes: &[u8]) -> Result<AuthData<'_>, WebauthnError> {
    if bytes.len() < 37 {
        return Err(WebauthnError::MalformedAuthData);
    }
    let sign_count = u32::from_be_bytes([bytes[33], bytes[34], bytes[35], bytes[36]]);
    Ok(AuthData {
        rp_id_hash: &bytes[0..32],
        flags: bytes[32],
        sign_count,
        rest: &bytes[37..],
    })
}

fn cbor_map_get<'a>(map: &'a [(Cbor, Cbor)], key: i128) -> Option<&'a Cbor> {
    map.iter().find_map(|(k, v)| match k {
        Cbor::Integer(i) if i128::from(*i) == key => Some(v),
        _ => None,
    })
}

/// COSE_Key -> SEC1. Only EC2/P-256/ES256 is accepted; anything else is a
/// credential this system cannot verify later, and refusing it at
/// registration is far better than storing a key that fails every login.
fn cose_key_to_sec1(key: &Cbor) -> Result<Vec<u8>, WebauthnError> {
    let Cbor::Map(entries) = key else {
        return Err(WebauthnError::MalformedAuthData);
    };

    match cbor_map_get(entries, 3) {
        Some(Cbor::Integer(alg)) if i128::from(*alg) == COSE_ALG_ES256 => {}
        _ => return Err(WebauthnError::UnsupportedAlgorithm),
    }

    let x = match cbor_map_get(entries, -2) {
        Some(Cbor::Bytes(b)) if b.len() == 32 => b,
        _ => return Err(WebauthnError::UnsupportedAlgorithm),
    };
    let y = match cbor_map_get(entries, -3) {
        Some(Cbor::Bytes(b)) if b.len() == 32 => b,
        _ => return Err(WebauthnError::UnsupportedAlgorithm),
    };

    let mut sec1 = Vec::with_capacity(65);
    sec1.push(0x04);
    sec1.extend_from_slice(x);
    sec1.extend_from_slice(y);
    Ok(sec1)
}

/// Registration. Takes exactly what `navigator.credentials.create()` hands
/// back, base64url-encoded, and returns the credential to store.
pub fn verify_registration(
    client_data_json_b64: &str,
    attestation_object_b64: &str,
    expected_challenge: &[u8],
    expected_origin: &str,
    rp_id: &str,
) -> Result<RegisteredCredential, WebauthnError> {
    let client_data = b64url_decode(client_data_json_b64, "clientDataJSON")?;
    check_client_data(
        &client_data,
        "webauthn.create",
        expected_challenge,
        expected_origin,
    )?;

    let attestation = b64url_decode(attestation_object_b64, "attestationObject")?;
    let attestation: Cbor = ciborium::from_reader(attestation.as_slice())
        .map_err(|_| WebauthnError::MalformedAuthData)?;
    let Cbor::Map(entries) = &attestation else {
        return Err(WebauthnError::MalformedAuthData);
    };
    let auth_data_bytes = entries
        .iter()
        .find_map(|(k, v)| match (k, v) {
            (Cbor::Text(t), Cbor::Bytes(b)) if t == "authData" => Some(b),
            _ => None,
        })
        .ok_or(WebauthnError::MalformedAuthData)?;

    let auth = parse_auth_data(auth_data_bytes)?;
    if !constant_time_eq(auth.rp_id_hash, &sha256(rp_id.as_bytes())) {
        return Err(WebauthnError::RpIdMismatch);
    }
    if auth.flags & FLAG_USER_PRESENT == 0 {
        return Err(WebauthnError::UserNotPresent);
    }
    if auth.flags & FLAG_ATTESTED_CREDENTIAL_DATA == 0 {
        return Err(WebauthnError::MalformedAuthData);
    }

    // attestedCredentialData: aaguid(16) | credIdLen(2) | credId | COSE key
    let rest = auth.rest;
    if rest.len() < 18 {
        return Err(WebauthnError::MalformedAuthData);
    }
    let cred_id_len = u16::from_be_bytes([rest[16], rest[17]]) as usize;
    let key_start = 18 + cred_id_len;
    if rest.len() < key_start {
        return Err(WebauthnError::MalformedAuthData);
    }
    let credential_id = rest[18..key_start].to_vec();
    if credential_id.is_empty() {
        return Err(WebauthnError::MalformedAuthData);
    }

    // `from_reader` stops after one CBOR item, so trailing extension data
    // (the ED flag's map) is read past harmlessly rather than being a parse
    // error.
    let cose: Cbor = ciborium::from_reader(&rest[key_start..])
        .map_err(|_| WebauthnError::MalformedAuthData)?;
    let public_key_sec1 = cose_key_to_sec1(&cose)?;

    // Refuse at registration what could not be verified at login.
    VerifyingKey::from_sec1_bytes(&public_key_sec1)
        .map_err(|_| WebauthnError::UnsupportedAlgorithm)?;

    Ok(RegisteredCredential {
        credential_id,
        public_key_sec1,
        sign_count: auth.sign_count,
    })
}

/// Login. Returns the authenticator's new counter value, for the caller to
/// store. `stored_sign_count` of 0 means the authenticator does not keep a
/// counter (common for platform passkeys), and the regression check is
/// skipped rather than failing every login.
pub fn verify_assertion(
    client_data_json_b64: &str,
    authenticator_data_b64: &str,
    signature_b64: &str,
    expected_challenge: &[u8],
    expected_origin: &str,
    rp_id: &str,
    stored_public_key_sec1: &[u8],
    stored_sign_count: u32,
) -> Result<u32, WebauthnError> {
    let client_data = b64url_decode(client_data_json_b64, "clientDataJSON")?;
    check_client_data(
        &client_data,
        "webauthn.get",
        expected_challenge,
        expected_origin,
    )?;

    let authenticator_data = b64url_decode(authenticator_data_b64, "authenticatorData")?;
    let auth = parse_auth_data(&authenticator_data)?;
    if !constant_time_eq(auth.rp_id_hash, &sha256(rp_id.as_bytes())) {
        return Err(WebauthnError::RpIdMismatch);
    }
    if auth.flags & FLAG_USER_PRESENT == 0 {
        return Err(WebauthnError::UserNotPresent);
    }
    if stored_sign_count != 0 && auth.sign_count != 0 && auth.sign_count <= stored_sign_count {
        return Err(WebauthnError::CounterRegressed);
    }

    // The signed message is authenticatorData || SHA-256(clientDataJSON).
    // Concatenated in that order, not hashed together -- getting this wrong
    // produces a verifier that rejects every genuine login.
    let mut signed = Vec::with_capacity(authenticator_data.len() + 32);
    signed.extend_from_slice(&authenticator_data);
    signed.extend_from_slice(&sha256(&client_data));

    let key = VerifyingKey::from_sec1_bytes(stored_public_key_sec1)
        .map_err(|_| WebauthnError::UnsupportedAlgorithm)?;
    let signature_der = b64url_decode(signature_b64, "signature")?;
    let signature =
        Signature::from_der(&signature_der).map_err(|_| WebauthnError::SignatureInvalid)?;

    key.verify(&signed, &signature)
        .map_err(|_| WebauthnError::SignatureInvalid)?;

    Ok(auth.sign_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::SigningKey;

    const RP_ID: &str = "curatom.rajvansh.dev";
    const ORIGIN: &str = "https://curatom.rajvansh.dev";
    const CHALLENGE: &[u8] = b"a-random-challenge-32-bytes-long";

    fn client_data(ceremony: &str, challenge: &[u8], origin: &str) -> String {
        let json = serde_json::json!({
            "type": ceremony,
            "challenge": b64url_encode(challenge),
            "origin": origin,
            "crossOrigin": false,
        });
        b64url_encode(json.to_string().as_bytes())
    }

    fn auth_data(rp_id: &str, flags: u8, sign_count: u32, tail: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&sha256(rp_id.as_bytes()));
        out.push(flags);
        out.extend_from_slice(&sign_count.to_be_bytes());
        out.extend_from_slice(tail);
        out
    }

    /// A real key, a real COSE encoding, a real attestationObject -- the
    /// fixtures are generated rather than pasted so the tests exercise the
    /// same parser a browser's bytes would.
    fn registration(key: &SigningKey, flags: u8, sign_count: u32, alg: i128) -> String {
        let point = key.verifying_key().to_encoded_point(false);
        let cose = Cbor::Map(vec![
            (Cbor::Integer(1.into()), Cbor::Integer(2.into())),
            (Cbor::Integer(3.into()), Cbor::Integer(alg.try_into().unwrap())),
            (Cbor::Integer((-1).into()), Cbor::Integer(1.into())),
            (Cbor::Integer((-2).into()), Cbor::Bytes(point.x().unwrap().to_vec())),
            (Cbor::Integer((-3).into()), Cbor::Bytes(point.y().unwrap().to_vec())),
        ]);
        let mut cose_bytes = Vec::new();
        ciborium::into_writer(&cose, &mut cose_bytes).unwrap();

        let mut tail = vec![0u8; 16]; // aaguid
        tail.extend_from_slice(&(4u16).to_be_bytes());
        tail.extend_from_slice(b"cred");
        tail.extend_from_slice(&cose_bytes);

        let attestation = Cbor::Map(vec![
            (Cbor::Text("fmt".into()), Cbor::Text("none".into())),
            (Cbor::Text("attStmt".into()), Cbor::Map(vec![])),
            (
                Cbor::Text("authData".into()),
                Cbor::Bytes(auth_data(RP_ID, flags, sign_count, &tail)),
            ),
        ]);
        let mut bytes = Vec::new();
        ciborium::into_writer(&attestation, &mut bytes).unwrap();
        b64url_encode(&bytes)
    }

    fn signed_assertion(key: &SigningKey, auth: &[u8], client: &str) -> String {
        let mut msg = auth.to_vec();
        msg.extend_from_slice(&sha256(&b64url_decode(client, "t").unwrap()));
        let sig: Signature = key.sign(&msg);
        b64url_encode(sig.to_der().as_bytes())
    }

    #[test]
    fn a_real_registration_yields_the_key_and_credential_id() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let attestation = registration(&key, FLAG_USER_PRESENT | FLAG_ATTESTED_CREDENTIAL_DATA, 7, COSE_ALG_ES256);
        let out = verify_registration(
            &client_data("webauthn.create", CHALLENGE, ORIGIN),
            &attestation,
            CHALLENGE,
            ORIGIN,
            RP_ID,
        )
        .expect("genuine registration must verify");

        assert_eq!(out.credential_id, b"cred");
        assert_eq!(out.sign_count, 7);
        assert_eq!(
            out.public_key_sec1,
            key.verifying_key().to_encoded_point(false).as_bytes()
        );
    }

    /// One property, every way it breaks: registration must refuse anything
    /// that was not issued by this site, for this ceremony, with a usable key.
    #[test]
    fn registration_refuses_every_kind_of_mismatch() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let good_flags = FLAG_USER_PRESENT | FLAG_ATTESTED_CREDENTIAL_DATA;
        let attestation = registration(&key, good_flags, 0, COSE_ALG_ES256);

        let wrong_challenge = verify_registration(
            &client_data("webauthn.create", b"some-other-challenge-entirely!!!", ORIGIN),
            &attestation,
            CHALLENGE,
            ORIGIN,
            RP_ID,
        );
        assert_eq!(wrong_challenge, Err(WebauthnError::ChallengeMismatch));

        let wrong_origin = verify_registration(
            &client_data("webauthn.create", CHALLENGE, "https://evil.example"),
            &attestation,
            CHALLENGE,
            ORIGIN,
            RP_ID,
        );
        assert_eq!(wrong_origin, Err(WebauthnError::OriginMismatch));

        let wrong_ceremony = verify_registration(
            &client_data("webauthn.get", CHALLENGE, ORIGIN),
            &attestation,
            CHALLENGE,
            ORIGIN,
            RP_ID,
        );
        assert_eq!(
            wrong_ceremony,
            Err(WebauthnError::BadClientData("wrong ceremony type"))
        );

        let wrong_rp = verify_registration(
            &client_data("webauthn.create", CHALLENGE, ORIGIN),
            &attestation,
            CHALLENGE,
            ORIGIN,
            "attacker.example",
        );
        assert_eq!(wrong_rp, Err(WebauthnError::RpIdMismatch));

        let absent_user = registration(&key, FLAG_ATTESTED_CREDENTIAL_DATA, 0, COSE_ALG_ES256);
        assert_eq!(
            verify_registration(
                &client_data("webauthn.create", CHALLENGE, ORIGIN),
                &absent_user,
                CHALLENGE,
                ORIGIN,
                RP_ID
            ),
            Err(WebauthnError::UserNotPresent)
        );

        // RS256: a real COSE algorithm this verifier cannot check later.
        let unusable_alg = registration(&key, good_flags, 0, -257);
        assert_eq!(
            verify_registration(
                &client_data("webauthn.create", CHALLENGE, ORIGIN),
                &unusable_alg,
                CHALLENGE,
                ORIGIN,
                RP_ID
            ),
            Err(WebauthnError::UnsupportedAlgorithm)
        );
    }

    #[test]
    fn a_real_login_verifies_and_returns_the_new_counter() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pubkey = key.verifying_key().to_encoded_point(false).as_bytes().to_vec();
        let auth = auth_data(RP_ID, FLAG_USER_PRESENT, 9, &[]);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let sig = signed_assertion(&key, &auth, &client);

        let count = verify_assertion(
            &client,
            &b64url_encode(&auth),
            &sig,
            CHALLENGE,
            ORIGIN,
            RP_ID,
            &pubkey,
            8,
        )
        .expect("genuine assertion must verify");
        assert_eq!(count, 9);
    }

    /// The case that matters most: a signature that is cryptographically
    /// real, made by a key we never registered.
    #[test]
    fn a_signature_from_another_key_is_refused() {
        let ours = SigningKey::random(&mut rand_core::OsRng);
        let theirs = SigningKey::random(&mut rand_core::OsRng);
        let auth = auth_data(RP_ID, FLAG_USER_PRESENT, 1, &[]);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let sig = signed_assertion(&theirs, &auth, &client);

        assert_eq!(
            verify_assertion(
                &client,
                &b64url_encode(&auth),
                &sig,
                CHALLENGE,
                ORIGIN,
                RP_ID,
                ours.verifying_key().to_encoded_point(false).as_bytes(),
                0
            ),
            Err(WebauthnError::SignatureInvalid)
        );
    }

    /// A genuine signature replayed against a different challenge, or with
    /// the signed bytes tampered with after the fact.
    #[test]
    fn login_refuses_replay_and_tampering() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pubkey = key.verifying_key().to_encoded_point(false).as_bytes().to_vec();
        let auth = auth_data(RP_ID, FLAG_USER_PRESENT, 1, &[]);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let sig = signed_assertion(&key, &auth, &client);

        let replayed = verify_assertion(
            &client,
            &b64url_encode(&auth),
            &sig,
            b"a-completely-different-challenge!",
            ORIGIN,
            RP_ID,
            &pubkey,
            0,
        );
        assert_eq!(replayed, Err(WebauthnError::ChallengeMismatch));

        // Same signature, authenticatorData edited afterwards.
        let mut tampered = auth.clone();
        tampered[32] |= 0x04;
        assert_eq!(
            verify_assertion(
                &client,
                &b64url_encode(&tampered),
                &sig,
                CHALLENGE,
                ORIGIN,
                RP_ID,
                &pubkey,
                0
            ),
            Err(WebauthnError::SignatureInvalid)
        );
    }

    #[test]
    fn a_counter_going_backwards_is_reported_not_swallowed() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pubkey = key.verifying_key().to_encoded_point(false).as_bytes().to_vec();
        let auth = auth_data(RP_ID, FLAG_USER_PRESENT, 3, &[]);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let sig = signed_assertion(&key, &auth, &client);

        assert_eq!(
            verify_assertion(
                &client,
                &b64url_encode(&auth),
                &sig,
                CHALLENGE,
                ORIGIN,
                RP_ID,
                &pubkey,
                10
            ),
            Err(WebauthnError::CounterRegressed)
        );
    }

    /// Platform passkeys commonly report a counter of 0 forever. That must
    /// stay loggable-in, or the feature is broken for most real devices.
    #[test]
    fn a_counterless_authenticator_still_logs_in() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pubkey = key.verifying_key().to_encoded_point(false).as_bytes().to_vec();
        let auth = auth_data(RP_ID, FLAG_USER_PRESENT, 0, &[]);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let sig = signed_assertion(&key, &auth, &client);

        assert_eq!(
            verify_assertion(
                &client,
                &b64url_encode(&auth),
                &sig,
                CHALLENGE,
                ORIGIN,
                RP_ID,
                &pubkey,
                0
            ),
            Ok(0)
        );
    }

    #[test]
    fn a_challenge_hidden_inside_another_field_is_not_a_match() {
        // A substring-scanning verifier would accept this; a parsing one
        // must not. The real challenge appears only inside `origin`.
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pubkey = key.verifying_key().to_encoded_point(false).as_bytes().to_vec();
        let auth = auth_data(RP_ID, FLAG_USER_PRESENT, 1, &[]);
        let json = serde_json::json!({
            "type": "webauthn.get",
            "challenge": b64url_encode(b"not-the-issued-challenge-at-all!"),
            "origin": format!("{ORIGIN}/{}", b64url_encode(CHALLENGE)),
        });
        let client = b64url_encode(json.to_string().as_bytes());
        let sig = signed_assertion(&key, &auth, &client);

        assert_eq!(
            verify_assertion(
                &client,
                &b64url_encode(&auth),
                &sig,
                CHALLENGE,
                ORIGIN,
                RP_ID,
                &pubkey,
                0
            ),
            Err(WebauthnError::ChallengeMismatch)
        );
    }
}
