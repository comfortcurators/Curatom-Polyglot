# Deferred work — rv0.4.0

This file is part of the release contract: these items are **not** claimed complete in rv0.4.0. Items that were present in older versions of this list but are now implemented have been removed rather than left as misleading debt.

## Cryptographic trust

- **Independent machine identity / signed requests.** The active machine path authenticates with the operator-issued Curatom bearer key. There is no separate production machine keypair, request-signature or mTLS/JWKS identity layer in rv0.4.0; the legacy `ProductionCloudflareInorganicIdentityProvider` remains a fail-closed stub. Do not describe bearer-key possession as proof of a hardware/person-bound machine identity.
- **Asymmetric attestations.** Worker/orchestrator trust currently uses HMAC with a shared secret. A future boundary should use Ed25519 (or equivalent asymmetric signing) so verifiers need only a public key and compromise of a verifier does not reveal signing authority. Per-relationship keys are a useful intermediate hardening step.
- **Crash-persistent replay protection.** The Elixir `ReplayGuard` is ETS-backed and resets with the BEAM process. `scripts/seam.sh` deliberately demonstrates this restart boundary. Persist replay state (for example with DETS or a database) before treating replay rejection as durable across orchestrator restarts.

## Execution hardening

- **Real HostOS adapter.** The adapter behaviour exists; the rv0.4.0 HostOS implementation is a mock. Do not describe HostOS execution as production-complete.
- **Timeout/cancellation supervision.** Actions can still hang. Execution should use supervised tasks with explicit timeouts and cancellation semantics.
- **Production end-to-end Worker → container → orchestrator → outcome proof.** Individual seams and the deployable gateway exist, but the complete production loop should be exercised and retained as a release test with evidence.

## Deployment configuration

- **Move bearer credentials and mutating payloads out of URLs.** Some machine/sandbox surfaces still carry a Curatom key, sandbox identifier, command, file body, or report in query/path data. Those values can be exposed by overly broad request logging, browser history, proxy analytics, or copied URLs. Current deployments must suppress sensitive query logging; a later protocol revision should prefer `Authorization` headers and request bodies for credentials and mutating content.
- **Cloudflare Access activation.** JWT verification code exists, but `TEAM_DOMAIN` and `POLICY_AUD` are empty in the checked-in `wrangler.toml`; the Access path is therefore inert until deployment configuration supplies both values.
- **Rate limiting.** Application code does not provide a global distributed rate limiter. Internet deployments should configure Cloudflare Rate Limiting/WAF controls for authentication and knock endpoints, with thresholds appropriate to their threat model.
- **D1 tenancy.** Account/ledger tables share one D1 database. At sufficient scale this becomes a contention and isolation concern; move to an explicit tenancy/sharding policy before one owner's load can materially affect another.

## Product surfaces

- **Notification delivery.** Dashboard polling exists; email/push/queue fan-out for new knocks is not implemented.
- **Account notification/privacy/consent preferences.** These need product semantics before implementation; no setting should be exposed until it has a defined effect.
- **Global checkpoint retention policy.** Per-key checkpoint history is capped, but a total owner-wide policy still needs a deterministic eviction rule.

## Release-quality gates not executable everywhere

- **Clippy-as-error.** `cargo clippy --workspace -- -D warnings` is not yet the release gate; known warnings should be driven to zero before making it a release gate.
- **External-platform tests.** Some correctness properties depend on real Cloudflare behavior rather than local emulators. Keep real-state probes for storage migrations, routing and deployment seams because prior production defects were found there rather than by diff review.
