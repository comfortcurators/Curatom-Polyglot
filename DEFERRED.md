# What is deferred and why

These are not fake. They are not implemented.

- **Real Cloudflare Access JWT verification.** The Rust
  `CloudflareAccessIdentityProvider` treats any presence of
  `cf-access-jwt-assertion` as the owner, and in `CURATOM_ENV=development`
  also accepts `x-curatom-dev-organic`. That is the local wrangler stub.
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
- **`cargo clippy --workspace -- -D warnings`.** Not a release gate yet.
- **Live Worker ↔ Elixir on wrangler.** `scripts/seam.sh` proves Contract 2/3
  against a Worker *stand-in*. `wrangler dev` routing a real DO to Elixir
  has not run. The `#[event(fetch)]` stub that looks up `CURATOM_KERNEL` is
  in `workers/api/src/lib.rs`; the loop itself is this commit's remaining
  unrun path.

