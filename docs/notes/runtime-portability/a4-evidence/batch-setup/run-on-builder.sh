#!/usr/bin/env bash
set -uo pipefail
cd /home/johannes/projects/trust-platform-portability-a4
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.95.0
unset RUSTC_BOOTSTRAP CARGO_ENCODED_RUSTFLAGS CC CXX
export RUSTC_WRAPPER=""
export RUSTFLAGS=-Dwarnings
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=6
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR=/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a4
export TMPDIR=/mnt/HC_Volume_107089260/builder-storage/tmp/trust-portability-a4
out=/home/johannes/.cache/trust-portability-a4-evidence/run-1
setup=/home/johannes/.cache/trust-portability-a4-setup-run-1
mkdir -p "$out" "$TMPDIR"
if [[ -e "$out/STARTED" ]]; then echo 'Refusing automatic A4 batch rerun'; exit 91; fi
date -u +%FT%TZ > "$out/STARTED"
printf 'step\tclass\tstatus\texit\n' > "$out/ledger.tsv"
required_failures=0
run_step() {
 local step=$1 classification=$2
 shift 2
 printf '[A4] starting %s\n' "$step"
 printf '%q ' "$@" >> "$out/commands.txt"
 printf '\n' >> "$out/commands.txt"
 python3 scripts/run_with_progress.py --phase "A4-$step" --target "$(hostname)" --timeout-seconds 5400 --progress-interval-seconds 30 --log "$out/$step.txt" -- "$@"
 local result=$? state=PASS
 if (( result != 0 )); then
  state=FAIL
  if [[ "$classification" == required ]]; then required_failures=$((required_failures+1)); fi
 fi
 printf '%s\t%s\t%s\t%s\n' "$step" "$classification" "$state" "$result" >> "$out/ledger.tsv"
 return "$result"
}
unrun() {
 printf '%s\trequired\tUNRUN\tprerequisite\n' "$1" >> "$out/ledger.tsv"
 required_failures=$((required_failures+1))
}
{ hostname; pwd; date -u +%FT%TZ; git rev-parse HEAD; rustc -Vv; cargo -V; rustup target list --installed; nproc; free -h; df -h "$CARGO_TARGET_DIR" "$TMPDIR" /home/johannes; } > "$out/environment.txt"
planned_steps=(
 preflight staged-index format-prepare provenance-helper-tests provenance-refresh
 source-fixtures original-fixture-parity saved-numeric-fixture freeze-source
 core-all-features core-portable core-i686 f401-core c6-core f401-features c6-features
 runtime-unit runtime-integration mutation-tooling affected-clippy runtime-cross-warnings
 supply-chain architecture diagrams diagram-drift format-check diff-integrity metadata-index metadata
)
abort_prerequisite() {
 local reason=$1 code=$2
 for step in "${planned_steps[@]}"; do
  if ! awk -F '\t' -v step="$step" '$1 == step { found=1 } END { exit !found }' "$out/ledger.tsv"; then
   case "$step" in
    provenance-refresh|metadata-index|metadata) printf '%s\tadvisory\tUNRUN\tprerequisite\n' "$step" >> "$out/ledger.tsv" ;;
    *) unrun "$step" ;;
   esac
  fi
 done
 printf '%s\nrequired_failures=%s\n' "$reason" "$required_failures" > "$out/result.txt"
 date -u +%FT%TZ > "$out/FINISHED"
 cat "$out/ledger.tsv"
 exit "$code"
}
free_kib=$(df -Pk "$CARGO_TARGET_DIR" | awk 'NR==2 {print $4}')
if (( free_kib < 80 * 1024 * 1024 )); then
 printf 'preflight\trequired\tFAIL\t92\n' >> "$out/ledger.tsv"
 required_failures=$((required_failures+1))
 abort_prerequisite 'Target filesystem below 80 GiB; no builds launched' 92
fi
printf 'preflight\trequired\tPASS\t0\n' >> "$out/ledger.tsv"
# The inherited freeze helper includes working-tree/untracked records. Reject a
# staged index rather than silently omitting a staged-only source change.
run_step staged-index required git diff --cached --quiet
if (( $? != 0 )); then abort_prerequisite 'Unexpected staged index; source freeze prerequisite failed' 95; fi
run_step format-prepare required bash "$setup/format-once.sh"
format_ok=$?
if (( format_ok != 0 )); then
 abort_prerequisite 'Formatting preparation failed; code freeze and dependent checks unrun' 93
