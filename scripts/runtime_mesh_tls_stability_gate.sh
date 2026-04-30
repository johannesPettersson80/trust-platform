#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

ITERATIONS=8
ATTEMPTS_PER_RUN="${MESH_GATE_ATTEMPTS_PER_RUN:-2}"
OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-mesh-tls-stability}"
if [[ "${1:-}" == "--iterations" ]]; then
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

echo "[mesh-gate] running mesh TLS publish regression ${ITERATIONS} time(s), ${ATTEMPTS_PER_RUN} attempt(s) per run"
for i in $(seq 1 "${ITERATIONS}"); do
  echo "[mesh-gate] run ${i}/${ITERATIONS}"
  ok=0
  for attempt in $(seq 1 "${ATTEMPTS_PER_RUN}"); do
    if run_observed "runtime-mesh-tls-stability" "run-${i}-attempt-${attempt}" "${GATE_TEST_TIMEOUT_SECONDS:-600}" "${OUT_DIR}/run-${i}-attempt-${attempt}.log" \
      cargo test -p trust-runtime --lib mesh::tests::mesh_tls_publish_applies_updates -- --nocapture; then
      ok=1
      break
    fi
    if [[ "${attempt}" -lt "${ATTEMPTS_PER_RUN}" ]]; then
      echo "[mesh-gate] retry run ${i}/${ITERATIONS} (attempt ${attempt}/${ATTEMPTS_PER_RUN})"
      sleep 1
    fi
  done
  if [[ "${ok}" -ne 1 ]]; then
    echo "[mesh-gate] FAIL: mesh TLS publish regression failed after ${ATTEMPTS_PER_RUN} attempt(s) on run ${i}/${ITERATIONS}"
    exit 1
  fi
done
echo "[mesh-gate] PASS"
