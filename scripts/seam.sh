#!/usr/bin/env bash
# Contract 2 + 3 seam. Two OS processes, not an in-process loop.
#
#   1. Rust signs a job envelope the way the Worker does.
#   2. Elixir accepts it, verifies attestation, mock-HostOS runs.
#   3. Elixir HMAC-POSTs /internal/outcome to a sink standing in for the Worker.
#   4. Same job, same process: ExecutionWorker replay-rejects; no second outcome.
#   5. Orchestrator restart: ETS is empty, so the same nonce is accepted again.
#      That is the deferred persistent-replay hole, observed, not faked green.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIR="$ROOT/scripts/.seam"
mkdir -p "$DIR"
trap 'kill $(jobs -p) 2>/dev/null || true' EXIT

export CURATOM_HMAC_KEY="${CURATOM_HMAC_KEY:-dGVzdC1obWFjLWtleS1jaGFuZ2UtbWUtaW4tcHJvZC0xMjM0NTY=}"
export PORT="${PORT:-14000}"
export SEAM_SINK_PORT="${SEAM_SINK_PORT:-18080}"
export CURATOM_WORKER_URL="http://127.0.0.1:${SEAM_SINK_PORT}"
export SEAM_OUTCOME_LOG="$DIR/outcomes.jsonl"
export MIX_ENV=dev
: >"$SEAM_OUTCOME_LOG"

if ! command -v mix >/dev/null; then
  echo "seam: mix not installed. Cannot run the Elixir side." >&2
  exit 2
fi

echo "seam: building sign_job"
cd "$ROOT"
cargo run -q -p curatom-attestation --example sign_job >/dev/null

sign_job() {
  local raw
  raw="$(mktemp)"
  cargo run -q -p curatom-attestation --example sign_job >"$raw"
  HMAC="$(sed -n '1s/^HMAC //p' "$raw")"
  BODY_FILE="$(mktemp)"
  tail -n +2 "$raw" >"$BODY_FILE"
  rm -f "$raw"
  if [ -z "$HMAC" ] || [ ! -s "$BODY_FILE" ]; then
    echo "seam: sign_job produced empty hmac or body" >&2
    exit 1
  fi
}


wait_outcomes() {
  local n="$1" timeout="${2:-15}" i=0
  while [ "$(grep -c . "$SEAM_OUTCOME_LOG" || true)" -lt "$n" ] && [ "$i" -lt "$timeout" ]; do
    sleep 1
    i=$((i + 1))
  done
  grep -c . "$SEAM_OUTCOME_LOG" || true
}

post_job() {
  curl -sS -o "$DIR/http.out" -w "%{http_code}" \
    -X POST "http://127.0.0.1:${PORT}/v1/jobs" \
    -H "content-type: application/json" \
    -H "x-curatom-hmac: ${HMAC}" \
    --data-binary @"$BODY_FILE"
}

start_sink() {
  python3 "$ROOT/scripts/outcome_sink.py" &
  SINK_PID=$!
  sleep 0.3
}

start_elixir() {
  cd "$ROOT/orchestrator"
  mix run --no-halt >/dev/null 2>"$DIR/elixir.err" &
  ELIXIR_PID=$!
  cd "$ROOT"
  for i in $(seq 1 30); do
    if curl -sS -o /dev/null -w "" -X POST "http://127.0.0.1:${PORT}/v1/jobs" \
         -H "content-type: application/json" --data '{}' 2>/dev/null; then
      # 401 (bad hmac) means the router is up
      return 0
    fi
    sleep 0.5
  done
  echo "seam: elixir did not bind :${PORT}" >&2
  cat "$DIR/elixir.err" >&2 || true
  exit 1
}

stop_elixir() {
  kill "$ELIXIR_PID" 2>/dev/null || true
  wait "$ELIXIR_PID" 2>/dev/null || true
}

echo "seam: mix deps + compile"
cd "$ROOT/orchestrator"
mix local.hex --force >/dev/null 2>&1 || true
mix local.rebar --force >/dev/null 2>&1 || true
mix deps.get >/dev/null
mix compile >/dev/null
cd "$ROOT"



start_sink
start_elixir

export SEAM_NONCE="nonce_seam_fixed"
export SEAM_JOB_ID="job_seam_fixed"
sign_job

echo "seam: first POST"
CODE="$(post_job)"
if [ "$CODE" != "202" ]; then
  echo "seam: expected 202, got $CODE body=$(cat "$DIR/http.out")" >&2
  exit 1
fi
N1="$(wait_outcomes 1 15)"
if [ "$N1" -lt 1 ]; then
  echo "seam: no outcome received. elixir.err:" >&2
  cat "$DIR/elixir.err" >&2 || true
  exit 1
fi
echo "seam: first outcome ok ($N1 line(s))"

echo "seam: in-process replay"
CODE="$(post_job)"
if [ "$CODE" != "202" ]; then
  echo "seam: replay POST expected 202 (router does not replay-guard), got $CODE" >&2
  exit 1
fi
sleep 3
N2="$(grep -c . "$SEAM_OUTCOME_LOG" || true)"
if [ "$N2" -ne "$N1" ]; then
  echo "seam: in-process replay produced another outcome ($N1 -> $N2)" >&2
  cat "$SEAM_OUTCOME_LOG" >&2
  exit 1
fi
echo "seam: in-process replay rejected (still $N2 outcome(s))"

echo "seam: restart orchestrator (ETS dies)"
stop_elixir
start_elixir
CODE="$(post_job)"
if [ "$CODE" != "202" ]; then
  echo "seam: post-restart POST expected 202, got $CODE" >&2
  exit 1
fi
N3="$(wait_outcomes $((N1 + 1)) 15)"
if [ "$N3" -le "$N1" ]; then
  echo "seam: unexpected: replay survived restart. N1=$N1 N3=$N3" >&2
  echo "seam: that would mean ReplayGuard persisted. It is not supposed to in v0." >&2
  exit 1
fi
echo "seam: post-restart replay executed again ($N1 -> $N3). ETS hole confirmed, as DEFERRED.md says."

echo
echo "SEAM PASS"
echo "  contract 2: elixir accepted a rust-signed job"
echo "  contract 3: hmac outcome landed on the worker-stand-in"
echo "  in-process replay: no second outcome"
echo "  across restart: second outcome (persistent replay still deferred)"
exit 0
