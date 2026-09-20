# Zenodo metadata — Curatom Polyglot rv0.4.0

Publication metadata for the archived rv0.4.0 source release. Version DOI: `10.5281/zenodo.22855407`.

- **Title:** Curatom Polyglot rv0.4.0 — Owner-Controlled Capability Infrastructure for Machine Authorization
- **Resource type:** Software
- **Version:** rv0.4.0
- **DOI:** 10.5281/zenodo.22855407
- **Publication date:** 2026-09-20
- **Creator:** Yash Rajvansh
- **ORCID:** 0009-0003-1658-2682
- **Affiliation:** COMFORT CURATORS PRIVATE LIMITED (Comfort Curators)
- **License:** GNU Affero General Public License v3.0 only (AGPL-3.0-only)
- **Repository:** `https://github.com/comfortcurators/Curatom-Polyglot`
- **Language:** English

## Description

Curatom Polyglot is open-source DeepTech software by Yash Rajvansh and Comfort Curators Private Limited for owner-controlled machine authorization. An operator issues a machine a key; the machine requests narrowly scoped authority through a time-bounded knock; the operator approves or refuses it; capability authority is consumed before remote execution; and authenticated outcomes return to owner-scoped state.

rv0.4.0 is a from-scratch implementation in the Curatom project lineage. It is not an in-place source upgrade of the earlier `comfortcurators/Curatom` codebase. That earlier repository contains the rv0.2.0 Zenodo artifact and the later rv0.3.0 Google Cloud All Things Agentic hackathon submission/evaluation state. Polyglot carries forward the broad intent of explicit human authority over machine action while replacing the architecture and implementation.

The implementation uses a Rust/WASM capability kernel on Cloudflare, an Elixir/OTP orchestration layer, a Rust/Leptos dashboard, Durable Objects, D1, R2, and disposable Valhalla sandbox containers. Known incomplete or deliberately deferred work is recorded in `DEFERRED.md`.

## Keywords

Curatom; Curatom Polyglot; Yash Rajvansh; Rajvansh; Comfort Curators; COMFORT CURATORS PRIVATE LIMITED; DeepTech; open source; capability security; machine authorization; human-in-the-loop AI; AI agents; agent security; Rust; WebAssembly; Elixir; Cloudflare Workers; Durable Objects

## Version lineage on Zenodo

The existing Curatom record shows rv0.2.0 at version DOI `10.5281/zenodo.22112980` and Concept DOI `10.5281/zenodo.22112979`. The rv0.3.0 hackathon state exists in the earlier GitHub repository but was not the Zenodo version shown in that record.

The rv0.4.0 archive was published as a new version in the Curatom Zenodo lineage. Its version DOI is **10.5281/zenodo.22855407**. The repository for this version is `comfortcurators/Curatom-Polyglot`; the source discontinuity from the earlier implementation is documented above and in `docs/lineage.md`.
