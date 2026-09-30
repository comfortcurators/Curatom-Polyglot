# Workspace wire patches

These patches complete the standing/workspace wire-up against this branch tip
(which already includes `standing.rs`, `workspace.rs`, protocol `StandingGrant`,
ports `STANDING`, docs, and env notes).

```bash
./scripts/apply-workspace-wire.sh
```

Prefer plain `.patch` files. `.gz.b64` may corrupt through some transports.

```bash
git apply --index patches/01-kernel-substrate-wire.patch
git apply --index patches/02a-worker-api-wire.patch
git apply --index patches/02b-valhalla-bootstrap.patch
```

Verified: `git apply --check` succeeds on `feat/company-workspace-standing`.

After apply: reusable standing capabilities, DO standing storage, standing CRUD +
`/organic/audit`, Valhalla bootstrap on key create, CF inventory adapter,
standing short-circuit on submit, `/organic/repositories/sync-all`.

**Not live until deploy.** Deploy needs wrangler auth + Node >=22 for
curatom-kernel, curatom-orchestrator, curatom-valhalla. Do not delete Workers/D1.
