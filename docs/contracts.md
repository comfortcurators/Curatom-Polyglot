# The three contracts

## Contract 1 — Expo ↔ Worker

HTTPS JSON. All organic routes require `Cf-Access-Jwt-Assertion`.

In development, the Worker accepts `x-curatom-dev-organic` as a stand-in
for Cloudflare Access. That header is not a production identity.

Responses never include:

- `token`
- `grt_` prefixes
- `cap_` prefixes
- OAuth scopes
- MCP schemas
- Cloudflare binding syntax

The owner sees intents, approvals, activity, and outcomes in human language.

## Contract 2 — Worker ↔ Elixir

One POST. HMAC-signed. Contains an attestation per authorized action.

Elixir never sees the underlying grant; it sees a signed proof that this
specific operation on this specific resource by this specific requester was
authorized, single-use.

- Endpoint: `POST {CURATOM_ORCHESTRATOR_URL}` (default `/v1/jobs`)
- Header: `x-curatom-hmac: hex(HMAC-SHA256(raw_body, key))`
- Body:

```json
{
  "job_id": "job_…",
  "approval_id": "appr_…",
  "intent_id": "int_…",
  "requester_id": "fleet.curatom",
  "actions": [
    {
      "resource": "hostos.inventory",
      "operation": "read",
      "attestation": "<urlsafe-b64(claims)>.<urlsafe-b64(hmac)>"
    }
  ]
}
```

Attestation claims:

```json
{
  "v": 1,
  "job_id": "job_…",
  "grant_id": "grt_…",
  "requester_id": "fleet.curatom",
  "resource": "hostos.inventory",
  "operation": "read",
  "issued_unix": 0,
  "expires_unix": 0,
  "nonce": "nonce_…"
}
```

HMAC key: `CURATOM_HMAC_KEY`. Both sides first try standard-base64 decode,
then fall back to the raw UTF-8 bytes of the secret. Do not mix encodings.

The Worker **pre-consumes** the capability in the kernel before this POST.
Elixir is handed a spent grant's attestation, not a live token.

## Contract 3 — Elixir ↔ HostOS

Elixir presents the attestation to HostOS. HostOS validates against the
Worker's HMAC secret (v0) or public key (Ed25519, deferred) and against a
replay guard.

HostOS does not call back to the Worker to consume the grant — the Worker
pre-consumed it before handing off. Consumption is a kernel decision, made
once, before the orchestrator is trusted with the action.

Outcomes flow the other way: Elixir POSTs `/internal/outcome` to the Worker,
HMAC-signed the same way as Contract 2. No token, no scope.

## HMAC envelope (v0)

- MAC: HMAC-SHA256
- Job body MAC: hex digest, header `x-curatom-hmac`
- Attestation MAC: URL-safe base64, no padding, `payload.sig`
- Compare signatures in constant time
