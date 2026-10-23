#!/usr/bin/env bash
set -uo pipefail
source /home/johannes/.cache/trust-portability-br4-setup-run8-validation2/environment.sh
PARENT=/home/johannes/.cache/trust-portability-b-evidence/b-r4-run8
# Explicitly recorded second validation attempt: first stopped at metadata parity before tests.
[[ ! -e "$B_EVIDENCE/validation-started.txt" ]] || exit 91
[[ $(cat "$PARENT/measurement-reviewed.txt") == $(sha256sum "$PARENT/formatted-source-manifest.json" | cut -d' ' -f1) ]] || exit 93
sha256sum --check "$B_SETUP/changed-source.sha256" || exit 94
date -u +%FT%TZ > "$B_EVIDENCE/validation-started.txt"
printf 'step\tclass\tstatus\texit\tstart\tend\n' > "$B_EVIDENCE/ledger.tsv"
# Preparation froze implementation, unchanged inputs and refreshed provenance before these suites.
# Shared core and hosted source/locks/artifacts are byte-identical to passing run7.
run_step reused-source-parity required sha256sum --check "$B_SETUP/reused-source.sha256"
if (( $? != 0 )); then exit 96; fi
cargo_step native-tools cargo test --locked -p trust-platform-stm32f4 -p xtask --no-fail-fast -- --nocapture --test-threads=1
cargo_step native-bundle cargo test --locked --manifest-path firmware/trust-nucleo-f401re/Cargo.toml --lib --target x86_64-unknown-linux-gnu -- --nocapture
cargo_step f401-adapter cargo check --locked -p trust-platform-stm32f4 --target thumbv7em-none-eabihf
cargo_step affected-clippy cargo clippy --locked -p trust-platform-stm32f4 -p xtask --all-targets --all-features -- -D warnings
cargo_step firmware-clippy env -u RUSTFLAGS bash -c 'cd firmware/trust-nucleo-f401re && cargo clippy --locked --release -- -D warnings'
# Runtime/cross-target/audit proof remains run7; no source or dependency change there.
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
