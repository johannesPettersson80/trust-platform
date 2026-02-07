#!/usr/bin/env bash
set -euo pipefail

ST_RUNTIME="${ST_RUNTIME:-trust-runtime}"
PROJECT="${1:-tests/fixtures/runtime_reliability_bundle}"
DURATION_HOURS="${DURATION_HOURS:-24}"
DURATION_SECONDS="${DURATION_SECONDS:-}"
INTERVAL_SEC="${INTERVAL_SEC:-60}"
OUT="${OUT:-runtime-soak-$(date +%Y%m%d_%H%M%S).log}"
BUILD_BEFORE_RUN="${BUILD_BEFORE_RUN:-true}"
HMI_POLL_ENABLED="${HMI_POLL_ENABLED:-false}"
HMI_BASE_URL="${HMI_BASE_URL:-http://127.0.0.1:18080}"
HMI_TIMEOUT_SEC="${HMI_TIMEOUT_SEC:-5}"

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
if [ "$HMI_POLL_ENABLED" = "true" ]; then
  echo "Waiting for HMI endpoint at ${HMI_BASE_URL}/hmi ..."
  hmi_ready="false"
  for _ in $(seq 1 60); do
    if python3 - "$HMI_BASE_URL" "$HMI_TIMEOUT_SEC" <<'PY'
import sys
import urllib.error
import urllib.request

base = sys.argv[1].rstrip("/")
timeout = float(sys.argv[2])
try:
    with urllib.request.urlopen(f"{base}/hmi", timeout=timeout) as response:
        ok = 200 <= response.getcode() < 300
except (urllib.error.URLError, TimeoutError, ValueError):
    ok = False
sys.exit(0 if ok else 1)
PY
    then
      hmi_ready="true"
      break
    fi
    sleep 1
  done
  if [ "$hmi_ready" != "true" ]; then
    echo "HMI endpoint did not become ready: ${HMI_BASE_URL}/hmi"
    exit 1
  fi
fi

echo "# timestamp status cpu_pct mem_rss_kb process_alive hmi_status hmi_latency_ms" >"$OUT"

if [ -n "$DURATION_SECONDS" ]; then
  duration_seconds="$DURATION_SECONDS"
else
  duration_seconds=$(( DURATION_HOURS * 3600 ))
fi
end=$(( $(date +%s) + duration_seconds ))
unplanned_exits=0
hmi_errors=0
while [ "$(date +%s)" -lt "$end" ]; do
  ts="$(date --iso-8601=seconds)"
  if ! kill -0 "$PID" >/dev/null 2>&1; then
    echo "$ts state=stopped cpu=0 mem_rss_kb=0 process_alive=false hmi_status=stopped hmi_latency_ms=0" >>"$OUT"
    unplanned_exits=$((unplanned_exits + 1))
    break
  fi
  status="$("$ST_RUNTIME" ctl --project "$PROJECT" status 2>/dev/null || echo "state=unknown")"
  cpu="$(ps -p "$PID" -o %cpu= | tr -d ' ')"
  rss="$(ps -p "$PID" -o rss= | tr -d ' ')"
  hmi_status="disabled"
  hmi_latency_ms="0"
  if [ "$HMI_POLL_ENABLED" = "true" ]; then
    hmi_probe="$(python3 - "$HMI_BASE_URL" "$HMI_TIMEOUT_SEC" <<'PY'
import json
import sys
import time
import urllib.error
import urllib.request

base = sys.argv[1].rstrip("/")
timeout = float(sys.argv[2])
payload = json.dumps({"id": 1, "type": "hmi.values.get"}).encode("utf-8")
request = urllib.request.Request(
    f"{base}/api/control",
    data=payload,
    headers={"Content-Type": "application/json"},
    method="POST",
)
start = time.perf_counter()
status = "error"
try:
    with urllib.request.urlopen(request, timeout=timeout) as response:
        body = response.read().decode("utf-8")
    decoded = json.loads(body)
    status = "ok" if decoded.get("ok") is True else "error"
except (
    urllib.error.URLError,
    TimeoutError,
    ValueError,
    json.JSONDecodeError,
):
    status = "error"
elapsed_ms = (time.perf_counter() - start) * 1000.0
print(f"{status} {elapsed_ms:.3f}")
PY
)"
    hmi_status="$(awk '{print $1}' <<<"$hmi_probe")"
    hmi_latency_ms="$(awk '{print $2}' <<<"$hmi_probe")"
    if [ "$hmi_status" != "ok" ]; then
      hmi_errors=$((hmi_errors + 1))
    fi
  fi
  echo "$ts $status cpu=${cpu:-0} mem_rss_kb=${rss:-0} process_alive=true hmi_status=${hmi_status} hmi_latency_ms=${hmi_latency_ms}" >>"$OUT"
  sleep "$INTERVAL_SEC"
done

if [ "$unplanned_exits" -gt 0 ]; then
  echo "Soak test failed: runtime exited unexpectedly."
  exit 1
fi

if [ "$HMI_POLL_ENABLED" = "true" ] && [ "$hmi_errors" -gt 0 ]; then
  echo "Soak test failed: HMI polling errors detected: ${hmi_errors}"
  exit 1
fi

echo "Soak test complete. Log: $OUT"
