-- A username chosen at registration, carried on the pending row the
-- same way the chosen password already is (see 0005). Empty string
-- means "not chosen" -- handle_verify falls back to minting one from
-- the email's local part, same as before this existed.
ALTER TABLE pending_verifications ADD COLUMN username TEXT NOT NULL DEFAULT '';
