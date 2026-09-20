#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run() { printf '\n==> %s\n' "$*"; "$@"; }
need() { command -v "$1" >/dev/null || { echo "missing release tool: $1" >&2; exit 127; }; }

# A release check is intentionally strict. Missing tooling is a failure,
# not a green skip. Install the release toolchain before tagging.
for tool in cargo rustup trunk mix node npm python3; do need "$tool"; done
rustup target list --installed | grep -qx wasm32-unknown-unknown || {
  echo "missing Rust target: wasm32-unknown-unknown" >&2
  exit 127
}

run cargo fmt --all -- --check
run cargo test --workspace --locked
run cargo check --manifest-path workers/api/Cargo.toml --target wasm32-unknown-unknown --locked
run cargo fmt --manifest-path workers/api/Cargo.toml -- --check

(
  cd apps/dashboard
  run cargo fmt -- --check
  run trunk build --release
)

(
  cd orchestrator
  run mix test
)

(
  cd workers/valhalla
  run npm ci --ignore-scripts --no-audit --no-fund
  run npx tsc --noEmit
)

run node --check workers/orchestrator/src/index.js
run python3 -m py_compile scripts/outcome_sink.py
for f in scripts/*.sh; do run bash -n "$f"; done

if [[ -x scripts/seam.sh ]]; then run scripts/seam.sh; fi

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  run git diff --check
fi

printf '\nLocal release checks passed.\n'
printf '%s\n' 'Still manual: full-history/object secret scan, third-party review,' \
  'real Cloudflare routing/storage probes, and final production smoke test.'
