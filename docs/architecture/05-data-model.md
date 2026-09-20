# Data model

## D1 — `CURATOM_LEDGER`

Account/auth migrations define `users`, `user_sessions`, `user_passkeys`, `webauthn_challenges`, `pending_verifications` and `password_resets`. `schema.sql` additionally carries the sketchpad `ledger`, sketchpad `sessions`, `freezes`, and Valhalla `drift_log`. `user_sessions` means browser sessions; `sessions` means machine sketchpad sessions.

## R2 — `CURATOM_ARTIFACTS`

The bucket stores sketchpad note bodies and receipts under `ledger/`, repository manifests under `repos/{owner_id}/...`, checkpoint manifests/blobs under `checkpoints/`, and Valhalla receipts under `valhalla/{sandbox_id}/receipt.json`. Checkpoint blobs are content-addressed by SHA-256.

## Durable Objects

`CuratomKernel` is one instance per owner. Its split storage keys include metadata, tokens, knocks, intents, approvals, grants, consumed/issued sets, outcomes, activity, freezes, connectors, repositories, checkpoints and whitepaper. Outcomes/activity/checkpoint histories are bounded in code. The old single `kernel_state` blob is legacy migration input.

`Scratchpad` is one instance per key hash and stores the current machine working session. Closing a session emits durable mirrors/receipts and resets current state.

## Bindings

`CURATOM_KERNEL` and `SCRATCHPAD` are Durable Object namespaces in the API Worker. `CURATOM_LEDGER` is D1; `CURATOM_ARTIFACTS` is R2. `CURATOM_ORCHESTRATOR` is the kernel-to-orchestrator service binding; `CURATOM_KERNEL`/`CURATOM_KERNEL_INTERNAL` are internal return/control bindings from Valhalla/orchestrator. `ASSETS` serves the dashboard.

Secrets such as `CURATOM_HMAC_KEY`, `RAJ_TOKEN`, `TURNSTILE_SECRET` and `ZEPTO_TOKEN` belong in Cloudflare secret storage, not source control. Deployment identifiers and non-secret vars are documented in the relevant Wrangler files.
