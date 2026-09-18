-- Forgot-password flow. Same shape as pending_verifications: a hashed,
-- single-purpose, short-lived token. The row is looked up by user_id
-- (not stored on the user row itself) so a reset request never mutates
-- `users` until the person actually completes it with a new password.
CREATE TABLE IF NOT EXISTS password_resets (
  token_hash  TEXT    PRIMARY KEY,
  user_id     TEXT    NOT NULL,
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_password_resets_expires_at ON password_resets (expires_at);