fi
run_step provenance-helper-tests required cargo test --locked -p verification-cases --example refresh_portability_provenance -- --nocapture
helper_ok=$?
if (( helper_ok == 0 )); then run_step provenance-refresh advisory bash scripts/refresh_a4_provenance.sh "$out/provenance"; else printf 'provenance-refresh\tadvisory\tUNRUN\tprerequisite\n' >> "$out/ledger.tsv"; fi
cp crates/trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc "$out/original-program-v2.stbc"
run_step source-fixtures required cargo run --locked -p trust-runtime --example portability_fixture
fixture_ok=$?
if (( fixture_ok == 0 )); then
 run_step original-fixture-parity required cmp "$out/original-program-v2.stbc" crates/trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc
 printf 'saved-numeric-fixture\trequired\tPASS\t0\n' >> "$out/ledger.tsv"
 sha256sum crates/trust-runtime/tests/fixtures/portability/stbc-2.0/*.stbc > "$out/generated-fixture-sha256.txt"
else
 unrun original-fixture-parity
 unrun saved-numeric-fixture
fi
run_step freeze-source required python3 "$setup/freeze-source.py"
if (( $? != 0 )); then
 abort_prerequisite 'Source freeze failed; no code validation launched' 94
fi
skip_numeric=()
if (( fixture_ok != 0 )); then skip_numeric=(--skip saved_numeric_artifact_preserves_accuracy_control_and_nominal_periods); fi
run_step core-all-features required cargo test --locked --no-fail-fast -p trust-runtime-core --all-features -- --nocapture --test-threads=1 "${skip_numeric[@]}"
run_step core-portable required cargo test --locked --no-fail-fast -p trust-runtime-core --no-default-features -- --nocapture --test-threads=1 "${skip_numeric[@]}"
run_step core-i686 required env RUSTFLAGS="-Dwarnings -C linker=/home/johannes/.rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld -C link-self-contained=yes" cargo test --locked --no-fail-fast -p trust-runtime-core --no-default-features --target i686-unknown-linux-musl --lib --test bytecode_decode_resource_bounds --test bytecode_source_free --test preparation_profile --test runtime_core_compiler_free_load --test portable_process_image -- --nocapture --test-threads=1
run_step f401-core required cargo check --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf
run_step c6-core required cargo check --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf
run_step f401-features required cargo tree --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf -e features,no-dev
run_step c6-features required cargo tree --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf -e features,no-dev
run_step runtime-unit required cargo test --locked --no-fail-fast -p trust-runtime --lib -- --nocapture --test-threads=1
run_step runtime-integration required cargo test --locked --no-fail-fast -p trust-runtime \
 --test runtime_core_compiler_free_load --test bytecode_source_free_authoring \
 --test source_free_assignment --test source_free_cycle_contract --test source_free_restart_graph --test source_free_readonly \
 --test bytecode_container --test bytecode_metadata --test bytecode_decode_resource_bounds --test bytecode_sections --test bytecode_optional_sections --test bytecode_encoder --test bytecode_roundtrip --test bytecode_validation --test bytecode_verification_cases \
 --test bytecode_vm_core --test bytecode_vm_differential --test phase11_seam_contract --test runtime_core_behavior_lock \
 --test vm_resource_limit_cases --test runtime_restart --test runtime_restart_trace_cases \
 --test vars_retain --test vars_access --test tasks --test tasks_fb --test scheduler_resource \
 --test var_init --test init_fail_closed --test initializer_architecture --test user_type_runtime_contract --test struct_initializers --test pou_interface --test pou_oop \
 --test portable_numeric_contract --test stdlib_core_contract --test stdlib_conversion_contract --test stdlib_helper_contract \
 --test stdlib_fb_contract --test stdlib_split_locals --test stdlib_enum_validate \
 --test stdlib_assertions --test stdlib_numeric --test stdlib_numeric_full \
 --test stdlib_conv --test stdlib_conv_full --test stdlib_bit_full \
 --test stdlib_select --test stdlib_select_full --test stdlib_string --test stdlib_string_full --test string_binding_bounds \
 --test process_image --test io_address --test io_cycle --test io_struct_array --test io_wildcard \
 --test io_fb_vars --test io_hierarchy --test io_driver \
 --test api_smoke --test debug_control --test complete_program --test runtime_reliability -- --nocapture --test-threads=1
run_step mutation-tooling required python3 -m unittest scripts.verification.focused_mutation_runner_tests
run_step affected-clippy required cargo clippy --locked -p trust-runtime-core -p trust-runtime -p verification-cases --all-targets --all-features -- -D warnings
run_step runtime-cross-warnings required env -u CC -u CXX bash scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross
run_step supply-chain required bash scripts/supply_chain_gate.sh
run_step architecture required cargo run --locked -p xtask -- architecture-doctor --full-map
architecture_ok=$?
if (( architecture_ok == 0 )); then
 run_step diagrams required bash scripts/render_diagrams.sh
 if (( $? == 0 )); then run_step diagram-drift required python3 scripts/check_diagram_drift.py; else unrun diagram-drift; fi
else unrun diagrams; unrun diagram-drift; fi
run_step format-check required bash "$setup/format-once.sh" --check
run_step diff-integrity required git diff --check
run_step metadata-index advisory python3 "$setup/prepare-metadata-index.py"
if (( $? == 0 )); then run_step metadata advisory env GIT_INDEX_FILE="$out/review-index" python3 scripts/validate_verification_metadata.py; else printf 'metadata\tadvisory\tUNRUN\tprerequisite\n' >> "$out/ledger.tsv"; fi
printf 'required_failures=%s\n' "$required_failures" > "$out/result.txt"
date -u +%FT%TZ > "$out/FINISHED"
cat "$out/ledger.tsv"
(( required_failures == 0 ))
