#!/usr/bin/env bash
# Applies the remaining workspace wire-up onto a tree that already has
# standing.rs, workspace.rs, protocol StandingGrant, ports STANDING, etc.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
git apply --index patches/01-kernel-substrate-wire.patch
git apply --index patches/02-worker-valhalla-wire.patch
echo "Wired. Commit and deploy when wrangler auth + Node>=22 are available."
