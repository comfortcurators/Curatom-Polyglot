# Curatom architecture — index

This map is for people writing about Curatom. It describes the rv0.4.0 tree; aspirational work belongs in [`../../DEFERRED.md`](../../DEFERRED.md).

| File | Answers |
|---|---|
| [`01-shape.md`](01-shape.md) | What the system is and how its trust boundaries fit together |
| [`02-vocabulary.md`](02-vocabulary.md) | Canonical project vocabulary |
| [`03-subsystems.md`](03-subsystems.md) | What each subsystem does and where it lives |
| [`04-data-flow.md`](04-data-flow.md) | A knock from machine request through recorded outcome |
| [`05-data-model.md`](05-data-model.md) | D1, R2 and Durable Object storage surfaces |
| [`06-security.md`](06-security.md) | Identity, signatures, verification and known gaps |
| [`07-boundaries.md`](07-boundaries.md) | What Curatom is not and what remains deferred |

## The system in one page

Curatom is a **capability gate for machine access to company systems**. An operator issues a machine a key. When the machine needs authority, it knocks for a specific resource and permission. The operator approves or refuses the request within an 88-second window. Approval yields bounded authority that is consumed before remote execution; outcomes return to the operator's activity record.

The enforcement kernel is Rust/WASM on Cloudflare. Elixir/OTP is the replaceable orchestration layer for remote work. The dashboard is a Leptos Rust/WASM application sharing protocol types with the kernel. Valhalla is a separate Cloudflare Worker/container subsystem for disposable workspaces, snapshots and restores.

Three contracts describe the important boundaries: client ↔ Worker; Worker ↔ orchestrator; orchestrator ↔ execution adapter plus the authenticated outcome return path. The key architectural rule is that a remote executor receives evidence derived from already-consumed authority, not a reusable live Curatom grant.

Curatom Polyglot rv0.4.0 is a vertical slice, not a finished product. The mechanism is real; breadth and several production-hardening items are explicitly deferred. The software is AGPL-3.0-only. The `curatom.rajvansh.dev` deployment is one project deployment, not the definition of the software.

## Publication context

Read architecture claims together with [`../lineage.md`](../lineage.md): the earlier `comfortcurators/Curatom` repository contains the rv0.2.0 Zenodo artifact and the later rv0.3.0 hackathon state; Polyglot rv0.4.0 is a separate from-scratch implementation. [`../publication-due-diligence.md`](../publication-due-diligence.md) records the public-release gates.
