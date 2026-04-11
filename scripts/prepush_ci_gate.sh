#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=1
fi

LOG_DIR="logs/prepush-gate-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "${LOG_DIR}"

run_step() {
  local id="$1"
  local name="$2"
  shift 2
  local log_file="${LOG_DIR}/${id}.log"

  echo
  echo "=== ${id}: ${name} ==="
  echo "LOG: ${log_file}"
  echo "CMD: $*"

  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "RESULT: DRY-RUN"
    return 0
  fi

  if "$@" 2>&1 | tee "${log_file}"; then
    echo "RESULT: PASS"
    return 0
  fi

  echo "RESULT: FAIL"
  echo "Pre-push gate failed on step ${id}: ${name}"
  exit 1
}

echo "Pre-push CI gate started at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Dry-run: ${DRY_RUN}"

run_step "01" "Path hygiene guard" ./scripts/check_test_path_hygiene.sh
run_step "02" "IEC log path guard" python3 ./scripts/check_iec_log_paths.py
run_step "03" "Rust fmt check" cargo fmt --all --check
run_step "04" "Clippy deny warnings (trust-hir + trust-lsp)" cargo clippy -p trust-hir -p trust-lsp -- -D warnings
run_step "05" "trust-lsp unit/integration tests" cargo test -p trust-lsp --bin trust-lsp
run_step "06" "trust-runtime cross-target warning gate" ./scripts/check_runtime_cross_target_warnings.sh
run_step "07" "trust-runtime mesh TLS stability gate" ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8

if [[ "${DRY_RUN}" -eq 0 ]] && ! rustup target list --installed | grep -q '^x86_64-pc-windows-gnu$'; then
  echo "Missing Rust target x86_64-pc-windows-gnu."
  echo "Install it with: rustup target add x86_64-pc-windows-gnu"
  exit 1
fi

run_step "08" "Windows test compile check (trust-lsp)" cargo check -p trust-lsp --tests --target x86_64-pc-windows-gnu

echo
echo "All pre-push gates passed."
