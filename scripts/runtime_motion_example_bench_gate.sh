#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

source "${ROOT_DIR}/scripts/runtime_host_codegen.sh"

PROJECT="${PROJECT:-examples/plcopen_motion_single_axis_demo}"
OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-motion-example-bench}"
SAMPLES="${TRUST_MOTION_BENCH_SAMPLES:-128}"
WARMUP_CYCLES="${TRUST_MOTION_BENCH_WARMUP_CYCLES:-32}"

mkdir -p "${OUT_DIR}"

BUILD_MODE="$(trust_runtime_detect_host_codegen_mode)"
REPORT_JSON="${OUT_DIR}/motion-example-bench.json"

echo "[motion-bench-gate] build mode: ${BUILD_MODE}"
trust_runtime_build_release_binary "${BUILD_MODE}"
BENCH_CMD=("$(trust_runtime_release_binary_path)")
echo "[motion-bench-gate] running trust-runtime bench project on ${PROJECT}"
"${BENCH_CMD[@]}" \
  bench project \
  --project "${PROJECT}" \
  --samples "${SAMPLES}" \
  --warmup-cycles "${WARMUP_CYCLES}" \
  --watch g_motion_demo_completed_sequences \
  --watch g_motion_demo_last_error \
  --watch g_motion_demo_limit_clamp_verified \
  --watch g_motion_demo_power_on \
  --watch g_motion_demo_is_homed \
  --output json \
  > "${REPORT_JSON}"

read_json() {
  local query="$1"
  jq -r "${query}" "${REPORT_JSON}"
}

float_le() {
  local left="$1"
  local right="$2"
  awk -v a="${left}" -v b="${right}" 'BEGIN { exit (a <= b) ? 0 : 1 }'
}

completed_sequences="$(read_json '.report.watched_globals.g_motion_demo_completed_sequences // 0')"
last_error="$(read_json '.report.watched_globals.g_motion_demo_last_error // 0')"
clamp_verified="$(read_json '.report.watched_globals.g_motion_demo_limit_clamp_verified // false')"
power_on="$(read_json '.report.watched_globals.g_motion_demo_power_on // false')"
is_homed="$(read_json '.report.watched_globals.g_motion_demo_is_homed // false')"
p95_us="$(read_json '.report.cycle_latency.p95_us')"
cycle_budget_us="$(read_json '.report.cycle_budget_us')"
budget_overruns="$(read_json '.report.budget_overruns')"

p95_limit_us="${TRUST_MOTION_P95_MAX_US:-${cycle_budget_us}}"
max_overruns="${TRUST_MOTION_MAX_OVERRUNS:-0}"

if [[ "${completed_sequences}" -lt 1 ]]; then
  echo "[motion-bench-gate] FAIL: completed sequences must be > 0 (got ${completed_sequences})"
  exit 1
fi
if [[ "${last_error}" != "0" ]]; then
  echo "[motion-bench-gate] FAIL: last error must be 0 (got ${last_error})"
  exit 1
fi
if [[ "${clamp_verified}" != "true" ]]; then
  echo "[motion-bench-gate] FAIL: limit clamp must be verified (got ${clamp_verified})"
  exit 1
fi
if [[ "${power_on}" != "true" ]]; then
  echo "[motion-bench-gate] FAIL: axis must remain powered on (got ${power_on})"
  exit 1
fi
if [[ "${is_homed}" != "true" ]]; then
  echo "[motion-bench-gate] FAIL: axis must remain homed (got ${is_homed})"
  exit 1
fi
if [[ "${budget_overruns}" -gt "${max_overruns}" ]]; then
  echo "[motion-bench-gate] FAIL: budget overruns ${budget_overruns} exceed limit ${max_overruns}"
  exit 1
fi
if ! float_le "${p95_us}" "${p95_limit_us}"; then
  echo "[motion-bench-gate] FAIL: p95 ${p95_us}us exceeds limit ${p95_limit_us}us"
  exit 1
fi

cat > "${OUT_DIR}/summary.md" <<MD
# Motion Example Bench Gate

- project: ${PROJECT}
- build mode: ${BUILD_MODE}
- samples: ${SAMPLES}
- warmup cycles: ${WARMUP_CYCLES}
- completed sequences: ${completed_sequences}
- last error: ${last_error}
- limit clamp verified: ${clamp_verified}
- power on: ${power_on}
- is homed: ${is_homed}
- cycle budget: ${cycle_budget_us} us
- cycle p95: ${p95_us} us
- cycle p95 limit: ${p95_limit_us} us
- budget overruns: ${budget_overruns}
- budget overrun limit: ${max_overruns}

Result: PASS
MD

jq -n \
  --arg project "${PROJECT}" \
  --arg build_mode "${BUILD_MODE}" \
  --argjson samples "${SAMPLES}" \
  --argjson warmup_cycles "${WARMUP_CYCLES}" \
  --arg completed_sequences "${completed_sequences}" \
  --arg last_error "${last_error}" \
  --arg clamp_verified "${clamp_verified}" \
  --arg power_on "${power_on}" \
  --arg is_homed "${is_homed}" \
  --arg p95_us "${p95_us}" \
  --arg p95_limit_us "${p95_limit_us}" \
  --arg cycle_budget_us "${cycle_budget_us}" \
  --arg budget_overruns "${budget_overruns}" \
  --arg max_overruns "${max_overruns}" \
  '{
    project: $project,
    build_mode: $build_mode,
    samples: $samples,
    warmup_cycles: $warmup_cycles,
    watched_globals: {
      completed_sequences: ($completed_sequences | tonumber),
      last_error: ($last_error | tonumber),
      limit_clamp_verified: ($clamp_verified == "true"),
      power_on: ($power_on == "true"),
      is_homed: ($is_homed == "true")
    },
    cycle_budget_us: ($cycle_budget_us | tonumber),
    cycle_p95_us: ($p95_us | tonumber),
    cycle_p95_limit_us: ($p95_limit_us | tonumber),
    budget_overruns: ($budget_overruns | tonumber),
    budget_overrun_limit: ($max_overruns | tonumber),
    result: "pass"
  }' > "${OUT_DIR}/summary.json"

echo "[motion-bench-gate] PASS"
