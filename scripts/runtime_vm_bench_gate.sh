#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

BENCH_OUT_DIR="${TRUST_VM_BENCH_ARTIFACT_DIR:-${OUT_DIR:-target/gate-artifacts/runtime-vm-bench}}"
PROFILE="${TRUST_VM_BENCH_PROFILE:-quick}"
TIER="${TRUST_VM_BENCH_TIER:-default}"
HOST_CODEGEN="${TRUST_VM_BENCH_HOST_CODEGEN:-generic}"
LOW_NOISE_RUNS=1

case "${PROFILE}" in
  quick)
    SAMPLES="${TRUST_VM_BENCH_SAMPLES:-32}"
    WARMUP_CYCLES="${TRUST_VM_BENCH_WARMUP_CYCLES:-8}"
    ;;
  quick-low-noise)
    SAMPLES="${TRUST_VM_BENCH_SAMPLES:-32}"
    WARMUP_CYCLES="${TRUST_VM_BENCH_WARMUP_CYCLES:-8}"
    LOW_NOISE_RUNS="${TRUST_VM_BENCH_LOW_NOISE_RUNS:-3}"
    ;;
  full)
    SAMPLES="${TRUST_VM_BENCH_SAMPLES:-128}"
    WARMUP_CYCLES="${TRUST_VM_BENCH_WARMUP_CYCLES:-32}"
    ;;
  full-low-noise)
    SAMPLES="${TRUST_VM_BENCH_SAMPLES:-128}"
    WARMUP_CYCLES="${TRUST_VM_BENCH_WARMUP_CYCLES:-32}"
    LOW_NOISE_RUNS="${TRUST_VM_BENCH_LOW_NOISE_RUNS:-3}"
    ;;
  *)
    echo "[vm-bench-gate] FAIL: unsupported profile '${PROFILE}' (expected quick|quick-low-noise|full|full-low-noise)"
    exit 1
    ;;
esac

mkdir -p "${BENCH_OUT_DIR}"

# Avoid leaking gate OUT_DIR into cargo/rustc build script env.
unset OUT_DIR

if ! [[ "${LOW_NOISE_RUNS}" =~ ^[0-9]+$ ]] || [[ "${LOW_NOISE_RUNS}" -lt 1 ]]; then
  echo "[vm-bench-gate] FAIL: TRUST_VM_BENCH_LOW_NOISE_RUNS must be a positive integer"
  exit 1
fi

CORPUS_SUMMARY_ARGS=()
if [[ "${LOW_NOISE_RUNS}" -eq 1 ]]; then
  echo "[vm-bench-gate] capturing VM syntax corpus benchmark (profile=${PROFILE}, tier=${TIER}, host_codegen=${HOST_CODEGEN})"
  python3 ./scripts/run_with_progress.py \
    --phase runtime-vm-bench \
    --target "syntax-corpus-${PROFILE}-${TIER}" \
    --timeout-seconds "${GATE_BENCH_TIMEOUT_SECONDS:-1800}" \
    --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" \
    --log "${BENCH_OUT_DIR}/gate.log" \
    -- env \
      TRUST_RUNTIME_HOST_CODEGEN="${HOST_CODEGEN}" \
      OUT_DIR="${BENCH_OUT_DIR}" \
      TRUST_VM_SYNTAX_CORPUS_SAMPLES="${SAMPLES}" \
      TRUST_VM_SYNTAX_CORPUS_WARMUP_CYCLES="${WARMUP_CYCLES}" \
      TRUST_VM_SYNTAX_CORPUS_TIER="${TIER}" \
      ./scripts/runtime_vm_syntax_corpus.sh
  CORPUS_SUMMARY_ARGS+=(--corpus-summary "${BENCH_OUT_DIR}/${TIER}/summary.json")
else
  for run_index in $(seq 1 "${LOW_NOISE_RUNS}"); do
    RUN_OUT_DIR="${BENCH_OUT_DIR}/runs/run-${run_index}"
    echo "[vm-bench-gate] capturing VM syntax corpus benchmark run ${run_index}/${LOW_NOISE_RUNS} (profile=${PROFILE}, tier=${TIER}, host_codegen=${HOST_CODEGEN})"
    python3 ./scripts/run_with_progress.py \
      --phase runtime-vm-bench \
      --target "syntax-corpus-${PROFILE}-${TIER}-run-${run_index}" \
      --timeout-seconds "${GATE_BENCH_TIMEOUT_SECONDS:-1800}" \
      --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" \
      --log "${BENCH_OUT_DIR}/gate-run-${run_index}.log" \
      -- env \
        TRUST_RUNTIME_HOST_CODEGEN="${HOST_CODEGEN}" \
        OUT_DIR="${RUN_OUT_DIR}" \
        TRUST_VM_SYNTAX_CORPUS_SAMPLES="${SAMPLES}" \
        TRUST_VM_SYNTAX_CORPUS_WARMUP_CYCLES="${WARMUP_CYCLES}" \
        TRUST_VM_SYNTAX_CORPUS_TIER="${TIER}" \
        ./scripts/runtime_vm_syntax_corpus.sh
    CORPUS_SUMMARY_ARGS+=(--corpus-summary "${RUN_OUT_DIR}/${TIER}/summary.json")
  done
fi

COMPARE_ARGS=()
if [[ -n "${TRUST_VM_BENCH_COMPARE_BASELINE:-}" ]]; then
  COMPARE_ARGS=(--compare-baseline "${TRUST_VM_BENCH_COMPARE_BASELINE}")
fi

python3 ./scripts/runtime_vm_bench_summary.py \
  --out-dir "${BENCH_OUT_DIR}" \
  --profile "${PROFILE}" \
  --tier "${TIER}" \
  --samples "${SAMPLES}" \
  --warmup-cycles "${WARMUP_CYCLES}" \
  --low-noise-runs "${LOW_NOISE_RUNS}" \
  "${COMPARE_ARGS[@]}" \
  "${CORPUS_SUMMARY_ARGS[@]}"

echo "[vm-bench-gate] RECORDED"
