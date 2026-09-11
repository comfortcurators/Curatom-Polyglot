# What is deferred and why

These are not fake. They are not implemented.

- **Real Cloudflare Access JWT verification.** The Rust
  `CloudflareAccessIdentityProvider` trusts a header set by a middleware.
  In production, verify the JWT against Cloudflare's JWKS. That is a
  workers-rs crate for JWKS plus a cache. Not written here.
- **Real machine authentication.** `ProductionCloudflareInorganicIdentityProvider`
  throws. That is intentional. Real machine auth (mTLS, signed requests,
  JWKS-validated bearer) is its own module with its own threat model.
- **Ed25519 attestations.** HMAC with a shared secret works for v0 because
  Worker and Elixir are on the same private network. Once HostOS adapters
  live on different hosts, move to Ed25519 and publish the Worker's public
  key.
- **Replay guard persistence.** `ReplayGuard` is ETS-backed and dies with
  the Elixir process. For crash-safe replay protection, back it with `:dets`
  or Postgres. The current version protects against an attacker replaying
  within the same BEAM lifetime, which is the realistic threat.
- **Supervision for stuck jobs.** Currently each action runs as a Task; if
  it hangs, it hangs. Add `Task.Supervisor.async_nolink/3` with a timeout,
  and a `:timer` based cancellation.
- **Real HostOS adapter.** Mock only. The adapter behaviour is stable; the
  real one wires to HostOS-MCP.
- **Rust DO compilation.** `workers/api` is shaped correctly (inline
  dispatch, no router holding `&mut Kernel` across `.await`) but is not a
  workspace member and is not `cargo check`-verified on wasm32. Expect
  borrow errors around `self.env` access inside async handlers — the fix is
  to snapshot env values before the borrow, which is a mechanical change.
- **`cargo clippy --workspace -- -D warnings`.** Not a release gate yet.
- **Live Worker ↔ Elixir seam test.** The HMAC envelope is tested in Rust
  (sign, verify, replay, TTL) and the Elixir modules have unit tests for
  attestation and replay. A script that boots both processes across a
  restart of the orchestrator is the next commit, not this one.
