#!/usr/bin/env bash
set -uo pipefail
source /home/johannes/.cache/trust-portability-br4-setup-run5/environment.sh
if [[ -e "$B_EVIDENCE/validation-started.txt" ]]; then echo 'Refusing automatic B-R4 validation rerun'; exit 91; fi
[[ -s "$B_EVIDENCE/preparation-finished.txt" ]] || { echo 'Preparation/freeze incomplete'; exit 92; }
# Root records independent approval of this exact post-format/provenance manifest.
[[ -s "$B_EVIDENCE/review-approved.txt" ]] || { echo 'Exact frozen-source review incomplete'; exit 93; }
[[ $(cat "$B_EVIDENCE/review-approved.txt") == $(sha256sum "$B_EVIDENCE/formatted-source-manifest.json" | cut -d' ' -f1) ]] || { echo 'Review does not match frozen source manifest'; exit 93; }
# The source-frozen measurement already linked this candidate once.
[[ -s "$B_EVIDENCE/measurement-stage-finished.txt" && -s "$B_EVIDENCE/measurement-reviewed.txt" && -s "$B_EVIDENCE/elf-report.json" ]] || exit 95
[[ $(cat "$B_EVIDENCE/measurement-reviewed.txt") == $(sha256sum "$B_EVIDENCE/formatted-source-manifest.json" | cut -d' ' -f1) ]] || exit 95
# Owner explicitly authorized this additional correction batch after B-R3 hardware failed.
[[ ! -e "$HOME/.cache/trust-portability-b-evidence/b-r4-run5-validation-started.txt" ]] || exit 94
date -u +%FT%TZ > "$HOME/.cache/trust-portability-b-evidence/b-r4-run5-validation-started.txt"
date -u +%FT%TZ > "$B_EVIDENCE/validation-started.txt"
# Preparation froze implementation, unchanged inputs and refreshed provenance before these suites.
cargo_step core-all-features cargo test --locked --no-fail-fast -p trust-runtime-core --all-features -- --nocapture --test-threads=1
cargo_step core-portable cargo test --locked --no-fail-fast -p trust-runtime-core --no-default-features -- --nocapture --test-threads=1
cargo_step core-i686 env RUSTFLAGS="-Dwarnings -C linker=/home/johannes/.rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld -C link-self-contained=yes" cargo test --locked --no-fail-fast -p trust-runtime-core --no-default-features --target i686-unknown-linux-musl --lib --test bytecode_decode_resource_bounds --test bytecode_source_free --test preparation_profile --test preparation_transport --test preparation_allocation_profile --test runtime_core_compiler_free_load --test portable_process_image -- --nocapture --test-threads=1
cargo_step f401-core cargo check --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf
cargo_step c6-core cargo check --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf
cargo_step f401-features cargo tree --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf -e features,no-dev
cargo_step c6-features cargo tree --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf -e features,no-dev
# Call-boundary codegen correction; run4 retains full hosted unit/integration evidence.
cargo_step runtime-integration cargo test --locked --no-fail-fast -p trust-runtime \
 --test source_free_execution_cost --test runtime_core_compiler_free_load \
 --test source_free_assignment --test source_free_cycle_contract --test source_free_restart_graph \
 --test source_free_blocks_and_io --test bytecode_vm_core --test bytecode_vm_differential \
 --test var_init --test init_fail_closed --test struct_initializers --test pou_interface --test pou_oop \
 --test api_smoke --test debug_control --test complete_program --test runtime_reliability -- --nocapture --test-threads=1
cargo_step native-tools cargo test --locked -p trust-platform-stm32f4 -p xtask --no-fail-fast -- --nocapture --test-threads=1
cargo_step native-bundle cargo test --locked --manifest-path firmware/trust-nucleo-f401re/Cargo.toml --lib --target x86_64-unknown-linux-gnu -- --nocapture
cargo_step f401-adapter cargo check --locked -p trust-platform-stm32f4 --target thumbv7em-none-eabihf
cargo_step affected-clippy cargo clippy --locked -p trust-runtime-core -p trust-runtime -p trust-platform-stm32f4 -p xtask -p verification-cases --all-targets --all-features -- -D warnings
cargo_step portable-clippy cargo clippy --locked -p trust-runtime-core --no-default-features --all-targets -- -D warnings
cargo_step firmware-clippy env -u RUSTFLAGS bash -c 'cd firmware/trust-nucleo-f401re && cargo clippy --locked --release -- -D warnings'
cargo_step cross-warnings env -u CC -u CXX bash scripts/check_runtime_cross_target_warnings.sh --require-cross
cargo_step supply-chain bash scripts/supply_chain_gate.sh
cargo_step provenance-helper-tests cargo test --locked -p verification-cases --example refresh_portability_provenance -- --nocapture
run_step metadata-index required python3 "$B_SETUP/prepare-metadata-index.py"
metadata_index_status=$?
run_step mutation-tooling required env -u GIT_INDEX_FILE python3 -m unittest scripts.verification.focused_mutation_runner_tests
if (( metadata_index_status == 0 )); then
 run_step mutation-contracts required env GIT_INDEX_FILE="$B_EVIDENCE/review-index" python3 -m unittest scripts.verification.mutation_program_contract_tests
 run_step metadata advisory env GIT_INDEX_FILE="$B_EVIDENCE/review-index" python3 scripts/validate_verification_metadata.py
else unrun mutation-contracts 'Metadata review-index creation failed'; fi
cargo_step architecture cargo xtask architecture-doctor --full-map
architecture_status=$?
if (( architecture_status == 0 )); then
 run_step diagrams required bash scripts/render_diagrams.sh
 if (( $? == 0 )); then run_step diagram-drift required python3 scripts/check_diagram_drift.py; else unrun diagram-drift 'Diagram rendering failed'; fi
else unrun diagrams 'Architecture prerequisite failed'; unrun diagram-drift 'Architecture prerequisite failed'; fi
cargo_step format-check bash "$B_SETUP/format-once.sh" --check
run_step diff-check required git diff --check
cargo_step pack cargo xtask portability pack crates/trust-runtime/tests/fixtures/portability/f401/gpio.stbc "$B_EVIDENCE/application.bin"
run_step artifact-parity required bash -c 'sha256sum --check "$B_SETUP/fixture-sha256.txt" || exit; for f in program-v2.stbc numeric-v2.stbc; do p=crates/trust-runtime/tests/fixtures/portability/stbc-2.0/$f; git show HEAD:"$p" | sha256sum; sha256sum "$p"; cmp <(git show HEAD:"$p") "$p" || exit; done'
# The coordinator may proceed to the local board phase only on every required PASS.
software_failures=$(awk -F '\t' 'NR>1 && $2=="required" && $3!="PASS" {n++} END {print n+0}' "$B_EVIDENCE/ledger.tsv")
printf 'software_required_failed_or_unrun=%s\n' "$software_failures" > "$B_EVIDENCE/result.txt"
date -u +%FT%TZ > "$B_EVIDENCE/builder-finished.txt"
cat "$B_EVIDENCE/ledger.tsv"
(( software_failures == 0 ))
