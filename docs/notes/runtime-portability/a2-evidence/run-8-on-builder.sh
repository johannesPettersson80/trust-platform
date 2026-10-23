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
out=/home/johannes/.cache/trust-portability-a2-evidence/run-8
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
# Final ST fixture setup correction and formatting; prior passing gates remain recorded.
run_step fragment-format required rustfmt --edition 2021 crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs
run_step fragment-format-check required rustfmt --check --edition 2021 crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs
run_step format-check required cargo fmt --all -- --check
run_step freeze-source required python3 /home/johannes/.cache/trust-portability-a2-setup/freeze-source-8.py
if (( $? != 0 )); then date -u +%FT%TZ > "$out/FINISHED"; exit 93; fi
run_step vm-core-tests required cargo test --locked -p trust-runtime --test bytecode_vm_core -- --nocapture --test-threads=1
run_step fixture-clippy required cargo clippy --locked -p trust-runtime --test bytecode_vm_core --all-features -- -D warnings
run_step runtime-cross-warnings required bash scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross
run_step retained-evidence required python3 /home/johannes/.cache/trust-portability-a2-setup/check-retained-evidence-8.py
run_step diff-integrity required git diff --check
printf 'required_failures=%s\n' "$required_failures" > "$out/result.txt"
date -u +%FT%TZ > "$out/FINISHED"
cat "$out/ledger.tsv"
(( required_failures == 0 ))
