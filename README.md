# Curatom Polyglot

**Owner-controlled capability infrastructure for letting machines request narrowly scoped authority, execute across explicit trust boundaries, and return recorded outcomes without receiving the operator's underlying credentials.**

**rv0.4.0 DOI:** [10.5281/zenodo.22855407](https://doi.org/10.5281/zenodo.22855407)

**Project lead:** [Yash Rajvansh](https://yashrajvansh.link/) ([ORCID 0009-0003-1658-2682](https://orcid.org/0009-0003-1658-2682)) · **Organization:** [Comfort Curators](https://comfortcurator.com/) / **COMFORT CURATORS PRIVATE LIMITED** · **Field:** DeepTech, capability security, machine authorization, human-in-the-loop AI infrastructure.

Curatom Polyglot rv0.4.0 is a from-scratch implementation in the Curatom project lineage. An operator issues a machine a key; the machine *knocks* for a specific resource and permission; the operator approves or refuses the request; the kernel issues and consumes bounded authority; execution crosses a signed boundary; and the outcome returns to the operator's state.

The implementation is deliberately polyglot: Rust/WASM is the enforcement kernel and Cloudflare edge integration, Elixir/OTP is the replaceable orchestration layer, Cloudflare Containers provide isolated execution, and the operator dashboard is Rust/Leptos compiled to WebAssembly.

## Release lineage

The version numbers mark the public Curatom lineage; they do **not** mean that this repository is an in-place upgrade of the earlier source tree.

- **Curatom rv0.2.0** — the earlier `comfortcurators/Curatom` implementation was archived on Zenodo before the hackathon submission.
- **Curatom rv0.3.0** — the same earlier repository later became the Google Cloud All Things Agentic hackathon evaluation/submission state. Its README explicitly records rv0.2.0 as the pre-hackathon Zenodo release.
- **Curatom Polyglot rv0.4.0** — this repository, `comfortcurators/Curatom-Polyglot`, rebuilt from scratch. It carries forward the broad intent — explicit human authority over machine action — while changing the architecture, vocabulary, trust boundaries, storage model and execution model substantially.

Accordingly, rv0.4.0 is **not “rv0.3.0 plus changes.”** The earlier repository remains a historical artifact; Polyglot is a new implementation in the same project lineage. See [`docs/lineage.md`](docs/lineage.md).

## rv0.4.0

This release includes per-owner Durable Object state, D1 account/authentication state, R2 artifacts and checkpoint snapshots, operator-issued machine keys, knock/approval flows, capability consumption, HMAC attestations, outcome routing, repository inventory/materialization, Valhalla sandbox provisioning and restore, whitepaper/sketchpad surfaces, passkeys, and a Rust/Leptos operator dashboard.

It is a working vertical slice, not a claim of production completeness. [`DEFERRED.md`](DEFERRED.md) names what is deliberately incomplete; [`SECURITY.md`](SECURITY.md) defines the security-reporting surface; [`docs/architecture/07-boundaries.md`](docs/architecture/07-boundaries.md) is the publishing honesty check.

## Trust model

1. **Operator → Worker.** Browser sessions authenticate the operator. Cloudflare Access JWT verification exists and activates when its deployment configuration is supplied; a separate break-glass operator credential also exists.
2. **Machine → Worker.** A machine presents an operator-issued key and requests explicit resources and permissions. The key lets it ask; it does not let it self-approve.
3. **Worker → execution.** Capability authority is consumed before remote handoff. The orchestrator receives HMAC-signed evidence of spent authority, not the operator's credential or a reusable live grant.
4. **Execution → Worker.** Outcomes return over an HMAC-authenticated internal callback and are routed to the correct owner state.
5. **Sandbox state.** Checkpoints can reference content-addressed R2 snapshots and restore workspaces into later sandboxes. Sandboxes are disposable; kernel state and persisted artifacts are authoritative.

Start with [`docs/architecture/README.md`](docs/architecture/README.md). Protocol boundaries are in [`docs/contracts.md`](docs/contracts.md).

## Repository map

```text
crates/                    Rust kernel, protocol types, ports and cryptographic boundaries
workers/api/               Cloudflare Worker + Durable Objects + D1/R2 integration
workers/orchestrator/      Cloudflare gateway for the Elixir container
workers/valhalla/          sandbox provisioning, execution, snapshot and restore
orchestrator/              Elixir/OTP intake, verification, execution and outcome callback
apps/dashboard/            Rust/Leptos operator dashboard compiled to WASM
docs/architecture/         canonical architectural map for readers and publishing
scripts/                   local verification, seam tests, backup and test utilities
```

## Verify a checkout

Curatom does not use hosted CI as release authority. Verification is run against the exact release candidate locally:

```bash
./scripts/release-check.sh
```

The release script is intentionally strict: missing Cargo/Rust WASM, Trunk, Mix, Node/npm, or Python tooling is a failure rather than a green skip. It runs the native Rust tests, Worker WASM check, dashboard build, Elixir tests, Valhalla TypeScript check, seam test, and syntax checks. Real Cloudflare behavioral probes, full-history/object secret scanning, and publication/license review remain manual release gates; see [`RELEASE.md`](RELEASE.md).

## Configuration and secrets

Copy `.env.example` for local development. Never commit `.env`, `.dev.vars`, production HMAC keys, Cloudflare credentials, mail-provider credentials, session material, private keys or exported state. Deployment identifiers in the checked-in Wrangler configuration are not credentials; deployment secrets belong in the platform secret store.

The GitHub-visible history was reviewed before the rv0.4.0 publication. If a separate local clone contains private refs or objects that never reached GitHub, run an object-level secret scanner there as an additional defense-in-depth check before visibility changes.

## Open-source position

Curatom Polyglot is released under **GNU Affero General Public License v3.0 only (`AGPL-3.0-only`)**. The choice is intentional: the mechanisms that decide and carry machine authority should remain inspectable when modified and offered over a network. See [`docs/why-open.md`](docs/why-open.md).

The repository contains the complete AGPLv3 license text. Third-party components retain their respective licenses and notices; see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## Citation and archival record

[`CITATION.cff`](CITATION.cff) and [`codemeta.json`](codemeta.json) describe this release. Search/discovery metadata for GitHub and Zenodo is recorded in [`docs/publication-metadata.md`](docs/publication-metadata.md). The release tag is **`rv0.4.0`**. The archived rv0.4.0 release is available on Zenodo at **[10.5281/zenodo.22855407](https://doi.org/10.5281/zenodo.22855407)**.

The older Curatom repository should be cited independently: its **rv0.2.0** release is the existing Zenodo artifact, while **rv0.3.0** is the later Google Cloud All Things Agentic hackathon submission/evaluation state in that repository. Polyglot should not be described as a source upgrade of either state.

## Contributing

Focused bug fixes, tests, documentation corrections and narrowly scoped features are welcome under the same `AGPL-3.0-only` terms. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before changing a trust boundary.

Copyright © 2026 Comfort Curators Private Limited.
