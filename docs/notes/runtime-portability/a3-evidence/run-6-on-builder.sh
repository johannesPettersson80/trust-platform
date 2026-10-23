#!/usr/bin/env bash
set -uo pipefail
cd /home/johannes/projects/trust-platform-portability-a3
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.95.0
unset RUSTC_BOOTSTRAP CARGO_ENCODED_RUSTFLAGS CC CXX
export RUSTC_WRAPPER=""
export RUSTFLAGS=-Dwarnings
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=6
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR=/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a3
export TMPDIR=/mnt/HC_Volume_107089260/builder-storage/tmp/trust-portability-a3
out=/home/johannes/.cache/trust-portability-a3-evidence/run-6
setup=/home/johannes/.cache/trust-portability-a3-setup-run-6
mkdir -p "$out" "$TMPDIR"
if [[ -e "$out/STARTED" ]]; then echo 'Refusing automatic A3 batch rerun'; exit 91; fi
date -u +%FT%TZ > "$out/STARTED"
printf 'step\tclass\tstatus\texit\n' > "$out/ledger.tsv"
required_failures=0
run_step() {
  local step=$1 classification=$2
  shift 2
  printf '[A3] starting %s\n' "$step"
  printf '%q ' "$@" >> "$out/commands.txt"
  printf '\n' >> "$out/commands.txt"
  python3 scripts/run_with_progress.py --phase "A3-$step" --target "$(hostname)" --timeout-seconds 5400 --progress-interval-seconds 30 --log "$out/$step.txt" -- "$@"
  local result=$? state=PASS
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
{ hostname; pwd; date -u +%FT%TZ; git rev-parse HEAD; rustc -Vv; cargo -V; rustup target list --installed; nproc; free -h; df -h "$CARGO_TARGET_DIR" "$TMPDIR" /home/johannes; } > "$out/environment.txt"
free_kib=$(df -Pk "$CARGO_TARGET_DIR" | awk 'NR==2 { print $4 }')
if (( free_kib < 80 * 1024 * 1024 )); then echo 'Target filesystem below 80 GiB; no builds launched'; exit 92; fi
# Supply-chain and no-dev feature-tree evidence is reused from run 1: no manifests, lockfile or dependencies changed.
# Format once as batch preparation, including Rust fragments cargo fmt does not visit.
run_step format-prepare required bash "$setup/format-once.sh"
format_ok=$?
if (( format_ok != 0 )); then
  echo 'Formatting preparation failed; code checks remain unrun' > "$out/result.txt"
  date -u +%FT%TZ > "$out/FINISHED"
  exit 93
fi
run_step freeze-source required python3 "$setup/freeze-source.py"
if (( $? != 0 )); then
  echo 'Source freeze failed; code checks remain unrun' > "$out/result.txt"
  date -u +%FT%TZ > "$out/FINISHED"
  exit 94
fi
# Run5 covers unchanged production core/host sources, MCU checks, fixture bytes,
# full hosted integration/unit suites and architecture/rendering. Only test fixtures changed.
run_step source-authoring-tests required cargo test --locked --no-fail-fast -p trust-runtime --test bytecode_source_free_authoring -- --nocapture --test-threads=1
run_step authoring-boundary-tests required cargo test --locked --no-fail-fast -p trust-runtime --lib authoring_boundary_tests -- --nocapture --test-threads=1
run_step affected-clippy required cargo clippy --locked -p trust-runtime-core -p trust-runtime --all-targets --all-features -- -D warnings
run_step runtime-cross-warnings required bash scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross
run_step format-check required bash "$setup/format-once.sh" --check
run_step diff-integrity required git diff --check
run_step metadata-index advisory python3 "$setup/prepare-metadata-index.py"
if (( $? == 0 )); then
  run_step metadata advisory env GIT_INDEX_FILE="$out/review-index" python3 scripts/validate_verification_metadata.py
else
  printf 'metadata\tadvisory\tUNRUN\tprerequisite\n' >> "$out/ledger.tsv"
fi
printf 'required_failures=%s\n' "$required_failures" > "$out/result.txt"
date -u +%FT%TZ > "$out/FINISHED"
cat "$out/ledger.tsv"
(( required_failures == 0 ))
