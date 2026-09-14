# Curatom-Polyglot

v0 vertical slice. Three languages, three contracts, one kernel.

Expo handles the human. Rust/WASM handles the enforcer. Elixir/OTP handles
the orchestration. Each language does the thing it is actually better at.

This is not Curatom Enterprise (the Google Cloud control plane at
`comfortcurators/Curatom`). This repository is the polyglot capability
kernel: owner-mode mobile, a fail-closed grant machine, and a replaceable
orchestrator.

## Trust boundaries

See [`docs/architecture.md`](docs/architecture.md) and
[`docs/contracts.md`](docs/contracts.md). The short version:

1. **Expo ↔ Worker.** HTTPS JSON. Cloudflare Access (dev: a header).
   Responses never include tokens, `grt_`, `cap_`, OAuth scopes, MCP
   schemas, or Cloudflare binding syntax.
2. **Worker ↔ Elixir.** One HMAC-signed POST. Attestation per action.
   Elixir never sees a live grant. The Worker consumes the capability
   *before* the handoff.
3. **Elixir ↔ HostOS.** Present the attestation. Replay-guarded. HostOS
   does not call back to consume — consumption already happened.

If Elixir is compromised, an attacker gets spent attestations. Replay
fails. Fabrication fails HMAC. Minting grants requires an approved
approval on the Worker.

## Layout

```
crates/           native Rust kernel (this is what `cargo test` runs)
  protocol/       types, including expires_unix on CapabilitySecret
  crypto/         random_id, now_unix (non-wasm)
  ports/          Clock, StateStore, EventLedger, ArtifactStore
  key-kernel/     TTL, consume, issue, approvals — no Cloudflare imports
  resource-registry/
  sign/           DevSignVerifier (sign2:ok)
  attestation/    HMAC-signed job envelope
  execution/      coordinator trait only; real execution lives in Elixir
  organic-router/ response views that cannot leak tokens
  inorganic-router/
  substrate-memory/
  substrate-cloudflare/   wasm: DO storage, R2, Access, CloudflareClock
workers/api/      Durable Object, inline routing (borrow-safe)
orchestrator/     Elixir OTP
apps/dashboard/   Vite + React + Framer Motion SPA, owner mode only
docs/             architecture + the three contracts, verbatim
```

## What actually runs today

```bash
# Kernel + memory substrate + Clock + TTL + attestation envelope
cargo test --workspace

# Orchestrator (requires Elixir 1.16+)
cd orchestrator && mix deps.get && mix test
# boot: MIX_ENV=dev mix run --no-halt

# Dashboard against a locally running Worker
cd apps/dashboard && npm install && npm run dev
```

`CURATOM_ENV=development` makes the Worker accept `x-curatom-dev-organic`.
Copy `.env.example`.

The Durable Object in `workers/api` is the production shape. It is not
cargo-check-verified on wasm in this commit. See `DEFERRED.md`.

## Honest notes

A serious Expo client, a serious Elixir OTP tree, and a serious Rust
kernel is three repos' worth of work. This commit is the contracts and
one working vertical slice per language:

- Rust: kernel with Clock, integer TTL, single-use consume, attestation
  sign/verify, organic views that refuse to serialize secrets.
- Elixir: JobQueue, ReplayGuard, HMAC verify, mock HostOS adapter,
  outcome report back to the Worker.
- Expo: three screens, owner mode, no tokens on the wire from its
  point of view.

The rest is in `DEFERRED.md`, not faked.

## License

AGPL-3.0-only. Copyright (C) 2026 Comfort Curators Private Limited.
