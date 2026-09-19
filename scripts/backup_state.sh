#!/usr/bin/env bash
# Owner-initiated backup of the Curatom DO's whole KernelState.
#
#   CURATOM_TOKEN=<RAJ_TOKEN or session cookie> ./scripts/backup_state.sh
#
# Writes one JSON file per run to backups/. Called before any deploy that
# touches the DO's storage format -- 5b's migration deleted the legacy
# blob on success, and until a backup procedure exists there is no way
# back from a bad migration except the bug fix. Reads /organic/admin/export
# (owner-only), so the credential must be the operator's own.
#
# To restore: the file is a `KernelState` JSON object. A restore path is
# not yet written -- this backup exists so a human can inspect or
# hand-recover state, not for automatic restore. If an automatic restore
# becomes necessary, it is its own part, and the file this script writes
# is its input.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT/backups"
mkdir -p "$OUT_DIR"

BASE_URL="${CURATOM_BASE_URL:-https://curatom.rajvansh.dev}"
TOKEN="${CURATOM_TOKEN:-}"

if [ -z "$TOKEN" ]; then
  echo "backup: CURATOM_TOKEN is required" >&2
  exit 2
fi

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT="$OUT_DIR/kernel_state_${STAMP}.json"

# The endpoint is owner-gated and accepts RAJ_TOKEN as a bearer, or a
# session cookie. Try bearer first -- if the caller pasted a session
# cookie instead, the second attempt handles it.
HTTP_CODE="$(curl -sS -o "$OUT.tmp" -w '%{http_code}' \
  -H "authorization: Bearer ${TOKEN}" \
  "${BASE_URL}/organic/admin/export")"

if [ "$HTTP_CODE" = "401" ]; then
  HTTP_CODE="$(curl -sS -o "$OUT.tmp" -w '%{http_code}' \
    -H "cookie: curatom_user_session=${TOKEN}" \
    "${BASE_URL}/organic/admin/export")"
fi

if [ "$HTTP_CODE" != "200" ]; then
  echo "backup: expected 200, got ${HTTP_CODE}" >&2
  cat "$OUT.tmp" >&2
  rm -f "$OUT.tmp"
  exit 1
fi

mv "$OUT.tmp" "$OUT"
BYTES="$(wc -c <"$OUT" | tr -d ' ')"
echo "backup: wrote ${OUT} (${BYTES} bytes)"
