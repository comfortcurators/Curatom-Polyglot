-- The mint-once, show-once password lived in the verify GET's response
-- body -- which meant whichever request hit that link first got it.
-- Mail providers' link scanners (Safe Links, and equivalents at Google/
-- Yahoo/Zoho) prefetch every URL in an inbound email before the person
-- ever clicks it, so the scanner won, and the human got
-- "unknown_or_used_token" on a token already burned by a GET they never
-- issued. The account existed; the only copy of its password didn't
-- reach anyone.
--
-- The fix removes the secret from this path entirely: the password is
-- chosen at registration (POST /auth/register, same request as the
-- email), hashed immediately, and carried on the pending row. The verify
-- GET no longer creates or reveals a credential -- it only flips pending
-- into a real account, which is safe to happen twice (a scanner first,
-- a human second) because there is nothing left in that response worth
-- winning a race for.
ALTER TABLE pending_verifications ADD COLUMN password_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE pending_verifications ADD COLUMN password_salt TEXT NOT NULL DEFAULT '';
