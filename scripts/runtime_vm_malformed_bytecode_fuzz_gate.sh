#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

ARTIFACT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-vm-malformed-bytecode-fuzz}"
mkdir -p "${ARTIFACT_DIR}"

echo "[vm-malformed-fuzz-gate] malformed bytecode fuzz smoke"
started_ns="$(date +%s%N)"
python3 ./scripts/run_with_progress.py \
  --phase runtime-vm-malformed-bytecode-fuzz \
  --target malformed-bytecode-fuzz-smoke \
  --timeout-seconds "${GATE_TEST_TIMEOUT_SECONDS:-900}" \
  --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" \
  --log "${ARTIFACT_DIR}/malformed-bytecode-fuzz-smoke.log" \
  -- env -u OUT_DIR cargo test -p trust-runtime --test bytecode_vm_core vm_malformed_bytecode_fuzz_smoke_budget -- --nocapture
duration_ms="$(( ($(date +%s%N) - started_ns) / 1000000 ))"

cat > "${ARTIFACT_DIR}/summary.md" <<MD
# Runtime VM Malformed Bytecode Fuzz Gate

- test: \`cargo test -p trust-runtime --test bytecode_vm_core vm_malformed_bytecode_fuzz_smoke_budget -- --nocapture\`
- duration_ms: ${duration_ms}

Checks:
- deterministic malformed-bytecode mutation smoke: PASS

Result: PASS
MD

jq -n \
  --arg duration_ms "${duration_ms}" \
  '{
    malformed_bytecode_fuzz_smoke: {
      duration_ms: ($duration_ms | tonumber),
      test: "cargo test -p trust-runtime --test bytecode_vm_core vm_malformed_bytecode_fuzz_smoke_budget -- --nocapture"
    },
    result: "pass"
  }' > "${ARTIFACT_DIR}/summary.json"

echo "[vm-malformed-fuzz-gate] PASS"
