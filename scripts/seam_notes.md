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
