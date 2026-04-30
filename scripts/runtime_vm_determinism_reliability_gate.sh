#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-vm-determinism}"
ITERATIONS="${TRUST_VM_DETERMINISM_ITERATIONS:-3}"
TEST_THREADS="${TRUST_VM_DETERMINISM_TEST_THREADS:-1}"

mkdir -p "${OUT_DIR}"

run_observed() {
  local phase="$1"
  local target="$2"
  local timeout="$3"
  local log_path="$4"
  shift 4
  python3 ./scripts/run_with_progress.py \
    --phase "${phase}" \
    --target "${target}" \
    --timeout-seconds "${timeout}" \
    --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" \
    --log "${log_path}" \
    -- "$@"
}

if [[ "${ITERATIONS}" -lt 2 ]]; then
  echo "[vm-determinism-gate] FAIL: TRUST_VM_DETERMINISM_ITERATIONS must be >= 2"
  exit 1
fi

hash_file() {
  local file="$1"
  sha256sum "${file}" | awk '{print $1}'
}

echo "[vm-determinism-gate] repeat-run VM behavior-lock suites (${ITERATIONS} iterations)"
suite_hashes=()
for run in $(seq 1 "${ITERATIONS}"); do
  log_path="${OUT_DIR}/vm-behavior-run-${run}.log"
  signature_path="${OUT_DIR}/vm-behavior-run-${run}.signature"
  json_path="${OUT_DIR}/vm-behavior-run-${run}.json"

  started_ns="$(date +%s%N)"
  run_observed "runtime-vm-determinism" "behavior-run-${run}" "${GATE_TEST_TIMEOUT_SECONDS:-1200}" "${log_path}" \
    cargo test -p trust-runtime --test api_smoke --test complete_program -- --test-threads="${TEST_THREADS}"
  ended_ns="$(date +%s%N)"
  duration_ms="$(( (ended_ns - started_ns) / 1000000 ))"

  grep '^test ' "${log_path}" \
    | sed -E 's/[[:space:]]+/ /g; s/finished in [0-9.]+s/finished in <time>/g' \
    > "${signature_path}"

  if [[ ! -s "${signature_path}" ]]; then
    echo "[vm-determinism-gate] FAIL: no VM behavior signatures captured for run ${run}"
    exit 1
  fi

  tests_hash="$(hash_file "${signature_path}")"
  suite_hashes+=("${tests_hash}")

  jq -n     --arg run "${run}"     --arg duration_ms "${duration_ms}"     --arg tests_hash "${tests_hash}"     '{
      run: ($run | tonumber),
      duration_ms: ($duration_ms | tonumber),
      tests_hash: $tests_hash
    }' > "${json_path}"
done

reference_hash="${suite_hashes[0]}"
for current_hash in "${suite_hashes[@]}"; do
  if [[ "${current_hash}" != "${reference_hash}" ]]; then
    echo "[vm-determinism-gate] FAIL: VM behavior-lock hash mismatch between runs"
    exit 1
  fi
done

echo "[vm-determinism-gate] runtime reliability suites"
runtime_reliability_log="${OUT_DIR}/runtime-reliability.log"
hot_reload_log="${OUT_DIR}/hot-reload.log"

started_ns="$(date +%s%N)"
run_observed "runtime-vm-determinism" "runtime-reliability" "${GATE_TEST_TIMEOUT_SECONDS:-1200}" "${runtime_reliability_log}" \
  cargo test -p trust-runtime --test runtime_reliability -- --test-threads=1
runtime_reliability_ms="$(( ($(date +%s%N) - started_ns) / 1000000 ))"

started_ns="$(date +%s%N)"
run_observed "runtime-vm-determinism" "hot-reload" "${GATE_TEST_TIMEOUT_SECONDS:-1200}" "${hot_reload_log}" \
  cargo test -p trust-runtime --test hot_reload -- --test-threads=1
hot_reload_ms="$(( ($(date +%s%N) - started_ns) / 1000000 ))"

cat > "${OUT_DIR}/summary.md" <<MD
# Runtime VM Determinism and Reliability Gate

- repeat-run iterations: ${ITERATIONS}
- behavior-lock test-set hash: ${reference_hash}
- runtime_reliability duration_ms: ${runtime_reliability_ms}
- hot_reload duration_ms: ${hot_reload_ms}

Checks:
- repeat-run VM behavior-lock suites: PASS
- fault/restart reliability suites: PASS

Result: PASS
MD

jq -n   --arg iterations "${ITERATIONS}"   --arg test_threads "${TEST_THREADS}"   --arg tests_hash "${reference_hash}"   --arg runtime_reliability_ms "${runtime_reliability_ms}"   --arg hot_reload_ms "${hot_reload_ms}"   '{
    vm_behavior_lock_repeat_runs: {
      iterations: ($iterations | tonumber),
      test_threads: ($test_threads | tonumber),
      tests_hash: $tests_hash
    },
    reliability_suites: {
      runtime_reliability_duration_ms: ($runtime_reliability_ms | tonumber),
      hot_reload_duration_ms: ($hot_reload_ms | tonumber)
    },
    result: "pass"
  }' > "${OUT_DIR}/summary.json"

echo "[vm-determinism-gate] PASS"
