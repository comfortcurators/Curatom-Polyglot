# Security model

Curatom is designed so a machine can act only within authority explicitly approved by an operator, with that authority consumed before downstream execution. The machine and downstream executor are not treated as trusted enforcement authorities.

## Identity and trust boundaries

Browser/operator routes use the account-session path and supported operator credentials implemented in `workers/api`; Cloudflare Access JWT verification exists but remains inert while its checked-in deployment configuration is empty. `RAJ_TOKEN` is a deployment break-glass credential and must remain a Worker secret.

Machine-facing requests currently use an operator-issued bearer key. rv0.4.0 does **not** ship an independent cryptographic machine-identity or signed-request layer; a legacy production inorganic identity-provider stub still fails closed, while the active knock path validates the Curatom key itself. Short validity, revocation and narrow approvals therefore remain important mitigations.

Contract 2 uses HMAC-SHA256 over the raw handoff body. The orchestrator verifies that HMAC before processing the job. Individual action attestations are also signed and checked for expiry/replay before the action is handed to an adapter. The shipped HostOS adapter is a mock; rv0.4.0 does not claim independent attestation verification by a real HostOS deployment.

Outcome callbacks to the Worker are HMAC-authenticated and owner-scoped before recording.

## Deliberately visible limitations

There is no application-level distributed rate limiter. Replay protection in the Elixir orchestrator is memory-backed and does not survive a BEAM restart. Passkey attestation provenance is not validated; the verifier proves possession of the registered key, not authenticator-model provenance. Contract HMACs cover bodies rather than arbitrary HTTP headers. Real HostOS execution is not shipped in this repository. Cloudflare Access verification is implemented but inactive until its deployment values are configured. These are gaps, not implied features; see [`../../DEFERRED.md`](../../DEFERRED.md).

## Compromise boundaries

Compromise of the orchestrator should not yield a reusable live Curatom capability because remote execution receives signed evidence after kernel consumption. The process-local replay guard still means a previously valid attestation may be replayable after a BEAM restart until persistent replay protection is implemented.

Compromise of a machine exposes its bearer key until expiry/revocation, but that key does not itself approve new authority. Compromise of an operator browser/session is serious because the operator surface can approve requests and manage keys; browser sessions should therefore be treated as high-value authority.
