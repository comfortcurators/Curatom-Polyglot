//! Calls the browser's own WebAuthn API -- a W3C standard, reached here
//! through web-sys's typed bindings for it. No JavaScript is written; this
//! is Rust calling a browser function the same way it calls `fetch`.
//!
//! What actually decides whether a passkey is accepted lives entirely on
//! the server (`curatom-webauthn`, `workers/api/src/passkey.rs`). This
//! file's only job is: build the options the browser wants, hand the
//! result back as base64url strings the server already knows how to read.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AuthenticatorAssertionResponse, AuthenticatorAttestationResponse,
    AuthenticatorSelectionCriteria, CredentialCreationOptions, CredentialRequestOptions,
    PublicKeyCredential, PublicKeyCredentialCreationOptions, PublicKeyCredentialParameters,
    PublicKeyCredentialRequestOptions, PublicKeyCredentialRpEntity, PublicKeyCredentialType,
    PublicKeyCredentialUserEntity, UserVerificationRequirement,
};

fn b64url_decode(s: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| format!("bad base64url: {e}"))
}

fn b64url_encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn to_uint8array(bytes: &[u8]) -> Uint8Array {
    let arr = Uint8Array::new_with_length(bytes.len() as u32);
    arr.copy_from(bytes);
    arr
}

fn array_buffer_to_b64url(buf: &JsValue) -> String {
    let arr = Uint8Array::new(buf);
    let mut bytes = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut bytes);
    b64url_encode(&bytes)
}

fn navigator_credentials() -> Result<web_sys::CredentialsContainer, String> {
    Ok(web_sys::window()
        .ok_or("no window")?
        .navigator()
        .credentials())
}

/// True when the browser exposes `navigator.credentials`, the baseline this
/// whole API needs. Missing on very old browsers and on plain HTTP origins
/// (WebAuthn requires a secure context) -- the caller uses this to decide
/// whether to show the passkey button at all.
pub fn supported() -> bool {
    navigator_credentials().is_ok()
}

pub struct RegistrationResult {
    pub client_data_json: String,
    pub attestation_object: String,
}

/// Runs `navigator.credentials.create()`. `challenge` and `user_id` are the
/// base64url strings the server issued; `rp_id` and `rp_name` name this site.
pub async fn create_passkey(
    challenge: &str,
    rp_id: &str,
    rp_name: &str,
    user_id: &str,
    username: &str,
) -> Result<RegistrationResult, String> {
    let challenge_bytes = b64url_decode(challenge)?;
    let user_id_bytes = b64url_decode(user_id)?;

    let rp = PublicKeyCredentialRpEntity::new(rp_name);
    rp.set_id(rp_id);

    let user = PublicKeyCredentialUserEntity::new(
        username,
        username,
        &to_uint8array(&user_id_bytes).into(),
    );

    // ES256 only -- the one algorithm `curatom-webauthn` verifies. A
    // credential minted under any other algorithm would register
    // successfully here and then never be able to log in, which is a
    // worse failure than refusing it at browser level never happening;
    // this is simply what's offered, not a refusal.
    let es256 = PublicKeyCredentialParameters::new(-7, PublicKeyCredentialType::PublicKey);

    let opts = PublicKeyCredentialCreationOptions::new(
        &to_uint8array(&challenge_bytes),
        &Array::of1(&es256),
        &rp,
        &user,
    );
    opts.set_timeout(120_000);

    let selection = AuthenticatorSelectionCriteria::new();
    selection.set_user_verification(UserVerificationRequirement::Preferred);
    opts.set_authenticator_selection(&selection);

    let creation_opts = CredentialCreationOptions::new();
    creation_opts.set_public_key(&opts);

    let credentials = navigator_credentials()?;
    let promise = credentials
        .create_with_options(&creation_opts)
        .map_err(|e| format!("create() rejected: {e:?}"))?;
    let cred = JsFuture::from(promise)
        .await
        .map_err(|e| format!("passkey creation failed or was cancelled: {e:?}"))?;
    let cred: PublicKeyCredential = cred.dyn_into().map_err(|_| "not a PublicKeyCredential")?;

    let response: AuthenticatorAttestationResponse = cred
        .response()
        .dyn_into()
        .map_err(|_| "not an AuthenticatorAttestationResponse")?;

    Ok(RegistrationResult {
        client_data_json: array_buffer_to_b64url(&response.client_data_json()),
        attestation_object: array_buffer_to_b64url(&response.attestation_object()),
    })
}

pub struct AssertionResult {
    pub credential_id: String,
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
}

/// Runs `navigator.credentials.get()`. No `user_id` on purpose -- the
/// browser offers whichever passkey it already holds for `rp_id`, which is
/// the entire point of signing in with one: nothing is typed first.
pub async fn get_passkey(challenge: &str, rp_id: &str) -> Result<AssertionResult, String> {
    let challenge_bytes = b64url_decode(challenge)?;

    let opts = PublicKeyCredentialRequestOptions::new(&to_uint8array(&challenge_bytes));
    opts.set_rp_id(rp_id);
    opts.set_timeout(120_000);
    opts.set_user_verification(UserVerificationRequirement::Preferred);

    let request_opts = CredentialRequestOptions::new();
    request_opts.set_public_key(&opts);

    let credentials = navigator_credentials()?;
    let promise = credentials
        .get_with_options(&request_opts)
        .map_err(|e| format!("get() rejected: {e:?}"))?;
    let cred = JsFuture::from(promise)
        .await
        .map_err(|e| format!("no passkey available or the user cancelled: {e:?}"))?;
    let cred: PublicKeyCredential = cred.dyn_into().map_err(|_| "not a PublicKeyCredential")?;

    let credential_id = {
        // `.raw_id()` is the bytes the server keyed the row on; `.id()`
        // would be the base64 the browser chose to render it as, which is
        // not guaranteed to be base64url.
        let raw: JsValue = Reflect::get(&cred, &JsValue::from_str("rawId"))
            .map_err(|_| "credential missing rawId")?;
        array_buffer_to_b64url(&raw)
    };

    let response: AuthenticatorAssertionResponse = cred
        .response()
        .dyn_into()
        .map_err(|_| "not an AuthenticatorAssertionResponse")?;

    Ok(AssertionResult {
        credential_id,
        client_data_json: array_buffer_to_b64url(&response.client_data_json()),
        authenticator_data: array_buffer_to_b64url(&response.authenticator_data()),
        signature: array_buffer_to_b64url(&response.signature()),
    })
}
