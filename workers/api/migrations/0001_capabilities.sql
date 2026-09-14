-- Curatom keys each own a stable HostOS workspace once approved for MCP
-- access. Keyed on the SHA-256 hash of the plaintext key -- the plaintext
-- exists once, in the handoff block, in the operator's clipboard, and is
-- never written here. Same discipline as HostOS's own credentials: a
-- secret that only ever exists in one place cannot leak from the others.
CREATE TABLE IF NOT EXISTS capabilities (
  key_hash    TEXT    PRIMARY KEY,
  scope       TEXT    NOT NULL CHECK (scope IN ('oauth','full')),
  expires_at  INTEGER NOT NULL,
  revoked     INTEGER NOT NULL DEFAULT 0,
  label       TEXT,
  knock_id    TEXT,
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_capabilities_expires_at
  ON capabilities (expires_at);
