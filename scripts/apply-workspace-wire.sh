#!/usr/bin/env bash
# Applies remaining workspace wire-up (kernel/substrate/worker/valhalla)
# onto a tree that already has standing.rs, workspace.rs, protocol StandingGrant, etc.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
git apply --index patches/01-kernel-substrate-wire.patch
git apply --index patches/02a-worker-api-wire.patch
git apply --index patches/02b-valhalla-bootstrap.patch
echo "Wired. Run tests, then deploy when wrangler auth + Node>=22 exist."
