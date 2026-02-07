#!/usr/bin/env bash
set -euo pipefail

ST_RUNTIME="${ST_RUNTIME:-trust-runtime}"
PROJECT="${1:-tests/fixtures/runtime_reliability_bundle}"
DURATION_HOURS="${DURATION_HOURS:-24}"
DURATION_SECONDS="${DURATION_SECONDS:-}"
INTERVAL_SEC="${INTERVAL_SEC:-60}"
OUT="${OUT:-runtime-soak-$(date +%Y%m%d_%H%M%S).log}"
BUILD_BEFORE_RUN="${BUILD_BEFORE_RUN:-true}"

if [ ! -d "$PROJECT" ]; then
  echo "project folder not found: $PROJECT"
  exit 1
fi
if [ ! -f "$PROJECT/runtime.toml" ]; then
  echo "missing runtime.toml in project: $PROJECT"
  exit 1
fi
if [ "$BUILD_BEFORE_RUN" = "true" ] || [ ! -f "$PROJECT/program.stbc" ]; then
  echo "Building project bytecode before soak test..."
  "$ST_RUNTIME" build --project "$PROJECT" >/dev/null
fi

echo "Starting runtime for soak test..."
"$ST_RUNTIME" play --project "$PROJECT" >"${OUT}.runtime.log" 2>&1 &
PID=$!

cleanup() {
  "$ST_RUNTIME" ctl --project "$PROJECT" shutdown >/dev/null 2>&1 || true
  kill "$PID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

sleep 1
echo "# timestamp status cpu_pct mem_rss_kb process_alive" >"$OUT"

if [ -n "$DURATION_SECONDS" ]; then
  duration_seconds="$DURATION_SECONDS"
else
  duration_seconds=$(( DURATION_HOURS * 3600 ))
fi
end=$(( $(date +%s) + duration_seconds ))
unplanned_exits=0
while [ "$(date +%s)" -lt "$end" ]; do
  ts="$(date --iso-8601=seconds)"
  if ! kill -0 "$PID" >/dev/null 2>&1; then
    echo "$ts state=stopped cpu=0 mem_rss_kb=0 process_alive=false" >>"$OUT"
    unplanned_exits=$((unplanned_exits + 1))
    break
  fi
  status="$("$ST_RUNTIME" ctl --project "$PROJECT" status 2>/dev/null || echo "state=unknown")"
  cpu="$(ps -p "$PID" -o %cpu= | tr -d ' ')"
  rss="$(ps -p "$PID" -o rss= | tr -d ' ')"
  echo "$ts $status cpu=${cpu:-0} mem_rss_kb=${rss:-0} process_alive=true" >>"$OUT"
  sleep "$INTERVAL_SEC"
done

if [ "$unplanned_exits" -gt 0 ]; then
  echo "Soak test failed: runtime exited unexpectedly."
  exit 1
fi

echo "Soak test complete. Log: $OUT"
