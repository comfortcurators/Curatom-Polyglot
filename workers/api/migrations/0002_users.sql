-- Real per-user accounts: email-verified registration, system-minted
-- username and password -- replacing (additively, for now -- RAJ_TOKEN
-- and Cloudflare Access are not removed by this migration) the single
-- shared-secret/Access-identity model. Nobody types a password in at
-- signup, the same mint-once shape every other credential in this org
-- already uses (organic tokens, capability keys, business_register's
-- keys): register with an email, verify it, and the account you get
-- back already has its username and password chosen for you.
--
-- `id` is a separate stable identifier from both `username` and
-- `email`, so a user's rows elsewhere (a Durable Object name, an R2
-- prefix) never have to be rewritten if either is ever corrected.
--
-- `password_hash` is PBKDF2-HMAC-SHA256 over `password_salt` -- never
-- the plaintext password, same discipline as `capabilities.key_hash`
-- in 0001: what leaks from a stolen row is not a usable credential.
CREATE TABLE IF NOT EXISTS users (
  id             TEXT    PRIMARY KEY,
  username       TEXT    NOT NULL UNIQUE,
  email          TEXT    NOT NULL UNIQUE,
  password_hash  TEXT    NOT NULL,
  password_salt  TEXT    NOT NULL,
  created_at     INTEGER NOT NULL
);

-- A session is a bearer credential exactly like an organic-router token
-- or a Curatom capability key: the plaintext (the cookie value) exists
-- once, in the browser; only its SHA-256 is stored here.
--
-- Named `user_sessions`, not `sessions`: this database already has a
-- `sessions` table (root schema.sql's Valhalla sessions -- session_id,
-- key_hash, opened_at, round_count...), a completely different shape.
-- `CREATE TABLE IF NOT EXISTS sessions` would have silently no-op'd
-- against it and then failed on the first `CREATE INDEX` naming a
-- column that table doesn't have -- which is exactly what happened
-- before this fix.
CREATE TABLE IF NOT EXISTS user_sessions (
  session_hash  TEXT    PRIMARY KEY,
  user_id       TEXT    NOT NULL,
  created_at    INTEGER NOT NULL,
  expires_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_user_sessions_expires_at ON user_sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_user_sessions_user_id ON user_sessions (user_id);
