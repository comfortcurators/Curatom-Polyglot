#!/usr/bin/env bash
# Apply company-workspace wire patches. Prefers plain .patch over .gz.b64.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
apply_file() {
  local plain="$1" gz="$2"
  if [[ -f "$plain" ]]; then
    echo "Applying $plain"
    git apply --index "$plain"
  elif [[ -n "$gz" && -f "$gz" ]]; then
    echo "Applying $gz"
    if ! base64 -d "$gz" | gzip -d | git apply --index; then
      echo "ERROR: failed to decode/apply $gz. Commit the plain .patch instead." >&2
      exit 1
    fi
  else
    return 1
  fi
}
if [[ -f patches/01-kernel-substrate-wire.patch ]]; then
  apply_file patches/01-kernel-substrate-wire.patch patches/01-kernel-substrate-wire.patch.gz.b64
elif [[ -f patches/01a-kernel-wire.patch ]]; then
  apply_file patches/01a-kernel-wire.patch ""
  apply_file patches/01b-substrate-wire.patch ""
else
  echo "missing kernel/substrate wire patches" >&2; exit 1
fi
if [[ -f patches/02a-worker-api-wire.patch ]]; then
  apply_file patches/02a-worker-api-wire.patch patches/02a-worker-api-wire.patch.gz.b64
elif [[ -f patches/02a-part0-worker-api-wire.patch ]]; then
  for f in patches/02a-part*-worker-api-wire.patch; do
    apply_file "$f" ""
  done
else
  echo "missing 02a worker API wire patch" >&2; exit 1
fi
apply_file patches/02b-valhalla-bootstrap.patch "" || apply_file patches/02b-valhalla-bootstrap.patch patches/02b-valhalla-bootstrap.patch
echo "Wired. Next: cargo check -p curatom-key-kernel (deploy needs wrangler auth + Node>=22)."
