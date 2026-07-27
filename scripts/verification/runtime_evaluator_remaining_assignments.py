"""Generate reviewed assignments for the remaining host evaluator tests."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


BehaviorKey = tuple[str, str]
TestBinding = tuple[str, str, str]

TEST_BINDINGS: Mapping[str, TestBinding] = {
    "bind_split_args_rejects_unnamed_named_call_without_panic": (
        "crates/trust-runtime/src/host/eval/expr/call/tests.rs",
        "RT_HOST_POU_EXECUTION_001",
        "SPLIT_UNNAMED_AFTER_NAMED_REJECTION",
    ),
    "bind_stdlib_named_args_rejects_unnamed_arg_without_panic": (
        "crates/trust-runtime/src/host/eval/expr/call/tests.rs",
        "RT_HOST_POU_EXECUTION_001",
        "STDLIB_UNNAMED_NAMED_ARGUMENT_REJECTION",
    ),
    "index_and_field": (
        "crates/trust-runtime/src/host/eval/tests/expr_access.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "INDEX_AND_FIELD_ACCESS",
    ),
    "nested_index_and_field_chains": (
        "crates/trust-runtime/src/host/eval/tests/expr_access.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "NESTED_INDEX_FIELD_CHAINS",
    ),
    "short_circuit": (
        "crates/trust-runtime/src/host/eval/tests/expr_bool.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "BOOLEAN_SHORT_CIRCUIT",
    ),
    "mixed_numeric_ops": (
        "crates/trust-runtime/src/host/eval/tests/expr_coercion.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "MIXED_NUMERIC_OPERATIONS",
    ),
    "iec_7_3_2": (
        "crates/trust-runtime/src/host/eval/tests/expr_full.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "REPRESENTATIVE_EXPRESSION_PIPELINE",
    ),
    "super_uses_parent_instance": (
        "crates/trust-runtime/src/host/eval/tests/expr_full.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "SUPER_PARENT_INSTANCE",
    ),
    "literal_eval": (
        "crates/trust-runtime/src/host/eval/tests/expr_literals.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "LITERAL_EVALUATION",
    ),
    "name_ref_eval": (
        "crates/trust-runtime/src/host/eval/tests/expr_literals.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "NAME_REFERENCE_EVALUATION",
    ),
    "time_arithmetic_and_compare": (
        "crates/trust-runtime/src/host/eval/tests/expr_time_ops.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "TIME_ARITHMETIC_AND_COMPARISON",
    ),
    "debug_hook_fires_once_per_statement": (
        "crates/trust-runtime/src/host/eval/tests/mod.rs",
        "DEBUG_CONTROL_EXECUTION_001",
        "STATEMENT_HOOK_ONCE_PER_STATEMENT",
    ),
    "en_eno_semantics": (
        "crates/trust-runtime/src/host/eval/tests/pou_en_eno.rs",
        "RT_HOST_POU_EXECUTION_001",
        "EN_FALSE_SKIPS_BODY",
    ),
    "param_binding": (
        "crates/trust-runtime/src/host/eval/tests/pou_params.rs",
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_OUTPUT_INOUT_BINDING",
    ),
    "default_null_reference": (
        "crates/trust-runtime/src/host/eval/tests/reference.rs",
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "REFERENCE_NULL_DEFAULT",
    ),
    "ref_and_deref": (
        "crates/trust-runtime/src/host/eval/tests/reference.rs",
        "RT_HOST_EXPRESSION_EVALUATION_001",
        "REFERENCE_DEREFERENCE_WRITE",
    ),
    "assignment": (
        "crates/trust-runtime/src/host/eval/tests/stmt_basic.rs",
        "RT_HOST_STATEMENT_EXECUTION_001",
        "ASSIGNMENT_CONTINUE",
    ),
    "case_labels": (
        "crates/trust-runtime/src/host/eval/tests/stmt_case.rs",
        "RT_HOST_STATEMENT_EXECUTION_001",
        "CASE_NUMERIC_SELECTION",
    ),
    "case_string_labels": (
        "crates/trust-runtime/src/host/eval/tests/stmt_case.rs",
        "RT_HOST_STATEMENT_EXECUTION_001",
        "CASE_STRING_SELECTION",
    ),
    "if_branches": (
        "crates/trust-runtime/src/host/eval/tests/stmt_if.rs",
        "RT_HOST_STATEMENT_EXECUTION_001",
        "IF_BRANCH_SELECTION",
    ),
    "loop_control": (
        "crates/trust-runtime/src/host/eval/tests/stmt_loops.rs",
        "RT_HOST_STATEMENT_EXECUTION_001",
        "FOR_WHILE_REPEAT_CONTROL",
    ),
    "default_result": (
        "crates/trust-runtime/src/host/eval/tests/stmt_return.rs",
        "RT_HOST_STATEMENT_EXECUTION_001",
        "RETURN_VALUE",
    ),
    "create_fb_instance_honors_declared_var_input_initializer": (
        "crates/trust-runtime/src/host/instance.rs",
        "RT_HOST_POU_EXECUTION_001",
        "FB_DECLARED_INPUT_INITIALIZER",
    ),
}

REVIEWED_ORACLE_EXPRESSIONS: Mapping[str, tuple[str, ...]] = {
    "bind_split_args_rejects_unnamed_named_call_without_panic": (
        'assert ! (matches ! (result , Err (RuntimeError :: InvalidArgumentName (name)) if name . as_str () == "<unnamed>"))',
    ),
    "bind_stdlib_named_args_rejects_unnamed_arg_without_panic": (
        'assert ! (matches ! (result , Err (RuntimeError :: InvalidArgumentName (name)) if name . as_str () == "<unnamed>"))',
    ),
    "index_and_field": (
        "assert_eq ! (index_value , Value :: Int (2))",
        "assert_eq ! (field_value , Value :: Int (10))",
    ),
    "nested_index_and_field_chains": (
        "assert_eq ! (field_after_index_value , Value :: Int (20))",
        "assert_eq ! (index_after_field_value , Value :: Int (4))",
    ),
    "short_circuit": ("assert_eq ! (value , Value :: Bool (false))",),
    "mixed_numeric_ops": (
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: DInt (3))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) , Err (RuntimeError :: TypeMismatch))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: LReal (3.0))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Real (2.5))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Bool (true))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Bool (true))",
    ),
    "iec_7_3_2": (
        'assert ! (cycle . errors . is_empty () , "unexpected runtime errors: {:?}" , cycle . errors)',
        'harness . assert_eq ("sum" , 5i16)',
        'harness . assert_eq ("neg" , - 5i16)',
        'harness . assert_eq ("arr_val" , 7i16)',
        'harness . assert_eq ("ref_out" , 2i16)',
        'harness . assert_eq ("fb_out" , 5i16)',
        'harness . assert_eq ("size_t" , 2i32)',
    ),
    "super_uses_parent_instance": (
        "assert_eq ! (value , Value :: Instance (parent))",
    ),
    "literal_eval": ("assert_eq ! (value , Value :: Int (42))",),
    "name_ref_eval": ("assert_eq ! (value , Value :: Int (7))",),
    "time_arithmetic_and_compare": (
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Time (Duration :: from_millis (1500)))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Tod (TimeOfDayValue :: new (1500)))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Time (Duration :: from_millis (1500)))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: LTime (Duration :: from_secs (6)))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Time (Duration :: from_millis (500)))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: Bool (true))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , Value :: LTime (Duration :: from_nanos (7)))",
    ),
    "debug_hook_fires_once_per_statement": (
        "assert_eq ! (hook . count , expected)",
    ),
    "en_eno_semantics": (
        'assert_eq ! (storage . get_global ("count") , Some (& Value :: Int (0)))',
        'assert_eq ! (storage . get_global ("eno") , Some (& Value :: Bool (false)))',
    ),
    "param_binding": (
        "assert_eq ! (result , Value :: Int (2))",
        'assert_eq ! (storage . get_global ("out") , Some (& Value :: Int (5)))',
        'assert_eq ! (storage . get_global ("inout") , Some (& Value :: Int (4)))',
    ),
    "default_null_reference": (
        "assert_eq ! (value , Value :: Reference (None))",
    ),
    "ref_and_deref": (
        "assert_eq ! (value , Value :: Int (5))",
        "assert_eq ! (updated , Value :: Int (9))",
    ),
    "assignment": (
        "assert ! (matches ! (result , trust_runtime :: eval :: stmt :: StmtResult :: Continue))",
        'assert_eq ! (storage . get_global ("x") , Some (& Value :: Int (2)))',
    ),
    "case_labels": (
        'assert_eq ! (storage . get_global ("x") , Some (& Value :: Int (9)))',
    ),
    "case_string_labels": (
        'assert_eq ! (storage . get_global ("x") , Some (& Value :: Int (9)))',
    ),
    "if_branches": (
        'assert_eq ! (storage . get_global ("x") , Some (& Value :: Int (1)))',
    ),
    "loop_control": (
        'assert_eq ! (ctx . storage . get_global ("sum") , Some (& Value :: Int (2)))',
        'assert_eq ! (ctx . storage . get_global ("w") , Some (& Value :: Int (2)))',
        'assert_eq ! (ctx . storage . get_global ("r") , Some (& Value :: Int (2)))',
    ),
    "default_result": (
        "assert_eq ! (result , StmtResult :: Return (Some (Value :: Int (4))))",
    ),
    "create_fb_instance_honors_declared_var_input_initializer": (
        'assert_eq ! (storage . get_instance_var (instance_id , "inc") , Some (& Value :: Int (5)))',
    ),
}

ASSERTION_BEHAVIOR_OVERRIDES: Mapping[tuple[str, str], str] = {
    (
        "mixed_numeric_ops",
        "assert_eq ! (eval_expr (& mut ctx , & expr) , Err (RuntimeError :: TypeMismatch))",
    ): "SIGNED_UNSIGNED_CROSS_FAMILY_REJECTION",
}


def combine_inventory_documents(documents: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    inventories: dict[str, Any] = {}
    for document in documents:
        rows = document.get("inventories")
        if not isinstance(rows, Mapping):
            raise ValueError("runtime evaluator assertion document lacks inventories")
        for discovery_id, row in rows.items():
            if discovery_id in inventories:
                raise ValueError(f"duplicate runtime evaluator inventory {discovery_id}")
            inventories[str(discovery_id)] = row
    return {"schema_version": 2, "inventories": inventories}


def _canonical_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("runtime evaluator assertion document fields drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("runtime evaluator assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("runtime evaluator inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append({"discovery_id": discovery_id, "path": path, "name": name})
        batch_rows.append(
            {key: value for key, value in raw.items() if key != "inventory_digest"}
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError("saved runtime evaluator assertion inventory is not canonical")
    return canonical


def select_live_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, document)
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    reviewed_keys = {
        (binding[0], name) for name, binding in TEST_BINDINGS.items()
    }
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and (str(row.get("path")), str(row.get("name"))) in reviewed_keys
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == "no_reviewed_spec_or_invariant_binding"
    }
    if set(expected.values()) != reviewed_keys:
        raise ValueError("live evaluator denominator does not equal reviewed bindings")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live evaluator mapping debt")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(root: Path, document: Mapping[str, Any]) -> dict[str, Any]:
    inventories = _canonical_inventories(root, document)
    if set(REVIEWED_ORACLE_EXPRESSIONS) != set(TEST_BINDINGS):
        raise ValueError("reviewed evaluator oracle table is not exhaustive")
    indexes = _behavior_indexes(root)
    rows = []
    seen = set()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        binding = TEST_BINDINGS.get(name)
        if binding is None or binding[0] != path or name in seen:
            raise ValueError(f"unexpected or duplicate evaluator test {path}:{name}")
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, Sequence) or isinstance(
            assertions, (str, bytes)
        ) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        invariant_id, default_partition = binding[1:]
        expected_oracles = Counter(REVIEWED_ORACLE_EXPRESSIONS[name])
        observed_oracles: Counter[str] = Counter()
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            is_oracle = observed_oracles[expression] < expected_oracles[expression]
            if is_oracle:
                observed_oracles[expression] += 1
            elif _looks_like_product_oracle(expression):
                raise ValueError(
                    f"{path}:{name} has an unreviewed product-oracle assertion"
                )
            partition = ASSERTION_BEHAVIOR_OVERRIDES.get(
                (name, expression),
                default_partition,
            )
            behavior_index = indexes[(invariant_id, partition)]
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_index,
                }
            )
            if not is_oracle:
                setup_ids.append(assertion_id)
        if observed_oracles != expected_oracles:
            raise ValueError(f"{path}:{name} reviewed oracle expressions drifted")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("runtime evaluator assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_remaining_runtime_evaluator_semantic_rules",
        "assignments": rows,
    }


def _looks_like_product_oracle(expression: str) -> bool:
    return (
        expression.startswith("assert")
        or "harness . assert_eq" in expression
        or ". expect_err" in expression
    )


def _behavior_indexes(root: Path) -> dict[BehaviorKey, int]:
    result: dict[BehaviorKey, int] = {}
    invariant_ids = {binding[1] for binding in TEST_BINDINGS.values()}
    for invariant_id in invariant_ids:
        matches = list((root / "verification/invariants").glob(f"**/{invariant_id}.toml"))
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition") if isinstance(behavior, Mapping) else None
            name = partition.get("equals") if isinstance(partition, Mapping) else None
            if isinstance(name, str) and name:
                result[(invariant_id, name)] = index
    return result


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _rooted(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--inventory", type=Path, action="append", required=True)
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    combined = combine_inventory_documents(
        [json.loads(_rooted(root, path).read_text()) for path in args.inventory]
    )
    selected = select_live_inventories(root, combined)
    assignments = generate_assignments(root, selected)
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(f"remaining runtime evaluator assignments generated: {len(assignments['assignments'])}")
    plan_args = (args.execution_dir, args.plans_out, args.reviewed_date, args.timestamp)
    if any(value is not None for value in plan_args):
        if not all(value is not None for value in plan_args):
            raise ValueError("runtime evaluator plan arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="runtime-remaining",
        )
        print(
            "remaining runtime evaluator catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
