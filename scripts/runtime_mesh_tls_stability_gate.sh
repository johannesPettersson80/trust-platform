#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

ITERATIONS=8
ATTEMPTS_PER_RUN="${MESH_GATE_ATTEMPTS_PER_RUN:-2}"
OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-mesh-tls-stability}"
CHECK_LOG=""
if [[ "${1:-}" == "--check-log" ]]; then
  if [[ -z "${2:-}" ]]; then
    echo "Usage: $0 --check-log <run log>"
    exit 2
  fi
  CHECK_LOG="$2"
  shift 2
elif [[ "${1:-}" == "--iterations" ]]; then
  if [[ -z "${2:-}" ]]; then
    echo "Usage: $0 [--iterations <n>]"
    exit 2
  fi
  ITERATIONS="$2"
  shift 2
fi

if ! [[ "${ITERATIONS}" =~ ^[0-9]+$ ]] || [[ "${ITERATIONS}" -lt 1 ]]; then
  echo "[mesh-gate] invalid --iterations value: ${ITERATIONS}"
  exit 2
fi
if ! [[ "${ATTEMPTS_PER_RUN}" =~ ^[0-9]+$ ]] || [[ "${ATTEMPTS_PER_RUN}" -lt 1 ]]; then
  echo "[mesh-gate] invalid MESH_GATE_ATTEMPTS_PER_RUN value: ${ATTEMPTS_PER_RUN}"
  exit 2
fi

# The mesh TLS tests the gate repeats, one filter each. Every run must pass at least one test of each filter, so that a
# renamed or removed test fails the gate instead of passing it with nothing run (checklist W38: the gate filtered a test
# deleted long before, and every run reported "0 passed").
FILTERS=(
  "mesh::tests::mesh_tls"
  "mesh::security_tests::mesh_tls"
  "mesh::hmi_remote_tests::two_tls_peers"
)

# Whether the log of a run shows a passing test of every filter.
ran_every_filter() {
  local log_path="$1" filter
  for filter in "${FILTERS[@]}"; do
    if ! grep -Eq "^test ${filter}[A-Za-z0-9_:]* \.\.\. ok$" "${log_path}"; then
      echo "[mesh-gate] no passing test matched ${filter} in ${log_path}"
      return 1
    fi
  done
}

# `--check-log <log>` only checks a run's log, as the gate checks each run's.
if [[ -n "${CHECK_LOG}" ]]; then
  ran_every_filter "${CHECK_LOG}"
  exit $?
fi

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

echo "[mesh-gate] running the mesh TLS tests ${ITERATIONS} time(s), ${ATTEMPTS_PER_RUN} attempt(s) per run"
for i in $(seq 1 "${ITERATIONS}"); do
  echo "[mesh-gate] run ${i}/${ITERATIONS}"
  ok=0
  for attempt in $(seq 1 "${ATTEMPTS_PER_RUN}"); do
    if run_observed "runtime-mesh-tls-stability" "run-${i}-attempt-${attempt}" "${GATE_TEST_TIMEOUT_SECONDS:-600}" "${OUT_DIR}/run-${i}-attempt-${attempt}.log" \
      "${ROOT_DIR}/scripts/cargo_test_fast_link.sh" test -p trust-runtime --lib -- "${FILTERS[@]}" --nocapture; then
      if ! ran_every_filter "${OUT_DIR}/run-${i}-attempt-${attempt}.log"; then
        echo "[mesh-gate] FAIL: run ${i}/${ITERATIONS} passed without running every mesh TLS test"
        exit 1
      fi
      ok=1
      break
    fi
    if [[ "${attempt}" -lt "${ATTEMPTS_PER_RUN}" ]]; then
      echo "[mesh-gate] retry run ${i}/${ITERATIONS} (attempt ${attempt}/${ATTEMPTS_PER_RUN})"
      sleep 1
    fi
  done
  if [[ "${ok}" -ne 1 ]]; then
    echo "[mesh-gate] FAIL: the mesh TLS tests failed after ${ATTEMPTS_PER_RUN} attempt(s) on run ${i}/${ITERATIONS}"
    exit 1
  fi
done
echo "[mesh-gate] PASS"
