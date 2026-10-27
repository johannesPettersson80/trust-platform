#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=runtime_test_common.sh
source "${SCRIPT_DIR}/runtime_test_common.sh"

ST_RUNTIME="${ST_RUNTIME:-trust-runtime}"
PROJECT="${1:-tests/fixtures/runtime_reliability_bundle}"
DURATION="${DURATION:-30}"
INTERVAL="${INTERVAL:-1}"
OUT="${OUT:-runtime-load-$(date +%Y%m%d_%H%M%S).log}"
BUILD_BEFORE_RUN="${BUILD_BEFORE_RUN:-true}"
TEMP_SRC_LINK=""

if [ ! -d "$PROJECT" ]; then
  echo "project folder not found: $PROJECT"
  exit 1
fi
if [ ! -f "$PROJECT/runtime.toml" ]; then
  echo "missing runtime.toml in project: $PROJECT"
  exit 1
fi
ST_RUNTIME="$(resolve_runtime_binary "$ST_RUNTIME")"
TEMP_SRC_LINK="$(prepare_project_sources_link "$PROJECT")"
if [ "$BUILD_BEFORE_RUN" = "true" ] || [ ! -f "$PROJECT/program.tpi" ]; then
  echo "Building the project's program before load test..."
  "$ST_RUNTIME" build --project "$PROJECT" >/dev/null
fi

echo "Starting runtime for load test..."
"$ST_RUNTIME" play --project "$PROJECT" >"${OUT}.runtime.log" 2>&1 &
PID=$!

cleanup() {
  "$ST_RUNTIME" ctl --project "$PROJECT" shutdown >/dev/null 2>&1 || true
  kill "$PID" >/dev/null 2>&1 || true
  cleanup_project_sources_link "$TEMP_SRC_LINK"
}
trap cleanup EXIT

sleep 1
echo "Collecting task stats every ${INTERVAL}s for ${DURATION}s..."
echo "# timestamp task stats" >"$OUT"

end=$(( $(date +%s) + DURATION ))
while [ "$(date +%s)" -lt "$end" ]; do
  ts="$(date --iso-8601=seconds)"
  if ! kill -0 "$PID" >/dev/null 2>&1; then
    echo "$ts runtime_exited=true stats=unavailable" >>"$OUT"
    echo "Runtime exited before load test completed."
    exit 1
  fi
  stats="$("$ST_RUNTIME" ctl --project "$PROJECT" stats 2>/dev/null || true)"
  if [ -z "$stats" ]; then
    echo "$ts stats=unavailable" >>"$OUT"
  else
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      echo "$ts $line" >>"$OUT"
    done <<< "$stats"
  fi
  sleep "$INTERVAL"
done

echo "Load test complete. Stats: $OUT"
