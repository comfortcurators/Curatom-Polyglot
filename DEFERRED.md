# What is deferred and why

These are not fake. They are not implemented.

Refreshed 19 Sep 2026. The previous version of this file predates
Contracts 1, 2, and 3 being exercised end-to-end against real
Cloudflare infrastructure. Several items on the old list are now
shipped; they are listed at the bottom under "Recently closed" so
their absence here is not mistaken for silent removal, and so anyone
planning work from the old commit history can see where each item
went.

## Still deferred

- **Real machine authentication.** `ProductionCloudflareInorganicIdentityProvider`
  throws. That is intentional. Real machine auth (mTLS, signed requests,
  JWKS-validated bearer) is its own module with its own threat model.
  Every compute connector naming a local device or a remote machine is
  blocked on this; building them before it means building on a stub.

- **Ed25519 attestations.** HMAC with a shared secret works for v0
  because Worker and Elixir are on the same private network. Once HostOS
  adapters live on different hosts, move to Ed25519 and publish the
  Worker's public key. The shared-secret model also means one key
  compromise is every contract's compromise; per-relationship keys
  (Worker↔Elixir separate from Valhalla↔Kernel) is the interim step
  that has also not been taken.

- **Replay guard persistence.** `ReplayGuard` is ETS-backed and dies
  with the Elixir process. For crash-safe replay protection, back it
  with `:dets` or Postgres. The current version protects against an
  attacker replaying within the same BEAM lifetime, which is the
  realistic threat -- `scripts/seam.sh` deliberately proves the
  restart-replay hole rather than faking it closed.

- **Supervision for stuck jobs.** Each action runs as a Task; if it
  hangs, it hangs. Add `Task.Supervisor.async_nolink/3` with a timeout
  and a `:timer` based cancellation.

- **Real HostOS adapter.** Mock only. The adapter behaviour is stable;
  the real one wires to HostOS-MCP.

- **`cargo clippy --workspace -- -D warnings`.** Not a release gate.
  Several pre-existing warnings, none are errors.

- **Live Worker ↔ Elixir on wrangler.** `scripts/seam.sh` proves
  Contract 2/3 against a Worker *stand-in* (a Python sink, not the
  deployed Worker). The Elixir orchestrator has since been wired into
  a `@cloudflare/containers` gateway that can be deployed, but the
  loop of "production Worker approves a knock, container handles it,
  outcome lands back" has not been run end to end in production. The
  handoff POST from Worker to orchestrator is the last verified step.

- **Cloudflare Access JWT verification is written but inert.**
  `access.rs` exists, compiles, is deployed -- and does nothing.
  `TEAM_DOMAIN` and `POLICY_AUD` are both empty strings in
  `wrangler.toml`, so `verify_and_resolve` returns `Ok(None)`
  immediately and the kernel falls through to session cookie /
  RAJ_TOKEN / dev header. The code path is complete; the activation is
  one config change and a redeploy, and needs the real values from the
  Cloudflare Access dashboard.

- **D1 remains one shared database.** `CURATOM_LEDGER` holds every
  account's `users`, `user_sessions`, `pending_verifications`,
  `password_resets`, `webauthn_challenges`, `user_passkeys`, `ledger`,
  `sessions`, `freezes`, and `drift_log`. Cloudflare's guidance is one
  D1 per tenant (10 GB each, single-threaded per-database); the current
  shape is a bottleneck the platform is designed to avoid. Not urgent
  at current scale; urgent the moment one owner's ledger write rate
  becomes another owner's problem.

- **Notification channel.** The knock list is polled every 2 seconds
  by every open dashboard tab (`counts.rs`). There is no email, no
  push, no Queues fan-out. A production "you have a knock" would be
  Cloudflare Queues + an Email Worker consumer. Not started.

- **Account preferences: notification, privacy, consent.** The design
  doc lists these under Account. None exist. Blocked on their own
  semantics before they're blocked on implementation: does consent
  gate anything, is notification preference a real setting when
  nothing notifies, is the anonymization checkbox opt-in or opt-out
  and what does it actually do.

