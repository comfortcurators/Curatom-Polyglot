# Security policy

## Supported versions

Security fixes are applied to the latest tagged release and `main`. Pre-release snapshots and old tags may be useful for research or reproducibility but should not be assumed to receive fixes.

## Reporting a vulnerability

Please do **not** open a public issue for a vulnerability that could expose credentials, bypass authorization, cross owner boundaries, forge or replay attestations, escape a sandbox, or corrupt/delete another owner's state.

Use GitHub's private vulnerability reporting for this repository when available. If that channel is unavailable, contact Comfort Curators through a private company contact channel and include the affected version/commit, reproduction steps, expected impact, and any suggested mitigation. Do not include live credentials or user data in the report.

## Security model

Curatom Polyglot is fail-closed at the capability boundary: a machine key can request authority but does not itself grant an operation; owner approval precedes capability issuance; authority is consumed before remote execution; and internal outcomes are authenticated.

The system still has explicitly documented hardening work. In particular, HMAC is a shared-secret trust model, the Elixir replay guard is process-local, some adapters are mocks, Cloudflare Access verification requires deployment configuration, and platform-level rate limiting is expected in front of internet-facing authentication/knock endpoints. See `DEFERRED.md` for the release-specific list.

## Deployment responsibility

Some rv0.4.0 machine/sandbox routes use URL path/query data for bearer-like identifiers or payloads. Treat full request URLs as sensitive operational data: do not retain them in analytics or verbose proxy logs, and do not paste them into public issue reports. Moving these values to authorization headers/request bodies is tracked in `DEFERRED.md`.

Do not deploy with example secrets. Keep `CURATOM_HMAC_KEY`, owner/admin tokens, mail credentials, Cloudflare API credentials, and other deployment secrets in platform secret stores. Configure Cloudflare Access and rate-limiting controls appropriate to the deployment before exposing privileged surfaces to untrusted networks.
