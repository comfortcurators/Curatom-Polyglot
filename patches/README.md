# Workspace wire patches

These patches complete the standing/workspace wire-up against this branch tip
(which already includes `standing.rs`, `workspace.rs`, protocol `StandingGrant`,
ports `STANDING`, docs, and env notes).

```bash
./scripts/apply-workspace-wire.sh
```

Or manually:

```bash
# If using gzipped transfers:
base64 -d patches/01-kernel-substrate-wire.patch.gz.b64 | gzip -d > /tmp/01.patch && git apply --index /tmp/01.patch
base64 -d patches/02a-worker-api-wire.patch.gz.b64 | gzip -d > /tmp/02a.patch && git apply --index /tmp/02a.patch
git apply --index patches/02b-valhalla-bootstrap.patch

# Or if plain patches are present:
git apply --index patches/01-kernel-substrate-wire.patch
git apply --index patches/02a-worker-api-wire.patch
git apply --index patches/02b-valhalla-bootstrap.patch
```

Verified: `git apply --check` succeeds on `feat/company-workspace-standing`.

After apply you get reusable standing capabilities, DO standing storage, standing
CRUD + `/organic/audit`, Valhalla bootstrap on key create, CF inventory adapter,
standing short-circuit on submit, and `/organic/repositories/sync-all`.

Deploy still requires wrangler auth + Node ≥22. Do not delete Workers/D1.