- **Valhalla checkpoint restore.** A `Checkpoint` is a
  `{id, note, created_at}` marker, not a snapshot. There is nothing to
  restore from. Making checkpoints real means deciding what they hold
  (an R2 tarball, a git SHA, a sandbox filesystem diff) and where the
  snapshot is taken. Design decision, not implementation.

- **Checkpoint cap across keys.** `Kernel::create_checkpoint` caps
  each key's own list at 200. The `checkpoints` map is keyed by token,
  so an owner with many keys can still exceed DO storage from
  checkpoints alone. A total cap needs a policy for which key's
  history is truncated when the map is full -- a product decision.

- **`company.inventory` is registered but nothing dispatches it.**
  `curatom_resource_registry::all()` lists it, `docs/llm-guide.md`
  promises it, `locally_dispatchable` in `workers/api/src/lib.rs` does
  not handle it, and the orchestrator's `adapter_for` raises
  `"unknown adapter target"` for anything not named `hostos.*` or
  `cloudflare.*`. A knock for it is approved, consumes the capability,
  hands off, and dies with no outcome recorded. Latent (nobody knocks
  for it) but a real gap between the guide and the code.

- **`handle_notes` creates an empty session as a side effect.**
  `Scratchpad::handle_notes` calls `load_or_init`, which creates a
  fresh `ScratchState` if none exists. Merely reading a key's
  sketchpad reserves a session for the machine that will eventually
  use it. Harmless in practice; means the dashboard's sketchpad view
  is not strictly read-only against the sketchpad store.

- **Turnstile wall on scripted knock approval.** Documented in
  `scripts/seam_notes.md`. Any verification path downstream of a real
  knock approval needs a human at a browser or a hand-crafted path
  that skips approval. Not a bug; the control working as designed.

- **`/internal/release` looks like dead code.** It appears in the DO's
  dispatch table but no caller sends to it -- Valhalla's close path
  hits `/internal/release-session`, not `/internal/release`. Confirm
  with a grep before removing. If a caller exists that this session
  missed, it has the same routing shape as `/internal/outcome` and
  needs the same treatment.

- **Rate limiting.** No endpoint has it. `/auth/login`,
  `/auth/register`, and `/inorganic/knock` are the obvious three.
  Turnstile on login and register raises the cost of scripting; it
  does not eliminate it, and Cloudflare's own Rate Limiting Rules are
  the natural place for the rest.

## Recently closed

Listed so their absence above is not mistaken for silent removal.
Each moved from "deferred" to "shipped" during 19 Sep 2026 sessions.

- **Real Cloudflare Access JWT verification.** Written and deployed;
  still inert pending config, see above.
- **Per-entity DO storage.** `KernelState` now stored across fourteen
  `kernel:*` keys with per-entity dirty tracking, not one JSON blob.
  Migration is one-way and idempotent; rollback note lives in
  `crates/substrate-cloudflare/src/lib.rs` above the migration `impl`.
- **`guarded_put` discipline.** Every `storage.put` call in
  `substrate-cloudflare` goes through a helper that names the key on
  the `serde_wasm_bindgen` `undefined` failure mode.
- **Owner-prefix routing for every token-bearing route without a
  session.** Seven routes; grep `forward_to_owner_do` in
  `workers/api/src/lib.rs` to list them.
- **Key validity, reroll, delete.** `OrganicToken` carries
  `validity_seconds` and `expires_unix`; `token_matches` respects
  expiry; `reroll_key` and `delete_key` exist with distinct semantics.
- **Dashboard plates** for checkpoints, whitepaper editor, and
  sketchpads viewer.
- **Login requires Turnstile.** Same gate as registration.
- **`[observability]` enabled** in `wrangler.toml`, 100% sampling,
  logs persisted.
- **Activity and outcomes Vec caps** in `key-kernel`; oversized
  `Outcome::data` is replaced with a truncation marker before storage.
- **`/internal/outcome` routing.** The envelope carries `owner_id`,
  the orchestrator echoes it, the Worker routes on it, and
  `h_internal_outcome` refuses a mismatched owner.
