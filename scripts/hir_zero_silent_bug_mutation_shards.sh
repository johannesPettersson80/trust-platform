#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:---list}"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.95}"

OUT_ROOT="target/gate-artifacts/hir-zero-mutants"
COMMON_ARGS=(
  -p trust-hir
  --timeout 120
  --minimum-test-timeout 20
  --jobs 1
  --baseline skip
  --caught
  --unviable
)

list_shard() {
  local name="$1"
  local file="$2"
  local regex="$3"
  local count
  count="$(cargo mutants -p trust-hir --file "$file" --re "$regex" --list --json | jq 'length')"
  printf '%s\t%s\t%s\n' "$name" "$count" "$file"
}

run_shard() {
  local name="$1"
  local file="$2"
  local regex="$3"
  shift 3
  cargo mutants \
    "${COMMON_ARGS[@]}" \
    --file "$file" \
    --re "$regex" \
    --output "$OUT_ROOT/$name" \
    -- "$@"
}

with_shard() {
  local name="$1"
  local file="$2"
  local regex="$3"
  shift 3
  case "$MODE" in
    --list)
      list_shard "$name" "$file" "$regex"
      ;;
    --run)
      mkdir -p "$OUT_ROOT"
      run_shard "$name" "$file" "$regex" "$@"
      ;;
    *)
      echo "usage: $0 [--list|--run]" >&2
      exit 2
      ;;
  esac
}

with_shard \
  semantic-outcome \
  crates/trust-hir/src/semantic.rs \
  'SemanticOutcome' \
  semantic::tests

with_shard \
  project-identity \
  crates/trust-hir/src/project.rs \
  'insert_with_id|normalize_path' \
  project::tests

with_shard \
  context-resolvers \
  crates/trust-hir/src/db/diagnostics/context.rs \
  'pou_context|expression_context|action_context|resolve_type_from_context' \
  db::diagnostics::context::tests

with_shard \
  is-assignable \
  crates/trust-hir/src/type_check/compatibility.rs \
  'is_assignable' \
  --test semantic_type_checking assignments_and_var_access

with_shard \
  type-check-expr \
  crates/trust-hir/src/type_check/expr.rs \
  'check_expression|infer_name_ref|infer_literal|common_numeric_type|common_bit_string_type|unary_bit_string_type' \
  --test semantic_type_checking

with_shard \
  type-check-calls \
  crates/trust-hir/src/type_check/calls.rs \
  'infer_call_expr|check_call_arguments|infer_function_call|infer_method_call' \
  --test semantic_type_checking

with_shard \
  type-check-const-eval \
  crates/trust-hir/src/type_check/const_eval.rs \
  'eval_const_int_expr_or_report|require_const_int_expr|try_eval_const_int_expr|report_const_int_eval_error' \
  --test semantic_type_checking hir_mutation_hardening

with_shard \
  collector-const-eval \
  crates/trust-hir/src/db/queries/collector/const_eval.rs \
  'try_eval_optional_int_expr_in_scope|try_eval_int_expr|report_const_eval_error' \
  --test semantic_type_checking

with_shard \
  symbol-import \
  crates/trust-hir/src/db/symbol_import.rs \
  'define_in_scope|import_type|record_import_collision|collision|cycle' \
  --test semantic_type_checking

STANDARD_RE='infer|common|unary|check|validate|call|expr|arg|string|bit|numeric|time|assert|compare|select'
for standard_file in \
  crates/trust-hir/src/type_check/standard/assertions.rs \
  crates/trust-hir/src/type_check/standard/bit.rs \
  crates/trust-hir/src/type_check/standard/comparison.rs \
  crates/trust-hir/src/type_check/standard/exprs.rs \
  crates/trust-hir/src/type_check/standard/numeric.rs \
  crates/trust-hir/src/type_check/standard/selection.rs \
  crates/trust-hir/src/type_check/standard/string.rs \
  crates/trust-hir/src/type_check/standard/time.rs
do
  base="$(basename "$standard_file" .rs)"
  with_shard \
    "type-check-standard-$base" \
    "$standard_file" \
    "$STANDARD_RE" \
    --test semantic_standard_functions
done
