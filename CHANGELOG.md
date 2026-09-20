# Changelog

Reverse-chronological. This records release-level changes; `git log` remains the detailed provenance.

## rv0.4.0 — 20 Sep 2026

**Archive:** Zenodo DOI `10.5281/zenodo.22855407`.

**Publication hardening:**
- Added consistent author/organization/discovery metadata for Yash Rajvansh, Rajvansh, Comfort Curators / COMFORT CURATORS PRIVATE LIMITED and DeepTech across README, CITATION.cff, CodeMeta and Zenodo copy.
- Removed a live-test personal mailbox from a source comment before publication.
- Reviewed the sole GitHub branch history (97 reachable commits) for high-confidence secret patterns; only the intentionally labelled test HMAC example was found.


First archival open-source release of Curatom Polyglot.

- Reframed the public documentation around the implemented capability/execution architecture rather than the original three-language prototype description.
- Recorded the release lineage correctly: rv0.2.0 is the earlier Zenodo artifact, rv0.3.0 is the later Google Cloud All Things Agentic hackathon submission state in `comfortcurators/Curatom`, and Polyglot rv0.4.0 is a from-scratch implementation in a separate repository.
- Added citation, security, contribution, architecture, publication-due-diligence and third-party-notice material; replaced the abbreviated license pointer with the complete GNU AGPLv3 text.
- Unified first-party release metadata at `0.4.0` while using `rv0.4.0` as the public release/tag spelling.
- Fixed owner routing for **all** `/scratch/*` machine requests. Earlier routing only forwarded GET requests by the presenting key's owner prefix, leaving `/scratch/write` and `/scratch/close` able to fall through to session/default-owner routing.
- Fixed Valhalla `/read` for the current Cloudflare Sandbox SDK result shape by extracting `readFile(...).content` instead of serializing the SDK wrapper object.
- Preserved the existing bearer-key machine path and legacy identity scaffolding rather than deleting code as part of publication cleanup.
- Refreshed `DEFERRED.md` to describe only gaps that remain in this release: bearer-only machine identity, shared-HMAC trust, process-local replay protection, mock HostOS execution, URL-carried bearer/payload data, timeout/cancellation work, production Access configuration, distributed rate limiting, shared D1 tenancy and unfinished product surfaces.

## 19 Sep 2026 (session 2)

Parts 6 through 10 of the multi-session plan. Infrastructure-heavy; one
production incident.

**Deployed:**
- Part 6: Whitepaper editor plate; new `WhitepaperPlate`, `GET`/`PUT
  /organic/whitepaper` now reachable from the dashboard.
- Part 7: Sketchpads viewer plate; new `SketchpadsPlate`, reads
  `/scratch/notes` and `/scratch/history` per key.
- Part 7b: read-side `/scratch/*` routing fixed for the dashboard's GET
  paths. A later review found that write/close methods were still able to
  fall through to default-owner routing; rv0.4.0 closes that remaining gap.
- Part 8: `/internal/outcome` routing. The handoff envelope carries
  `owner_id`; the orchestrator echoes it; the Worker routes on it;
  `h_internal_outcome` refuses a mismatched owner. Non-founder remote
  knocks now record outcomes on the right account.
- Part 8 fix: `WhitepaperBlob` wrapper and `guarded_put` helper. Root
  cause of the production incident below.
- Part 5c: activity and outcomes Vec caps in `key-kernel`; oversized
  `Outcome::data` replaced with a truncation marker. Closes the 128 KiB
  DO storage value failure mode that was live-but-latent.
- Part 9: `owner_key_or_default`, `rebuild_request`, and
  `forward_to_owner_do` helpers; the seven owner-prefix routing blocks known
  in that pass were rewritten in terms of them. The later rv0.4.0 review
  caught the method-specific scratch write/close omission.
- `[observability] enabled = true`, 100% sampling.
- `Login requires Turnstile` -- was register-only before.

**Incident:**
- `c7aff1d1` (Part 5b, per-entity DO storage with migration) crashed on
  first production request against the founder's DO:
  `TypeError: put() called with undefined value`. Root cause:
  `serde_wasm_bindgen` lowers a top-level `Option::None` to JS
  `undefined`, which Cloudflare DO storage rejects; Miniflare accepts
  it, so local verification passed. Found by a read-only probe against
  the founder's real DO within a minute of deploy. Rolled back to
  `f327583d` immediately; no data lost, because the migration writes
  `kernel:meta` (the commit marker) and deletes the legacy blob only
  after every entity key succeeds, and it crashed before that. Fix
  landed as `103fdff`; redeployed as `2e231c11`; two clean probes
  confirmed write and read paths.

**Also shipped this session, after the incident writeup above:**
- Part 9's own re-verification caught a second-order regression before
  it deployed: the refactor initially ran `/internal/outcome`'s
  `owner_id` through the same dot-splitting helper as the token-based
  routes, silently reintroducing Part 8's exact bug for every real
  value. Found by a hand-signed HMAC test, not by diff review. Fixed in
  the same commit (`5fe7adc`) before deploy.
- `DEFERRED.md` refresh (`58fbd18`).
- This file.

**Verification note, recorded because it held three times:** every
production bug in this session was caught by a behavioral test against
real state, not by diff review. Diff review looked correct on the code
that crashed.

## 18 Sep 2026 (session 1)

Parts 1 through 5b, plus the `[observability]` gap closing at the tail
of session 2.

**Deployed:**
- Part 1: Turnstile on login.
- Part 2: Key validity (`validity_seconds`, `expires_unix` on
  `OrganicToken`), true reroll, hard delete. Kernel, Worker, and
  dashboard.
- Part 3: Keys plate rewired to the above; checkpoints modal added.
- Part 4: `access.rs`, Cloudflare Access JWT verification. Deployed
  but inert -- `TEAM_DOMAIN` and `POLICY_AUD` are empty.
- Part 5a: `DirtyKinds`, `StateStore::load`/`persist(state, dirty)`,
  memory and DO stores updated. Behavior-preserving refactor.
- Part 5b: per-entity DO storage keys, one-way migration from the
  legacy single-blob format. Rolled back; the fix is what's live.

## Earlier

Pre-session history is in `git log`. Notable commits before this
changelog existed: JWKS code, passkey support, and the initial
per-owner DO routing fix (which was found the same way the later
routing bugs were -- by testing with a non-founder account).
