# Sourced by the two task-owned builder scripts. These are not local commands.
cd /home/johannes/projects/trust-platform-portability-b || exit 1
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.95.0
export PYTHONDONTWRITEBYTECODE=1
unset RUSTC_BOOTSTRAP CARGO_ENCODED_RUSTFLAGS CC CXX GIT_INDEX_FILE
export RUSTC_WRAPPER=/usr/bin/env CARGO_BUILD_RUSTC_WRAPPER=/usr/bin/env
export SCCACHE_DISABLE=1
export RUSTFLAGS=-Dwarnings
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=6
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR=/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-b
export TMPDIR=/mnt/HC_Volume_107089260/builder-storage/tmp/portability-b
export B_EVIDENCE=/home/johannes/.cache/trust-portability-b-evidence/b-r4-run7
export B_SETUP=/home/johannes/.cache/trust-portability-br4-setup-run7
mkdir -p "$B_EVIDENCE" "$TMPDIR"
run_step() {
 local name=$1 classification=$2 start result state=PASS
 shift 2
 start=$(date -u +%FT%TZ)
 printf '%q ' "$@" > "$B_EVIDENCE/$name.command.txt"
 printf '\n' >> "$B_EVIDENCE/$name.command.txt"
 python3 scripts/run_with_progress.py --phase "B-R4-$name" --target "$(hostname)" --timeout-seconds 7200 --progress-interval-seconds 30 --log "$B_EVIDENCE/$name.txt" -- "$@"
 result=$?
 if (( result != 0 )); then state=FAIL; fi
 printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$classification" "$state" "$result" "$start" "$(date -u +%FT%TZ)" >> "$B_EVIDENCE/ledger.tsv"
 return "$result"
}
cargo_step() {
 local name=$1
 shift
 run_step "$name" required bash scripts/with_cargo_target_lease.sh "$CARGO_TARGET_DIR" "$@"
}
unrun() {
 printf '%s\trequired\tUNRUN\tprerequisite\t-\t-\n' "$1" >> "$B_EVIDENCE/ledger.tsv"
 printf '%s: %s\n' "$1" "$2" >> "$B_EVIDENCE/unrun.txt"
}
