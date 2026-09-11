//! Per-key working memory. Token is checked in the kernel fetch path first.

use curatom_crypto::{random_id, sha256_hex};
use serde::{Deserialize, Serialize};
use worker::*;

fn now_iso() -> String {
    js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub kind: String,
    pub body: String,
    pub at: String,
    pub knock_id: Option<String>,
    pub round: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchState {
    pub session_id: String,
    pub key_hash: String,
    pub key_label: String,
    pub opened_at: String,
    pub seq: i64,
    pub round: i64,
    pub notes: Vec<Note>,
}

impl ScratchState {
    fn fresh(key_hash: &str, key_label: &str) -> Self {
        Self {
            session_id: random_id("session"),
            key_hash: key_hash.to_string(),
            key_label: key_label.to_string(),
            opened_at: now_iso(),
            seq: 0,
            round: 1,
            notes: Vec::new(),
        }
    }
}

#[durable_object]
pub struct Scratchpad {
    state: State,
    env: Env,
}

impl DurableObject for Scratchpad {
    fn new(state: State, env: Env) -> Self {
        console_error_panic_hook::set_once();
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path().to_string();
        let params: std::collections::HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned().replace('+', " ")))
            .collect();

        let token = match params.get("token") {
            Some(t) if !t.is_empty() => t.clone(),
            _ => return json_err(400, "missing_token"),
        };
        let key_hash = sha256_hex(token.as_bytes());
        let label_param = params.get("label").cloned().unwrap_or_default();
        let action = path.trim_start_matches("/scratch/").to_string();

        match action.as_str() {
            "open" => self.handle_open(&key_hash, &label_param).await,
            "write" => {
                let kind = params.get("kind").cloned().unwrap_or_else(|| "note".into());
                let body = params.get("body").cloned().unwrap_or_default();
                let knock_id = params.get("knock_id").cloned();
                let round_advance = params.get("new_round").map(|v| v == "1").unwrap_or(false);
                self.handle_write(&key_hash, kind, body, knock_id, round_advance)
                    .await
            }
            "notes" => self.handle_notes(&key_hash).await,
            "close" => self.handle_close(&key_hash).await,
            "history" => self.handle_history(&key_hash).await,
            _ => json_err(404, "unknown_scratch_action"),
        }
    }
}

impl Scratchpad {
    async fn load_or_init(&self, key_hash: &str, label: &str) -> Result<ScratchState> {
        let st = self.state.storage();
        match st.get::<ScratchState>("current").await {
            Ok(Some(s)) => Ok(s),
            _ => {
                let s = ScratchState::fresh(key_hash, label);
                st.put("current", &s).await?;
                Ok(s)
            }
        }
    }

    async fn save(&self, s: &ScratchState) -> Result<()> {
        self.state.storage().put("current", s).await
    }

    async fn handle_open(&self, key_hash: &str, label: &str) -> Result<Response> {
        let s = self.load_or_init(key_hash, label).await?;
        Response::from_json(&serde_json::json!({
            "session_id": s.session_id,
            "key_hash": &s.key_hash[..s.key_hash.len().min(16)],
            "opened_at": s.opened_at,
            "round": s.round,
            "note_count": s.notes.len(),
        }))
    }

    async fn handle_write(
        &self,
        key_hash: &str,
        kind: String,
        body: String,
        knock_id: Option<String>,
        round_advance: bool,
    ) -> Result<Response> {
        if body.is_empty() {
            return json_err(400, "empty_body");
        }
        if body.len() > 16_000 {
            return json_err(413, "body_too_large");
        }
        let mut s = self.load_or_init(key_hash, "").await?;
        if round_advance {
            s.round += 1;
        }
        s.seq += 1;
        let note = Note {
            id: random_id("note"),
            kind: kind.clone(),
            body: body.clone(),
            at: now_iso(),
            knock_id: knock_id.clone(),
            round: s.round,
        };
        s.notes.push(note.clone());
        if let Err(e) = self.mirror_note(&s, &note).await {
            console_log!("mirror_failed: {}", e);
        }
        self.save(&s).await?;
        Response::from_json(&serde_json::json!({
            "note_id": note.id,
            "round": note.round,
            "at": note.at,
            "seq": s.seq,
        }))
    }

    async fn handle_notes(&self, key_hash: &str) -> Result<Response> {
        let s = self.load_or_init(key_hash, "").await?;
        Response::from_json(&serde_json::json!({
            "session_id": s.session_id,
            "round": s.round,
            "opened_at": s.opened_at,
            "notes": s.notes,
        }))
    }

