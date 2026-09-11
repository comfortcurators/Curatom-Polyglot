CREATE TABLE IF NOT EXISTS ledger (
  id TEXT PRIMARY KEY,
  key_hash TEXT NOT NULL,
  key_label TEXT,
  session_id TEXT NOT NULL,
  round INTEGER NOT NULL DEFAULT 1,
  kind TEXT NOT NULL,
  body_digest TEXT NOT NULL,
  body_ref TEXT NOT NULL,
  knock_id TEXT,
  created_at TEXT NOT NULL,
  seq INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ledger_key_hash ON ledger(key_hash);
CREATE INDEX IF NOT EXISTS idx_ledger_created_at ON ledger(created_at);
CREATE INDEX IF NOT EXISTS idx_ledger_kind ON ledger(kind);
CREATE INDEX IF NOT EXISTS idx_ledger_session ON ledger(session_id);

CREATE TABLE IF NOT EXISTS sessions (
  session_id TEXT PRIMARY KEY,
  key_hash TEXT NOT NULL,
  key_label TEXT,
  opened_at TEXT NOT NULL,
  closed_at TEXT,
  round_count INTEGER NOT NULL DEFAULT 0,
  intent_count INTEGER NOT NULL DEFAULT 0,
  pattern_count INTEGER NOT NULL DEFAULT 0,
  receipt_ref TEXT
);

CREATE INDEX IF NOT EXISTS idx_sessions_key_hash ON sessions(key_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_opened_at ON sessions(opened_at);
