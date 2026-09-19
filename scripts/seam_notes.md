# Seam test

`scripts/seam.sh` is the live Contract 2 + 3 test.

It does **not** loop the kernel in-process. It:

1. Signs a job envelope with `cargo run -p curatom-attestation --example sign_job`
   (same `sign` / `hmac_hex` the Worker uses).
2. Boots the Elixir orchestrator as its own OS process.
3. POSTs the job to `/v1/jobs` with `x-curatom-hmac`.
4. Receives the HMAC outcome on a Worker stand-in (`scripts/outcome_sink.py`
   at `/internal/outcome`).
5. POSTs the same job again in-process: ExecutionWorker replay-rejects;
   no second outcome.
6. Restarts the orchestrator. ETS is empty, so the same nonce runs again.
   That is the deferred persistent-replay hole, **observed**, not faked green.

The Cloudflare Worker process itself is not in this loop. wasm `cargo check`
is green; wrangler deploy is still deferred. This seam proves the bytes
that cross the Worker↔Elixir boundary, using the same crates the Worker
calls.

```bash
export PATH="$PATH"   # mix + erl on PATH
./scripts/seam.sh
```

## Turnstile is a hard wall for any scripted verification of a knock approval

Found repeatedly verifying Parts 6-8: no automation -- curl, a headless
Chromium driven by Playwright with a real session cookie, a non-headless
browser with a virtual display -- gets past `POST
/organic/knocks/<id>/approve`'s Turnstile confirmation. A real attempt with
real Chromium got as far as opening the confirmation sheet and watching the
widget render, then sat with `CONFIRM` disabled for 10+ seconds while the
hidden `cf-turnstile-response` field never populated. That is not a bug in
the automation; it is Turnstile correctly refusing to pass a script, which
is the entire reason it is there. Do not spend more time trying to defeat it
-- that would mean deliberately circumventing this account's own anti-bot
control, not testing around an inconvenience.

Any verification step downstream of a real knock approval -- Part 6's step
3 (knock/approve/read `company.whitepaper` as a resource), Part 8's step 3
(approve, watch the outcome route back) -- needs one of:

- **A human at a real browser**, clicking `APPROVE` themselves. The
  session's other side (register a throwaway account, mint a key, submit
  the knock, then read the result afterward) can all be done by a script;
  only the click itself cannot.
- **A hand-crafted path that skips approval entirely.** Part 8 turned out
  to have exactly this: `/internal/outcome` is HMAC'd, not
  session-or-Turnstile-gated, so its routing and the orchestrator's field
  echoing can both be verified end to end without ever going near
  `/organic/knocks/*/approve`. See the two-half test below. Not every
  approval-gated flow will have an equivalent bypass -- `/organic/whitepaper`
  read via `execute_locally` genuinely does require a real approved knock,
  which is why Part 6's step 3 was argued rather than run (the identical
  code path was already proven via the plain `GET`/`PUT` round trip, which
  needs no knock at all).

## Part 8, verified without a browser -- two independent halves

Neither touches Turnstile.

**Orchestrator echo (local).** Add `"owner_id": "org_test_seam"` to the
envelope in `crates/attestation/examples/sign_job.rs`, rerun
`scripts/seam.sh`, check `scripts/.seam/outcomes.jsonl` for
`"owner_id":"org_test_seam"` in the written body. Confirms `Job.from_map`
and both `report_outcome` call sites in `ExecutionWorker` carry the field
through.

**Routing (production, no browser).** Hand-craft an HMAC'd `POST` straight
to `/internal/outcome` with a real test account's `owner_id` and a
distinctive marker in `data`. `fetch()`'s routing block reads `owner_id`,
forwards to that account's DO, `h_internal_outcome` checks the HMAC and the
owner match, records the outcome. `GET /organic/activity` on that account
shows the marker; the founder's does not -- before this fix, the reverse was
true (the founder's feed got the stranger's outcome). Confirmed live this
way on 19 Sep 2026, independently of the two halves above, by reading the
account's own retained event log (`workers/observability/telemetry/query`,
requires `[observability] enabled = true` in `wrangler.toml` -- see the 5b
incident writeup for why that was added): the `approve` call and the
returning `/internal/outcome` POST landed on the identical Durable Object
ID, both `200`, no `403 owner_mismatch`.
