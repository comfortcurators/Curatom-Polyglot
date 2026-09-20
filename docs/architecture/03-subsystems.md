# Subsystems

**Kernel — `crates/key-kernel/src/lib.rs`.** Pure capability rules over traits: TTL, single-use consumption, digest binding, knocks, freezes, checkpoints, connectors, repositories, whitepaper and organic-safe views.

**Protocol — `crates/protocol/`.** Shared serializable vocabulary used across first-party Rust components.

**Cloudflare substrate — `crates/substrate-cloudflare/`.** Durable Object state and R2 artifact adapters. Cloudflare-specific I/O stays outside the pure kernel.

**Worker — `workers/api/src/lib.rs`.** Authentication, owner routing, HTTP/kernel translation, local dispatch, signed remote handoff and outcome ingestion. User auth/passkeys/scratchpad live in sibling modules.

**Dashboard — `apps/dashboard/`.** Leptos SPA using typed same-origin HTTP calls. It presents knocks, keys, activity, account, compute/data, whitepaper and sketchpad surfaces without direct Cloudflare access.

**Orchestrator — `orchestrator/lib/curatom_orchestrator/`.** HMAC verification, per-action task execution, replay guard, adapter dispatch and signed outcome callback. The shipped HostOS implementation is a mock; real HostOS execution remains deferred.

**Valhalla — `workers/valhalla/src/index.ts`.** Sandbox lifecycle, exec/write/read/parity/close plus snapshot and restore. Snapshot blobs are content-addressed and verified on restore.

**Signing — `crates/attestation/`, `crates/webauthn/`, `crates/sign/`.** HMAC attestations and WebAuthn verification are real; the development signing interface in `crates/sign` is not a production signature system.

**Seam test — `scripts/seam.sh` and helpers.** Cross-language behavioral evidence using a Worker stand-in. Its limitations, including replay after orchestrator restart, are intentionally visible rather than reported as green production proof.
