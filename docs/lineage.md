# Curatom release lineage

Curatom Polyglot belongs to the Curatom project lineage, but it is not a conventional next version of the earlier source tree.

## rv0.2.0 — the earlier Zenodo artifact

The earlier implementation lives in `comfortcurators/Curatom`. Its rv0.2.0 source archive was published on Zenodo before the later hackathon submission. That record is an independently citable historical artifact and should remain intact.

## rv0.3.0 — the hackathon submission state

The same earlier repository later reached rv0.3.0 as the Google Cloud All Things Agentic hackathon evaluation/submission build. The repository's own rv0.3.0 README states that Curatom existed before the hackathon and identifies rv0.2.0 as the Zenodo release.

rv0.3.0 therefore belongs to the earlier implementation line. It is useful history, but it is not the source base of Polyglot.

## rv0.4.0 — `comfortcurators/Curatom-Polyglot`

Curatom Polyglot was built from scratch in a separate repository. Readers should not infer source compatibility, migration compatibility, or architectural continuity from the version numbers alone.

The continuity is at the level of intent: explicit human authority over machine action, inspectable boundaries, and evidence of what happened. Polyglot develops that intent through a different architecture: a Rust/WASM enforcement kernel, owner-scoped Cloudflare state, an Elixir/OTP orchestrator, signed handoffs, disposable Valhalla sandboxes, and a Rust/Leptos operator surface.

## How to describe the relationship

Preferred: **“Curatom Polyglot rv0.4.0 is a from-scratch implementation in the Curatom project lineage. The earlier `comfortcurators/Curatom` repository contains the rv0.2.0 Zenodo artifact and the later rv0.3.0 Google Cloud All Things Agentic hackathon submission state.”**

Avoid: “rv0.4.0 upgrades rv0.3.0”, “Polyglot is the rv0.3.0 codebase rewritten incrementally”, or claims that imply this repository is derived from the earlier source tree unless a specific file actually is.

For publication, cite the archives/repositories independently. The older artifacts document where the project came from; this repository documents what Polyglot is.
