//! Same-origin fetch client for curatom-kernel's `/organic/*` API.
//! curatom-kernel serves this dashboard and the API from one hostname --
//! one login, one bookmark, no CORS -- so every path here is relative.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ApiError(pub String);

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

type ApiResult<T> = Result<T, ApiError>;

async fn get<T: for<'a> Deserialize<'a>>(path: &str) -> ApiResult<T> {
    let resp = Request::get(path)
        .send()
        .await
        .map_err(|e| ApiError(format!("{path}: {e}")))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status != 200 {
        return Err(ApiError(format!("{path}: {status} {text}")));
    }
    serde_json::from_str(&text).map_err(|e| ApiError(format!("{path}: bad json: {e}")))
}

async fn post<B: Serialize, T: for<'a> Deserialize<'a>>(
    path: &str,
    body: &B,
    want: u16,
) -> ApiResult<T> {
    let resp = Request::post(path)
        .header("content-type", "application/json")
        .json(body)
        .map_err(|e| ApiError(format!("{path}: {e}")))?
        .send()
        .await
        .map_err(|e| ApiError(format!("{path}: {e}")))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status != want {
        return Err(ApiError(format!("{path}: {status} {text}")));
    }
    serde_json::from_str(&text).map_err(|e| ApiError(format!("{path}: bad json: {e}")))
}

async fn post_empty(path: &str, want: u16) -> ApiResult<()> {
    let resp = Request::post(path)
        .header("content-type", "application/json")
        .body("{}")
        .map_err(|e| ApiError(format!("{path}: {e}")))?
        .send()
        .await
        .map_err(|e| ApiError(format!("{path}: {e}")))?;
    let status = resp.status();
    if status != want {
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError(format!("{path}: {status} {text}")));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct Me {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub display_name: String,
    #[allow(dead_code)]
    pub mode: String,
    pub enrolled: bool,
    #[allow(dead_code)]
    pub enrolled_at: Option<String>,
}

pub async fn me() -> ApiResult<Me> {
    get("/organic/me").await
}

#[derive(Serialize)]
struct ClaimBody<'a> {
    display_name: Option<&'a str>,
}

pub async fn claim() -> ApiResult<serde_json::Value> {
    post("/organic/claim", &ClaimBody { display_name: None }, 201).await
}

#[derive(Debug, Clone, Deserialize)]
pub struct Knock {
    pub id: String,
    pub name: String,
    pub reason: String,
    pub resources: Vec<String>,
    pub permissions: Vec<String>,
    pub seconds_remaining: f64,
}

pub async fn list_knocks() -> ApiResult<Vec<Knock>> {
    get("/organic/knocks").await
}

#[derive(Serialize)]
struct ApproveBody<'a> {
    turnstile_token: &'a str,
}

pub async fn approve_knock(id: &str, turnstile_token: &str) -> ApiResult<()> {
    let path = format!("/organic/knocks/{id}/approve");
    let _: serde_json::Value = post(&path, &ApproveBody { turnstile_token }, 200).await?;
    Ok(())
}

pub async fn refuse_knock(id: &str) -> ApiResult<()> {
    post_empty(&format!("/organic/knocks/{id}/refuse"), 200).await
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActivityEvent {
    pub kind: String,
    pub at: String,
}

pub async fn activity() -> ApiResult<Vec<ActivityEvent>> {
    get("/organic/activity").await
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyInfo {
    pub token: String,
    pub label: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub activity_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyLogEntry {
    pub kind: String,
    pub at: String,
    pub name: Option<String>,
    pub reason: Option<String>,
}

pub async fn list_keys() -> ApiResult<Vec<KeyInfo>> {
    get("/organic/keys").await
}

#[derive(Serialize)]
struct CreateKeyBody<'a> {
    label: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedKey {
    pub file_text: String,
}

pub async fn create_key(label: &str) -> ApiResult<CreatedKey> {
    post("/organic/keys", &CreateKeyBody { label }, 201).await
}

pub async fn revoke_key(token: &str) -> ApiResult<()> {
    post_empty(
        &format!(
            "/organic/keys/{}/revoke",
            js_sys::encode_uri_component(token)
        ),
        200,
    )
    .await
}

pub async fn key_log(token: &str) -> ApiResult<Vec<KeyLogEntry>> {
    get(&format!(
        "/organic/keys/{}/log",
        js_sys::encode_uri_component(token)
    ))
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct BillboardEntry {
    pub id: String,
    pub key_hash: String,
    pub key_label: String,
    pub round: u64,
    pub kind: String,
    pub body_ref: String,
    pub knock_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BillboardResponse {
    entries: Vec<BillboardEntry>,
}

pub async fn billboard(kind: Option<&str>, limit: u32) -> ApiResult<Vec<BillboardEntry>> {
    let mut path = format!("/organic/billboard?limit={limit}");
    if let Some(k) = kind {
        path.push_str(&format!("&kind={k}"));
    }
    let resp: BillboardResponse = get(&path).await?;
    Ok(resp.entries)
}

#[derive(Debug, Clone, Deserialize)]
struct BlobResponse {
    body: String,
}

pub async fn billboard_blob(reference: &str) -> ApiResult<String> {
    let resp: BlobResponse = get(&format!(
        "/organic/billboard/blob?ref={}",
        js_sys::encode_uri_component(reference)
    ))
    .await?;
    Ok(resp.body)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValhallaSession {
    pub sandbox_id: String,
    pub label: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub entry_count: u64,
    pub alive: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ValhallaResponse {
    sessions: Vec<ValhallaSession>,
}

pub async fn valhalla_sessions() -> ApiResult<Vec<ValhallaSession>> {
    let resp: ValhallaResponse = get("/organic/valhalla/sessions").await?;
    Ok(resp.sessions)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Frozen {
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
struct FrozenResponse {
    frozen: Vec<Frozen>,
}

pub async fn frozen_list() -> ApiResult<Vec<Frozen>> {
    let resp: FrozenResponse = get("/organic/frozen").await?;
    Ok(resp.frozen)
}
