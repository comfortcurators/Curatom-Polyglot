# Workspace wire patches

Complete standing/workspace wire-up on this branch tip (`standing.rs`, `workspace.rs`,
protocol `StandingGrant`, ports `STANDING`, docs, env notes already present).

```bash
./scripts/apply-workspace-wire.sh
```

Plain patches (preferred):
- `01-kernel-substrate-wire.patch` **or** `01a-kernel-wire.patch` + `01b-substrate-wire.patch`
- `02a-worker-api-wire.patch` **or** `02a-part{0..3}-worker-api-wire.patch`
- `02b-valhalla-bootstrap.patch`

Verified: sequential `git apply --check` on `feat/company-workspace-standing`.

After apply: reusable standing, DO standing storage, standing CRUD + `/organic/audit`,
Valhalla bootstrap on key create, CF inventory adapter, standing short-circuit on
submit, `/organic/repositories/sync-all`.

**Not live until deploy.** Needs wrangler auth + Node >=22 for curatom-kernel,
curatom-orchestrator, and curatom-valhalla. Do not delete Workers/D1.
