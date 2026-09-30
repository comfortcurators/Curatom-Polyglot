# Workspace wire patches

```bash
./scripts/apply-workspace-wire.sh
```

On this branch tip that already has `standing.rs`, `workspace.rs`, protocol/ports/docs:

| Patch | Status on branch |
|-------|------------------|
| `01a-part0` + `01a-part1` + `01b` | plain ✓ (kernel + substrate standing wire) |
| `02a-worker-api-wire.patch.gz.b64` | valid gzip+b64 ✓ (materialized by apply script) |
| `02b-valhalla-bootstrap.patch` | plain ✓ |
| `01-*.gz.b64` | **corrupt** — ignored; use 01a/01b splits |

After apply: reusable standing, DO standing storage, standing CRUD + `/organic/audit`, Valhalla bootstrap on key create, CF inventory adapter, standing short-circuit on submit, `/organic/repositories/sync-all`.

**Not live until deploy.** Needs wrangler auth + Node ≥22 for curatom-kernel / orchestrator / valhalla. Do not delete Workers/D1.
