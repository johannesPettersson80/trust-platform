#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:---list}"
SHARD_FILTER="${2:-}"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.95}"

OUT_ROOT="${OUT_DIR:-target/gate-artifacts/runtime-vm-mutants}"
COMMON_ARGS=(
  -p trust-runtime
  --timeout 120
  --minimum-test-timeout 20
  --baseline skip
  --caught
  --unviable
  --no-times
)

if [[ "${TRUST_VM_MUTANTS_IN_PLACE:-0}" == "1" ]]; then
  if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    echo "TRUST_VM_MUTANTS_IN_PLACE=1 requires a clean tracked worktree" >&2
    exit 2
  fi
  COMMON_ARGS+=(--in-place)
else
  COMMON_ARGS+=(--gitignore true --jobs 1)
fi

list_shard() {
  local name="$1"
  local file="$2"
  local regex="$3"
  mkdir -p "$OUT_ROOT/lists"
  local list_path="$OUT_ROOT/lists/$name.json"
  cargo mutants -p trust-runtime --file "$file" --re "$regex" --list --json \
    | tee "$list_path" \
    | jq -r --arg name "$name" --arg file "$file" '"\($name)\t\(length)\t\($file)"'
}

run_shard() {
  local name="$1"
  local file="$2"
  local regex="$3"
  local target_spec="$4"
  shift 4
  local target_args=()
  case "$target_spec" in
    lib)
      target_args=(--cargo-arg --lib)
      ;;
    test:*)
      target_args=(--cargo-arg --test --cargo-arg "${target_spec#test:}")
      ;;
    *)
      echo "unknown cargo target spec for shard '$name': $target_spec" >&2
      exit 2
      ;;
  esac
  cargo mutants \
    "${COMMON_ARGS[@]}" \
    "${target_args[@]}" \
    --file "$file" \
    --re "$regex" \
    --output "$OUT_ROOT/$name" \
    -- "$@"
}

with_shard() {
  local name="$1"
  local file="$2"
  local regex="$3"
  local target_spec="$4"
  shift 4
  if [[ -n "$SHARD_FILTER" && "$SHARD_FILTER" != "$name" ]]; then
    return 0
  fi
  case "$MODE" in
    --list)
      list_shard "$name" "$file" "$regex"
      ;;
    --run)
      mkdir -p "$OUT_ROOT"
      run_shard "$name" "$file" "$regex" "$target_spec" "$@"
      ;;
    *)
      echo "usage: $0 [--list|--run] [shard-name]" >&2
      exit 2
      ;;
  esac
}

ALL_MUTANTS='.'

with_shard \
  call-root \
  crates/trust-runtime/src/runtime/vm/call.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core

with_shard \
  call-bindings \
  crates/trust-runtime/src/runtime/vm/call/bindings.rs \
  "$ALL_MUTANTS" \
  lib \
  runtime::vm::call::tests

with_shard \
  call-stdlib \
  crates/trust-runtime/src/runtime/vm/call/stdlib.rs \
  "$ALL_MUTANTS" \
  lib \
  runtime::vm::call::tests

with_shard \
  call-symbols \
  crates/trust-runtime/src/runtime/vm/call/symbols.rs \
  "$ALL_MUTANTS" \
  lib \
  runtime::vm::call::tests

with_shard \
  dispatch-root \
  crates/trust-runtime/src/runtime/vm/dispatch.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core

with_shard \
  dispatch-refs \
  crates/trust-runtime/src/runtime/vm/dispatch_refs.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core

with_shard \
  dispatch-sizeof \
  crates/trust-runtime/src/runtime/vm/dispatch_sizeof.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core

with_shard \
  register-ir-root \
  crates/trust-runtime/src/runtime/vm/register_ir.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-interpreter \
  crates/trust-runtime/src/runtime/vm/register_ir/interpreter.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-lower-root \
  crates/trust-runtime/src/runtime/vm/register_ir/lower.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-lower-decode \
  crates/trust-runtime/src/runtime/vm/register_ir/lower/decode.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-lower-fuse \
  crates/trust-runtime/src/runtime/vm/register_ir/lower/fuse.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-lower-verify \
  crates/trust-runtime/src/runtime/vm/register_ir/lower/verify.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-tier1-root \
  crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-tier1-compile \
  crates/trust-runtime/src/runtime/vm/register_ir/tier1/compile.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-tier1-execute \
  crates/trust-runtime/src/runtime/vm/register_ir/tier1/execute.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  register-ir-tier1-state \
  crates/trust-runtime/src/runtime/vm/register_ir/tier1/state.rs \
  "$ALL_MUTANTS" \
  lib \
  register_ir::tests

with_shard \
  vm-stack \
  crates/trust-runtime/src/runtime/vm/stack.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core

with_shard \
  memory-references \
  crates/trust-runtime/src/memory/references.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core

with_shard \
  memory-frames \
  crates/trust-runtime/src/memory/frames.rs \
  "$ALL_MUTANTS" \
  test:bytecode_vm_core
