-- Register via email -> ZeptoMail carries a one-time link -> the account
-- (username + password) is minted only once that link is followed. Until
-- then the registration is just a claim on an email address, not yet a
-- user row. `token_hash` is the SHA-256 of the link's token, same
-- discipline as every other credential in this file: the plaintext lives
-- once, in the email that was sent.
CREATE TABLE IF NOT EXISTS pending_verifications (
  token_hash  TEXT    PRIMARY KEY,
  email       TEXT    NOT NULL,
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pending_verifications_expires_at ON pending_verifications (expires_at);
