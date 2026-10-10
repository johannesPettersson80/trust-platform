#!/usr/bin/env bash
set -uo pipefail
cd /home/johannes/projects/trust-platform-portability-a2
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.95.0
unset RUSTC_BOOTSTRAP CARGO_ENCODED_RUSTFLAGS
export RUSTC_WRAPPER=""
export RUSTFLAGS=-Dwarnings
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=6
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR=/home/johannes/.cache/codex-targets/trust-platform-gate
export TMPDIR=/home/johannes/.cache/trust-portability-a2-setup/tmp
out=/home/johannes/.cache/trust-portability-a2-evidence/run-6
mkdir -p "$out" "$TMPDIR"
if [[ -e "$out/STARTED" ]]; then echo 'Refusing automatic A2 batch rerun'; exit 91; fi
date -u +%FT%TZ > "$out/STARTED"
printf 'step\tclass\tstatus\texit\n' > "$out/ledger.tsv"
required_failures=0
run_step() {
  local step=$1 classification=$2
  shift 2
  printf '[A2] starting %s\n' "$step"
  printf '%q ' "$@" >> "$out/commands.txt"
  printf '\n' >> "$out/commands.txt"
  python3 scripts/run_with_progress.py --phase "A2-$step" --target "$(hostname)" --timeout-seconds 3600 --progress-interval-seconds 30 --log "$out/$step.log" -- "$@"
  local result=$?
  local state=PASS
  if (( result != 0 )); then
    state=FAIL
    if [[ "$classification" == required ]]; then required_failures=$((required_failures + 1)); fi
  fi
  printf '%s\t%s\t%s\t%s\n' "$step" "$classification" "$state" "$result" >> "$out/ledger.tsv"
  return "$result"
}
unrun() {
  printf '%s\trequired\tUNRUN\tprerequisite\n' "$1" >> "$out/ledger.tsv"
  required_failures=$((required_failures + 1))
}
{ hostname; pwd; date -u +%FT%TZ; git rev-parse HEAD; rustc -Vv; cargo -V; rustup target list --installed; nproc; free -h; df -h /home/johannes; } > "$out/environment.txt"
free_kib=$(df -Pk /home/johannes | awk 'NR==2 { print $4 }')
if (( free_kib < 25 * 1024 * 1024 )); then echo 'Disk below 25 GiB; no builds launched'; exit 92; fi
# All implementation/review precedes this one authorized correction batch.
run_step format-check required cargo fmt --all -- --check
run_step mutation-selector-refresh advisory python3 /home/johannes/.cache/trust-portability-a2-setup/refresh-mutation-selectors-6.py
run_step freeze-source required python3 /home/johannes/.cache/trust-portability-a2-setup/freeze-source-6.py
if (( $? != 0 )); then
  echo 'Source freeze failed; no dependent checks launched' > "$out/result.txt"
  date -u +%FT%TZ > "$out/FINISHED"
  exit 93
fi
run_step portable-i686-tests required env RUSTFLAGS="-Dwarnings -C linker=/home/johannes/.rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld -C link-self-contained=yes" cargo test --locked --no-fail-fast -p trust-runtime-core --no-default-features --target i686-unknown-linux-musl --lib --test bytecode_container --test bytecode_decode_resource_bounds --test bytecode_sections --test bytecode_portable_load --test bytecode_validation_budget --test bytecode_validation_scale -- --nocapture --test-threads=1
run_step f401-core required cargo check --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf
run_step c6-core required cargo check --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf
run_step f401-features required cargo tree --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf -e features,no-dev
run_step c6-features required cargo tree --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf -e features,no-dev
run_step core-tests required cargo test --locked --no-fail-fast -p trust-runtime-core --all-features -- --nocapture --test-threads=1
run_step portable-loader-tests required cargo test --locked --no-fail-fast -p trust-runtime-core --no-default-features --test bytecode_container --test bytecode_decode_resource_bounds --test bytecode_sections --test bytecode_portable_load --test bytecode_validation_budget --test bytecode_validation_scale -- --nocapture --test-threads=1
run_step runtime-tests required cargo test --locked --no-fail-fast -p trust-runtime --test bytecode_container --test bytecode_metadata --test bytecode_decode_resource_bounds --test bytecode_sections --test process_image --test bytecode_validation --test bytecode_optional_sections --test bytecode_encoder --test bytecode_roundtrip --test bytecode_verification_cases --test bytecode_vm_core --test bytecode_vm_differential --test runtime_core_behavior_lock --test api_smoke --test debug_control --test complete_program --test runtime_reliability --test vars_access --test vm_resource_limit_cases --test phase11_seam_contract -- --nocapture --test-threads=1
run_step runtime-unit-tests required cargo test --locked --no-fail-fast -p trust-runtime --lib -- --nocapture --test-threads=1
run_step metadata-index required python3 /home/johannes/.cache/trust-portability-a2-setup/prepare-metadata-index-6.py
metadata_index_ready=$?
if (( metadata_index_ready == 0 )); then
  run_step mutation-provenance-tooling required env GIT_INDEX_FILE="$out/review-index" python3 -m unittest scripts.verification.bytecode_validator_mutation_tests scripts.verification.mutation_program_contract_tests scripts.verification.mutation_program_report_tests scripts.verification.gate_inventory_tests scripts.verification.metadata_validator.suite_contracts_tests scripts.verification.fuzz_program_source_contract_tests
else
  unrun mutation-provenance-tooling
fi
run_step affected-clippy required cargo clippy --locked -p trust-runtime-core -p trust-runtime --all-targets --all-features -- -D warnings
run_step runtime-cross-warnings required bash scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross
run_step supply-chain required bash scripts/supply_chain_gate.sh
run_step architecture required cargo run --locked -p xtask -- architecture-doctor --full-map
architecture_ok=$?
if (( architecture_ok == 0 )); then
  run_step diagrams required bash scripts/render_diagrams.sh
  run_step diagram-drift required python3 scripts/check_diagram_drift.py
  tar -czf "$out/rendered-diagrams.tar.gz" docs/diagrams
else
  unrun diagrams
  unrun diagram-drift
fi
run_step retained-evidence required python3 /home/johannes/.cache/trust-portability-a2-setup/check-retained-evidence-6.py
run_step diff-integrity required git diff --check
if (( metadata_index_ready == 0 )); then
  run_step metadata advisory env GIT_INDEX_FILE="$out/review-index" python3 scripts/validate_verification_metadata.py
else
  printf 'metadata\tadvisory\tUNRUN\tprerequisite\n' >> "$out/ledger.tsv"
fi
printf 'required_failures=%s\n' "$required_failures" > "$out/result.txt"
date -u +%FT%TZ > "$out/FINISHED"
cat "$out/ledger.tsv"
(( required_failures == 0 ))