    async fn handle_close(&self, key_hash: &str) -> Result<Response> {
        let s = self.load_or_init(key_hash, "").await?;
        if s.notes.is_empty() {
            return Response::from_json(&serde_json::json!({
                "closed": false,
                "reason": "empty_session",
            }));
        }
        let closed = now_iso();
        let receipt = serde_json::json!({
            "session_id": s.session_id,
            "key_hash": s.key_hash,
            "key_label": s.key_label,
            "opened_at": s.opened_at,
            "closed_at": closed,
            "round_count": s.round,
            "notes": s.notes,
        });
        let receipt_bytes =
            serde_json::to_vec(&receipt).map_err(|e| Error::RustError(e.to_string()))?;
        let receipt_ref = format!("ledger/{}/receipt-{}.json", key_hash, s.session_id);
        let bucket = self.env.bucket("CURATOM_ARTIFACTS")?;
        bucket.put(&receipt_ref, receipt_bytes).execute().await?;

        let intents = s.notes.iter().filter(|n| n.kind == "intent").count() as i64;
        let patterns = s.notes.iter().filter(|n| n.kind == "pattern").count() as i64;
        if let Ok(db) = self.env.d1("CURATOM_LEDGER") {
            let stmt = db.prepare(
                "UPDATE sessions SET closed_at = ?1, round_count = ?2, \
                 intent_count = ?3, pattern_count = ?4, receipt_ref = ?5 \
                 WHERE session_id = ?6",
            );
            if let Ok(bound) = stmt.bind(&[
                closed.clone().into(),
                (s.round as f64).into(),
                (intents as f64).into(),
                (patterns as f64).into(),
                receipt_ref.clone().into(),
                s.session_id.clone().into(),
            ]) {
                let _ = bound.run().await;
            }
        }

        let fresh = ScratchState::fresh(key_hash, &s.key_label);
        self.save(&fresh).await?;
        Response::from_json(&serde_json::json!({
            "closed": true,
            "session_id": s.session_id,
            "receipt_ref": receipt_ref,
            "notes_flushed": s.notes.len(),
        }))
    }

    async fn handle_history(&self, key_hash: &str) -> Result<Response> {
        let db = self.env.d1("CURATOM_LEDGER")?;
        let stmt = db.prepare(
            "SELECT session_id, opened_at, closed_at, round_count, \
             intent_count, pattern_count, receipt_ref \
             FROM sessions WHERE key_hash = ?1 ORDER BY opened_at DESC LIMIT 50",
        );
        let bound = stmt.bind(&[key_hash.to_string().into()])?;
        let result = bound.all().await?;
        let rows: Vec<serde_json::Value> = result.results::<serde_json::Value>().unwrap_or_default();
        Response::from_json(&serde_json::json!({ "sessions": rows }))
    }

    async fn mirror_note(&self, s: &ScratchState, note: &Note) -> Result<()> {
        let digest = sha256_hex(note.body.as_bytes());
        let body_ref = format!(
            "ledger/{}/{}/{}-{}.txt",
            s.key_hash, s.session_id, note.round, note.id
        );
        let bucket = self.env.bucket("CURATOM_ARTIFACTS")?;
        bucket
            .put(&body_ref, note.body.as_bytes().to_vec())
            .execute()
            .await?;

        let db = self.env.d1("CURATOM_LEDGER")?;
        let upsert = db.prepare(
            "INSERT OR IGNORE INTO sessions \
             (session_id, key_hash, key_label, opened_at) \
             VALUES (?1, ?2, ?3, ?4)",
        );
        upsert
            .bind(&[
                s.session_id.clone().into(),
                s.key_hash.clone().into(),
                s.key_label.clone().into(),
                s.opened_at.clone().into(),
            ])?
            .run()
            .await?;

        let ins = db.prepare(
            "INSERT INTO ledger \
             (id, key_hash, key_label, session_id, round, kind, \
              body_digest, body_ref, knock_id, created_at, seq) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        );
        ins.bind(&[
            note.id.clone().into(),
            s.key_hash.clone().into(),
            s.key_label.clone().into(),
            s.session_id.clone().into(),
            (note.round as f64).into(),
            note.kind.clone().into(),
            digest.into(),
            body_ref.into(),
            note.knock_id.clone().unwrap_or_default().into(),
            note.at.clone().into(),
            (s.seq as f64).into(),
        ])?
        .run()
        .await?;
        Ok(())
    }
}

fn json_err(status: u16, msg: &str) -> Result<Response> {
    Response::from_json(&serde_json::json!({ "error": msg })).map(|r| r.with_status(status))
}
