-- Real per-user accounts: email + password, replacing (additively, for
-- now -- RAJ_TOKEN and Cloudflare Access are not removed by this migration)
-- the single shared-secret/Access-identity model. `email` is the natural
-- key a person registers with; `id` is a separate stable identifier so a
-- user's rows elsewhere (a Durable Object name, an R2 prefix) never have
-- to be rewritten if an email is ever corrected.
--
-- `password_hash` is PBKDF2-HMAC-SHA256 over `password_salt` -- never the
-- plaintext password, same discipline as `capabilities.key_hash` in
-- 0001: what leaks from a stolen row is not a usable credential.
CREATE TABLE IF NOT EXISTS users (
  id             TEXT    PRIMARY KEY,
  email          TEXT    NOT NULL UNIQUE,
  password_hash  TEXT    NOT NULL,
  password_salt  TEXT    NOT NULL,
  created_at     INTEGER NOT NULL
);

-- A session is a bearer credential exactly like an organic-router token
-- or a Curatom capability key: the plaintext (the cookie value) exists
-- once, in the browser; only its SHA-256 is stored here.
CREATE TABLE IF NOT EXISTS sessions (
  session_hash  TEXT    PRIMARY KEY,
  user_id       TEXT    NOT NULL,
  created_at    INTEGER NOT NULL,
  expires_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id);
