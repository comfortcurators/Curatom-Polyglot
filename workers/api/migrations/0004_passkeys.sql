-- Passkeys. A credential here is a public key and nothing else -- the
-- private half never leaves the person's phone or laptop, which is the
-- whole reason a passkey beats a password. Losing this table to an
-- attacker gives them no way to log in as anyone.
--
-- `credential_id` is the authenticator's own id for the key, base64url,
-- and is the primary key because it is what the browser hands back at
-- login and what we look the row up by.
CREATE TABLE IF NOT EXISTS user_passkeys (
  credential_id  TEXT    PRIMARY KEY,
  user_id        TEXT    NOT NULL,
  public_key     TEXT    NOT NULL,
  -- The authenticator's own counter. Kept so a value that goes backwards
  -- can be spotted -- the one signal the spec gives that a credential may
  -- have been cloned. Platform passkeys commonly report 0 forever, which
  -- is not a fault and must stay loggable-in.
  sign_count     INTEGER NOT NULL DEFAULT 0,
  label          TEXT,
  created_at     INTEGER NOT NULL,
  last_used_at   INTEGER
);

CREATE INDEX IF NOT EXISTS idx_user_passkeys_user_id ON user_passkeys (user_id);

-- A ceremony's one-time challenge. Written when the browser asks to begin,
-- deleted the moment it is spent, so the same signed assertion cannot be
-- replayed even inside its expiry window.
--
-- `user_id` is NULL for a login ceremony: the browser discovers which
-- passkey to offer on its own, so the server does not know who is arriving
-- until the assertion comes back naming a credential.
CREATE TABLE IF NOT EXISTS webauthn_challenges (
  challenge   TEXT    PRIMARY KEY,
  user_id     TEXT,
  ceremony    TEXT    NOT NULL CHECK (ceremony IN ('register','login')),
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webauthn_challenges_expires_at ON webauthn_challenges (expires_at);
