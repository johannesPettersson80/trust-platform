#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-comms-bench}"
SAMPLES="${TRUST_COMMS_BENCH_SAMPLES:-256}"

T0_P95_MAX_US="${TRUST_COMMS_T0_P95_MAX_US:-500}"
MESH_P95_MAX_US="${TRUST_COMMS_MESH_P95_MAX_US:-2000}"
DISPATCH_P95_MAX_US="${TRUST_COMMS_DISPATCH_P95_MAX_US:-3000}"

mkdir -p "${OUT_DIR}"

run_json_case() {
  local case_id="$1"
  local json_path="$2"
  shift 2
  python3 ./scripts/run_with_progress.py \
    --phase runtime-comms-bench \
    --target "${case_id}" \
    --timeout-seconds "${GATE_BENCH_TIMEOUT_SECONDS:-600}" \
    --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" \
    --log "${OUT_DIR}/${case_id}.log" \
    -- "$@" > "${json_path}"
}

echo "[bench-gate] running trust-runtime bench t0-shm"
run_json_case "t0-shm" "${OUT_DIR}/t0-shm.json" \
  cargo run -p trust-runtime --bin trust-runtime -- \
    bench t0-shm --samples "${SAMPLES}" --payload-bytes 32 --output json

echo "[bench-gate] running trust-runtime bench mesh-zenoh"
run_json_case "mesh-zenoh" "${OUT_DIR}/mesh-zenoh.json" \
  cargo run -p trust-runtime --bin trust-runtime -- \
    bench mesh-zenoh --samples "${SAMPLES}" --payload-bytes 64 --output json

echo "[bench-gate] running trust-runtime bench dispatch"
run_json_case "dispatch" "${OUT_DIR}/dispatch.json" \
  cargo run -p trust-runtime --bin trust-runtime -- \
    bench dispatch --samples "${SAMPLES}" --payload-bytes 32 --fanout 3 --output json

read_json_number() {
  local file="$1"
  local query="$2"
  jq -r "${query}" "${file}"
}

float_le() {
  local left="$1"
  local right="$2"
  awk -v a="${left}" -v b="${right}" 'BEGIN { exit (a <= b) ? 0 : 1 }'
}

t0_p95="$(read_json_number "${OUT_DIR}/t0-shm.json" '.report.round_trip_latency.p95_us')"
mesh_p95="$(read_json_number "${OUT_DIR}/mesh-zenoh.json" '.report.pub_sub_latency.p95_us')"
dispatch_p95="$(read_json_number "${OUT_DIR}/dispatch.json" '.report.end_to_end_latency.p95_us')"
fallback_denied="$(read_json_number "${OUT_DIR}/t0-shm.json" '.report.fallback_denied')"

if ! float_le "${t0_p95}" "${T0_P95_MAX_US}"; then
  echo "[bench-gate] FAIL: t0-shm p95 ${t0_p95}us exceeds threshold ${T0_P95_MAX_US}us"
  exit 1
fi
if ! float_le "${mesh_p95}" "${MESH_P95_MAX_US}"; then
  echo "[bench-gate] FAIL: mesh-zenoh p95 ${mesh_p95}us exceeds threshold ${MESH_P95_MAX_US}us"
  exit 1
fi
if ! float_le "${dispatch_p95}" "${DISPATCH_P95_MAX_US}"; then
  echo "[bench-gate] FAIL: dispatch p95 ${dispatch_p95}us exceeds threshold ${DISPATCH_P95_MAX_US}us"
  exit 1
fi
if [[ "${fallback_denied}" != "0" ]]; then
  echo "[bench-gate] FAIL: t0-shm fallback_denied must be 0 (got ${fallback_denied})"
  exit 1
fi

cat > "${OUT_DIR}/summary.md" <<MD
# Runtime Comms Bench Gate

- samples: ${SAMPLES}
- t0-shm round-trip p95: ${t0_p95} us (threshold ${T0_P95_MAX_US} us)
- mesh-zenoh pub/sub p95: ${mesh_p95} us (threshold ${MESH_P95_MAX_US} us)
- dispatch end-to-end p95: ${dispatch_p95} us (threshold ${DISPATCH_P95_MAX_US} us)
- t0 fallback_denied: ${fallback_denied}

Result: PASS
MD

jq -n \
  --argjson samples "${SAMPLES}" \
  --arg t0_p95 "${t0_p95}" \
  --arg mesh_p95 "${mesh_p95}" \
  --arg dispatch_p95 "${dispatch_p95}" \
  --arg t0_threshold "${T0_P95_MAX_US}" \
  --arg mesh_threshold "${MESH_P95_MAX_US}" \
  --arg dispatch_threshold "${DISPATCH_P95_MAX_US}" \
  --arg fallback_denied "${fallback_denied}" \
  '{
    samples: $samples,
    thresholds_us: {
      t0_shm_round_trip_p95: ($t0_threshold | tonumber),
      mesh_zenoh_pub_sub_p95: ($mesh_threshold | tonumber),
      dispatch_end_to_end_p95: ($dispatch_threshold | tonumber)
    },
    observed_us: {
      t0_shm_round_trip_p95: ($t0_p95 | tonumber),
      mesh_zenoh_pub_sub_p95: ($mesh_p95 | tonumber),
      dispatch_end_to_end_p95: ($dispatch_p95 | tonumber)
    },
    fallback_denied: ($fallback_denied | tonumber),
    result: "pass"
  }' > "${OUT_DIR}/summary.json"

echo "[bench-gate] PASS"
