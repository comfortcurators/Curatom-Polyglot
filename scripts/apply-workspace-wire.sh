#!/usr/bin/env bash
# Apply company-workspace wire patches. Prefers plain .patch over .gz.b64.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
apply_one() {
  local plain="$1" gz="$2"
  if [[ -f "$plain" ]]; then
    echo "Applying $plain"
    git apply --index "$plain"
  elif [[ -f "$gz" ]]; then
    echo "Applying $gz"
    if ! base64 -d "$gz" | gzip -d | git apply --index; then
      echo "ERROR: failed to decode/apply $gz. Commit the plain .patch instead." >&2
      exit 1
    fi
  else
    echo "missing $plain and $gz" >&2
    exit 1
  fi
}
apply_one patches/01-kernel-substrate-wire.patch patches/01-kernel-substrate-wire.patch.gz.b64
apply_one patches/02a-worker-api-wire.patch patches/02a-worker-api-wire.patch.gz.b64
apply_one patches/02b-valhalla-bootstrap.patch patches/02b-valhalla-bootstrap.patch
echo "Wired. Next: cargo check -p curatom-key-kernel (deploy needs wrangler auth + Node>=22)."
