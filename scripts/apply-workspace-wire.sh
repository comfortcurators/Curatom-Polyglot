#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
apply_one() {
  local plain="$1" gz="$2"
  if [[ -f "$plain" ]]; then
    git apply --index "$plain"
  elif [[ -f "$gz" ]]; then
    base64 -d "$gz" | gzip -d | git apply --index
  else
    echo "missing $plain and $gz" >&2; exit 1
  fi
}
apply_one patches/01-kernel-substrate-wire.patch patches/01-kernel-substrate-wire.patch.gz.b64
apply_one patches/02a-worker-api-wire.patch patches/02a-worker-api-wire.patch.gz.b64
apply_one patches/02b-valhalla-bootstrap.patch patches/02b-valhalla-bootstrap.patch
echo "Wired. Deploy when wrangler auth + Node>=22 exist."
