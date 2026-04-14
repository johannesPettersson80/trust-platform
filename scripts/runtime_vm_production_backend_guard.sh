#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

fail() {
  echo "[vm-production-guard] FAIL: $1"
  exit 1
}

has_rg() {
  command -v rg >/dev/null 2>&1
}

search_regex() {
  local regex="$1"
  shift
  if has_rg; then
    rg -n -e "${regex}" "$@"
  else
    grep -R -nE -- "${regex}" "$@"
  fi
}

expect_match() {
  local pattern="$1"
  local file="$2"
  local description="$3"
  if ! grep -F -q -- "${pattern}" "${file}"; then
    fail "${description} (missing: ${pattern} in ${file})"
  fi
}

expect_no_match() {
  local regex="$1"
  local description="$2"
  shift 2
  if search_regex "${regex}" "$@" >/dev/null; then
    echo "[vm-production-guard] unexpected match for ${description}:"
    search_regex "${regex}" "$@" || true
    fail "${description}"
  fi
}

if grep -n 'legacy-interpreter' crates/trust-runtime/Cargo.toml >/dev/null; then
  fail "legacy-interpreter feature must be removed from trust-runtime"
fi

if [[ -e crates/trust-runtime/src/runtime/backend.rs ]]; then
  fail "runtime/backend.rs must be removed in the VM-only runtime"
fi

expect_match   "runtime.execution_backend='interpreter' is no longer supported for production runtimes; use 'vm'"   "crates/trust-runtime/src/config/parser/validation/runtime/entry.rs"   "runtime config parser must explicitly reject interpreter backend"

expect_no_match   'ExecutionBackend::Interpreter|execute_program_interpreter|execute_function_block_ref_interpreter|InterpreterBackend|RuntimeExecutionBackend|legacy-interpreter'   "runtime source must not carry interpreter backend seams"   crates/trust-runtime/src/runtime crates/trust-runtime/src/execution_backend.rs

expect_no_match   'bench execution-backend|bytecode_vm_differential|--features legacy-interpreter|legacy-interpreter'   "runtime VM gate scripts must not depend on interpreter oracle tooling"   scripts/runtime_vm_bench_gate.sh scripts/runtime_vm_determinism_reliability_gate.sh .github/workflows/ci.yml

expect_no_match   'crate::eval::'   "VM modules must not depend on eval namespace"   crates/trust-runtime/src/runtime/vm

echo "[vm-production-guard] PASS"
