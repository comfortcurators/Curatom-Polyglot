#!/usr/bin/env bash
# Apply company-workspace wire patches. Prefers plain .patch; materializes from .patch.b64 if needed.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
for f in patches/*.patch.b64; do
  [[ -e "$f" ]] || continue
  out="${f%.b64}"
  if [[ ! -f "$out" ]]; then
    echo "Materializing $out from $f"
    base64 -d "$f" > "$out"
  fi
done
apply_file() {
  local plain="$1" gz="${2:-}"
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
    echo "missing $plain ${gz:+and $gz}" >&2
    exit 1
  fi
}
if [[ -f patches/01-kernel-substrate-wire.patch ]]; then
  apply_file patches/01-kernel-substrate-wire.patch patches/01-kernel-substrate-wire.patch.gz.b64
elif [[ -f patches/01a-kernel-wire.patch ]]; then
  apply_file patches/01a-kernel-wire.patch
  apply_file patches/01b-substrate-wire.patch
elif [[ -f patches/01a-part0-kernel-wire.patch ]]; then
  for f in patches/01a-part*-kernel-wire.patch; do apply_file "$f"; done
  apply_file patches/01b-substrate-wire.patch
else
  echo "missing kernel/substrate wire patches" >&2; exit 1
fi
if [[ -f patches/02a-worker-api-wire.patch ]]; then
  apply_file patches/02a-worker-api-wire.patch patches/02a-worker-api-wire.patch.gz.b64
elif [[ -f patches/02a-part0-worker-api-wire.patch ]]; then
  for f in patches/02a-part*-worker-api-wire.patch; do apply_file "$f"; done
else
  echo "missing 02a worker API wire patch" >&2; exit 1
fi
apply_file patches/02b-valhalla-bootstrap.patch
echo "Wired. Next: cargo check -p curatom-key-kernel (deploy needs wrangler auth + Node>=22)."
