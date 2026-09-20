# The three contracts

## Contract 1 — operator or machine ↔ Worker

HTTPS/JSON is the public application boundary.

Operator routes use an authenticated owner context: account sessions are the normal path, Cloudflare Access JWT verification is supported when `TEAM_DOMAIN` and `POLICY_AUD` are configured, and a deployment break-glass credential exists separately. Development-only identity shortcuts are gated by development configuration.

Machine routes use an operator-issued Curatom key. The key identifies the owner and machine-facing state and lets the machine request authority; it does not approve its own request.

A useful precision for publishing: **operator key-management routes may return the machine key token**, because the operator has to hand that credential to a machine. What the browser-facing surface must not expose is a reusable capability/grant secret, the HMAC signing secret, connector header values, or Cloudflare binding internals. Do not summarize Contract 1 as “no token ever reaches the browser.”

The operator sees knocks, key lifecycle, activity and outcomes in the dashboard. Machine-facing routes receive only the information required for their own flow.

## Contract 2 — Worker ↔ Elixir orchestrator

One signed job POST per handoff. The Cloudflare Worker consumes capability authority **before** remote execution and then sends the orchestrator signed evidence of the authorized action.

The transport body is authenticated with HMAC-SHA256 in `x-curatom-hmac`. Each action also carries an HMAC-signed attestation containing the job/grant/requester/resource/operation, issuance and expiry timestamps, and a nonce. `CURATOM_HMAC_KEY` is the shared secret in rv0.4.0; both sides use the same key-decoding rules.

The important property is negative: Elixir does **not** receive the live capability secret that the kernel consumed. Compromise of the orchestrator therefore does not by itself provide a reusable Curatom grant.

The complete wire shape is defined by `workers/api/src/lib.rs`, `crates/attestation/`, and `orchestrator/lib/curatom_orchestrator/` in the release commit. Those files, rather than examples in prose, are authoritative when exact field names matter.

## Contract 3 — orchestrator ↔ execution adapter, then outcome ↔ Worker

In rv0.4.0, Elixir verifies the action attestation and its expiry/replay state **before** invoking an execution adapter. The shipped HostOS adapter implementation is a mock. The adapter receives the already-verified action fields; this release does not ship a real HostOS target that independently receives and verifies the raw Curatom attestation.

That distinction matters: public writing must not claim that a real HostOS deployment currently verifies Curatom attestations. Real HostOS execution is explicitly deferred in `DEFERRED.md`.

After execution, Elixir sends the outcome back to the Worker over the internal callback. The callback body is HMAC-authenticated, carries the owner identifier used for routing, and is checked against the receiving owner's Durable Object before the outcome is recorded.

## HMAC envelope in rv0.4.0

- MAC: HMAC-SHA256.
- Job/callback body MAC: hex digest in `x-curatom-hmac`.
- Action attestation: URL-safe-base64 payload plus URL-safe-base64 signature.
- Signature comparisons are constant-time where authentication material is compared.
- Replay protection in Elixir is process-local in rv0.4.0; persistence across BEAM restarts is deferred.

The HMAC design is deliberately version-bounded. Asymmetric attestations and stronger persistent replay protection are listed in `DEFERRED.md` rather than implied complete here.
