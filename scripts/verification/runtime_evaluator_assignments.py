"""Generate reviewed runtime evaluator assertion assignments in one batch."""

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


STORAGE_LVALUE = "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs"
POU_FB = "crates/trust-runtime/src/host/eval/tests/pou_fb.rs"
ERRORS = "crates/trust-runtime/src/host/eval/tests/errors.rs"
POU_FUNCTION = "crates/trust-runtime/src/host/eval/tests/pou_function.rs"
EXPR_OPS = "crates/trust-runtime/src/host/eval/tests/expr_ops.rs"
CONST_EXPR = "crates/trust-runtime/src/host/helper_eval/const_expr.rs"
STORAGE_EXPR = "crates/trust-runtime/src/host/helper_eval/storage_expr.rs"

EXPECTED_PATH_COUNTS: Mapping[str, int] = {
    STORAGE_LVALUE: 7,
    POU_FB: 5,
    ERRORS: 5,
    POU_FUNCTION: 4,
    EXPR_OPS: 4,
    CONST_EXPR: 4,
    STORAGE_EXPR: 3,
}

BehaviorKey = tuple[str, str]
TestKey = tuple[str, str]

TEST_BINDINGS: Mapping[TestKey, BehaviorKey] = {
    (STORAGE_LVALUE, "writes_local_name_without_eval_context"): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "DIRECT_HOST_LVALUE_TARGETING",
    ),
    (STORAGE_LVALUE, "unknown_name_write_fails_without_creating_global"): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "UNKNOWN_LVALUE_WRITE_REJECTION",
    ),
    (STORAGE_LVALUE, "writes_array_element_without_eval_context"): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "DIRECT_HOST_LVALUE_TARGETING",
    ),
    (STORAGE_LVALUE, "writes_deref_target_without_eval_context"): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "DIRECT_HOST_LVALUE_TARGETING",
    ),
    (STORAGE_LVALUE, "writes_struct_field_without_eval_context"): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "DIRECT_HOST_LVALUE_TARGETING",
    ),
    (
        STORAGE_LVALUE,
        "writes_nested_struct_array_element_without_eval_context",
    ): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "DIRECT_HOST_LVALUE_TARGETING",
    ),
    (
        STORAGE_LVALUE,
        "writes_nested_array_of_struct_field_without_eval_context",
    ): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "DIRECT_HOST_LVALUE_TARGETING",
    ),
    (POU_FB, "fb_stateful"): (
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_BLOCK_PERSISTENT_STATE",
    ),
    (POU_FB, "fb_omitted_var_input_reuses_stored_value_after_explicit_update"): (
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_BLOCK_OMITTED_INPUT_REUSE",
    ),
    (POU_FB, "var_input_pointer_deref_write_mutates_callers_storage"): (
        "RT_HOST_POU_EXECUTION_001",
        "VAR_INPUT_POINTER_POINTEE_WRITE_THROUGH",
    ),
    (POU_FB, "wildcard_array_var_in_out_writes_through_correctly"): (
        "RT_HOST_POU_EXECUTION_001",
        "WILDCARD_ARRAY_IN_OUT_WRITE_THROUGH",
    ),
    (POU_FB, "pointer_to_wildcard_array_writes_through_correctly"): (
        "RT_HOST_POU_EXECUTION_001",
        "POINTER_TO_WILDCARD_ARRAY_WRITE_THROUGH",
    ),
    (ERRORS, "type_errors"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "UNSUPPORTED_UNARY_OPERAND_REJECTION",
    ),
    (ERRORS, "neg_overflow_returns_runtime_error"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "SIGNED_NEGATION_OVERFLOW_REJECTION",
    ),
    (ERRORS, "index_and_null_ref"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "ARRAY_AND_REFERENCE_ACCESS_REJECTION",
    ),
    (ERRORS, "evaluator_unknown_assignment_fails_without_creating_global"): (
        "RT_VARIABLE_STORAGE_REFERENCE_001",
        "UNKNOWN_LVALUE_WRITE_REJECTION",
    ),
    (ERRORS, "datetime_range"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "DATETIME_TICK_RANGE_REJECTION",
    ),
    (POU_FUNCTION, "call_function_exec"): (
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_NAMED_INPUT_AND_RESULT",
    ),
    (POU_FUNCTION, "function_input_interface_defaults_to_null"): (
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_INTERFACE_NULL_DEFAULTS",
    ),
    (POU_FUNCTION, "function_interface_return_defaults_to_null"): (
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_INTERFACE_NULL_DEFAULTS",
    ),
    (POU_FUNCTION, "function_local_interface_defaults_to_null"): (
        "RT_HOST_POU_EXECUTION_001",
        "FUNCTION_INTERFACE_NULL_DEFAULTS",
    ),
    (EXPR_OPS, "precedence"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "PRE_GROUPED_AST_RECURSION",
    ),
    (EXPR_OPS, "power_operator"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "REPRESENTATIVE_EXPONENT_RESULTS",
    ),
    (EXPR_OPS, "bitwise_ops"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "REPRESENTATIVE_BITWISE_RESULTS",
    ),
    (EXPR_OPS, "string_and_bool_compare"): (
        "RT_CORE_VALUE_PROGRAM_MODEL_001",
        "REPRESENTATIVE_STRING_BOOL_ORDERING_RESULTS",
    ),
    (CONST_EXPR, "evaluates_nested_const_expression"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "NESTED_CONSTANT_EVALUATION",
    ),
    (CONST_EXPR, "rejects_non_const_access"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "RESOLVERLESS_NON_CONSTANT_REJECTION",
    ),
    (CONST_EXPR, "resolves_named_const_with_resolver"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "RESOLVER_BACKED_NAMED_CONSTANT",
    ),
    (CONST_EXPR, "array_repetition_initializer_uses_expanded_value_shape"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "ARRAY_REPETITION_EXPANSION",
    ),
    (STORAGE_EXPR, "allows_pure_stdlib_calls_when_stdlib_is_provided"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "PURE_STDLIB_WITH_CAPABILITY",
    ),
    (STORAGE_EXPR, "rejects_calls_without_stdlib_surface"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "ABSENT_STDLIB_CAPABILITY_REJECTION",
    ),
    (STORAGE_EXPR, "array_repetition_initializer_uses_expanded_value_shape"): (
        "RT_HOST_HELPER_EVALUATION_001",
        "ARRAY_REPETITION_EXPANSION",
    ),
}

SIBLING_OBSERVATIONS: Mapping[TestKey, str] = {
    (
        STORAGE_LVALUE,
        "writes_array_element_without_eval_context",
    ): (
        "assert_eq ! (array . elements () , & [Value :: DInt (1) , "
        "Value :: DInt (9) , Value :: DInt (3)])"
    ),
    (
        STORAGE_LVALUE,
        "writes_struct_field_without_eval_context",
    ): 'assert_eq ! (st . field ("y") , Some (& Value :: DInt (2)))',
    (
        STORAGE_LVALUE,
        "writes_nested_struct_array_element_without_eval_context",
    ): (
        "assert_eq ! (array . elements () , & [Value :: DInt (1) , "
        "Value :: DInt (9) , Value :: DInt (3)])"
    ),
    (
        STORAGE_LVALUE,
        "writes_nested_array_of_struct_field_without_eval_context",
    ): (
        'assert_eq ! (sibling . field ("value") , '
        "Some (& Value :: DInt (1)))"
    ),
}

REVIEWED_ORACLE_EXPRESSIONS: Mapping[TestKey, tuple[str, ...]] = {
    (ERRORS, "datetime_range"): (
        "assert_eq ! (err , RuntimeError :: DateTimeRange "
        "(DateTimeError :: OutOfRange))",
    ),
    (ERRORS, "evaluator_unknown_assignment_fails_without_creating_global"): (
        "write_lvalue (& mut ctx , & LValue :: Name "
        '("missing" . into ()) , Value :: DInt (7)) . expect_err '
        '("unknown evaluator assignment must fail")',
        "assert_eq ! (err , RuntimeError :: UndefinedVariable "
        '("missing" . into ()))',
        'assert ! (ctx . storage . get_global ("missing") . is_none ())',
    ),
    (ERRORS, "index_and_null_ref"): (
        "assert ! (matches ! (err , RuntimeError :: IndexOutOfBounds { .. }))",
        "assert_eq ! (err , RuntimeError :: NullReference)",
        "assert_eq ! (err , RuntimeError :: UndefinedVariable "
        '("missing" . into ()))',
    ),
    (ERRORS, "neg_overflow_returns_runtime_error"): (
        "assert_eq ! (apply_unary (UnaryOp :: Neg , Value :: SInt "
        "(i8 :: MIN)) , Err (RuntimeError :: Overflow))",
        "assert_eq ! (apply_unary (UnaryOp :: Neg , Value :: Int "
        "(i16 :: MIN)) , Err (RuntimeError :: Overflow))",
        "assert_eq ! (apply_unary (UnaryOp :: Neg , Value :: DInt "
        "(i32 :: MIN)) , Err (RuntimeError :: Overflow))",
        "assert_eq ! (apply_unary (UnaryOp :: Neg , Value :: LInt "
        "(i64 :: MIN)) , Err (RuntimeError :: Overflow))",
    ),
    (ERRORS, "type_errors"): (
        "assert_eq ! (apply_unary (UnaryOp :: Neg , Value :: Bool (true)) , "
        "Err (RuntimeError :: TypeMismatch))",
    ),
    (EXPR_OPS, "bitwise_ops"): (
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: Byte (0x00))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: Word (0x0FFF))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: Byte (0x0F))",
    ),
    (EXPR_OPS, "power_operator"): (
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: Int (8))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: LReal (8.0))",
    ),
    (EXPR_OPS, "precedence"): (
        "assert_eq ! (value , Value :: Int (7))",
    ),
    (EXPR_OPS, "string_and_bool_compare"): (
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: Bool (true))",
        "assert_eq ! (eval_expr (& mut ctx , & expr) . unwrap () , "
        "Value :: Bool (true))",
    ),
    (POU_FB, "fb_omitted_var_input_reuses_stored_value_after_explicit_update"): (
        "assert_eq ! (ctx . storage . get_instance_var "
        '(instance_id , "result") , Some (& Value :: Int (8)))',
        "assert_eq ! (ctx . storage . get_instance_var "
        '(instance_id , "result") , Some (& Value :: Int (12)))',
        "assert_eq ! (ctx . storage . get_instance_var "
        '(instance_id , "result") , Some (& Value :: Int (12)))',
    ),
    (POU_FB, "fb_stateful"): (
        "assert_eq ! (storage . get_instance_var "
        '(instance_id , "count") , Some (& Value :: Int (2)))',
    ),
    (POU_FB, "pointer_to_wildcard_array_writes_through_correctly"): (
        "assert_eq ! (local . elements () , & [Value :: Byte (0) , "
        "Value :: Byte (0) , Value :: Byte (11) , Value :: Byte (0)])",
    ),
    (POU_FB, "var_input_pointer_deref_write_mutates_callers_storage"): (
        "assert_eq ! (local . elements () , & [Value :: Int (0) , "
        "Value :: Int (123) , Value :: Int (0) , Value :: Int (0)])",
    ),
    (POU_FB, "wildcard_array_var_in_out_writes_through_correctly"): (
        "assert_eq ! (small . elements () , & [Value :: Byte (0) , "
        "Value :: Byte (9) , Value :: Byte (0) , Value :: Byte (0)])",
        "assert_eq ! (large . elements () , & [Value :: Byte (0) , "
        "Value :: Byte (9) , Value :: Byte (0) , Value :: Byte (0) , "
        "Value :: Byte (0) , Value :: Byte (0) , Value :: Byte (0) , "
        "Value :: Byte (0)])",
    ),
    (POU_FUNCTION, "call_function_exec"): (
        "assert_eq ! (result , Value :: Int (6))",
    ),
    (POU_FUNCTION, "function_input_interface_defaults_to_null"): (
        "assert_eq ! (result , Value :: Null)",
    ),
    (POU_FUNCTION, "function_interface_return_defaults_to_null"): (
        "assert_eq ! (result , Value :: Null)",
    ),
    (POU_FUNCTION, "function_local_interface_defaults_to_null"): (
        "assert_eq ! (result , Value :: Null)",
    ),
    (CONST_EXPR, "array_repetition_initializer_uses_expanded_value_shape"): (
        "assert_eq ! (array . dimensions () , & [(1 , 6)])",
        "assert_eq ! (array . elements () , & [Value :: Int (1) , "
        "Value :: Int (2) , Value :: Int (1) , Value :: Int (2) , "
        "Value :: Int (1) , Value :: Int (2) ,])",
    ),
    (CONST_EXPR, "evaluates_nested_const_expression"): (
        "assert_eq ! (value , Value :: Int (- 20))",
    ),
    (CONST_EXPR, "rejects_non_const_access"): (
        "assert ! (matches ! (eval_const_expr "
        "(& expr , & DateTimeProfile :: default ()) , "
        "Err (ConstExprError :: UnsupportedExpr)))",
    ),
    (CONST_EXPR, "resolves_named_const_with_resolver"): (
        "assert_eq ! (value , Value :: Int (12))",
    ),
    (STORAGE_EXPR, "allows_pure_stdlib_calls_when_stdlib_is_provided"): (
        "assert_eq ! (value , Value :: DInt (1))",
    ),
    (STORAGE_EXPR, "array_repetition_initializer_uses_expanded_value_shape"): (
        "assert_eq ! (array . dimensions () , & [(1 , 6)])",
        "assert_eq ! (array . elements () , & [Value :: Int (1) , "
        "Value :: Int (2) , Value :: Int (1) , Value :: Int (2) , "
        "Value :: Int (1) , Value :: Int (2) ,])",
    ),
    (STORAGE_EXPR, "rejects_calls_without_stdlib_surface"): (
        "assert ! (matches ! (eval_storage_expr_with_stdlib "
        "(& storage , & registry , & profile , None , None , & expr) , "
        "Err (RuntimeError :: TypeMismatch)))",
    ),
    (STORAGE_LVALUE, "unknown_name_write_fails_without_creating_global"): (
        "write_storage_lvalue (& mut storage , & registry , "
        "& DateTimeProfile :: default () , None , & LValue :: Name "
        '("missing" . into ()) , Value :: DInt (7) ,) . expect_err '
        '("unknown lvalue write must fail")',
        "assert_eq ! (err , RuntimeError :: UndefinedVariable "
        '("missing" . into ()))',
        'assert ! (storage . get_global ("missing") . is_none ())',
    ),
    (STORAGE_LVALUE, "writes_array_element_without_eval_context"): (
        "assert_eq ! (array . elements () , & [Value :: DInt (1) , "
        "Value :: DInt (9) , Value :: DInt (3)])",
    ),
    (STORAGE_LVALUE, "writes_deref_target_without_eval_context"): (
        'assert_eq ! (storage . get_global ("x") , '
        "Some (& Value :: DInt (11)))",
        "assert_eq ! (stored , Value :: DInt (11))",
    ),
    (STORAGE_LVALUE, "writes_local_name_without_eval_context"): (
        'assert_eq ! (storage . get_local ("x") , '
        "Some (& Value :: DInt (7)))",
    ),
    (
        STORAGE_LVALUE,
        "writes_nested_array_of_struct_field_without_eval_context",
    ): (
        'assert_eq ! (item . field ("value") , '
        "Some (& Value :: DInt (7)))",
        'assert_eq ! (sibling . field ("value") , '
        "Some (& Value :: DInt (1)))",
    ),
    (
        STORAGE_LVALUE,
        "writes_nested_struct_array_element_without_eval_context",
    ): (
        "assert_eq ! (array . elements () , & [Value :: DInt (1) , "
        "Value :: DInt (9) , Value :: DInt (3)])",
    ),
    (STORAGE_LVALUE, "writes_struct_field_without_eval_context"): (
        'assert_eq ! (st . field ("x") , Some (& Value :: DInt (3)))',
        'assert_eq ! (st . field ("y") , Some (& Value :: DInt (2)))',
    ),
}


def combine_inventory_documents(
    documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
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


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = _canonical_saved_inventories(
        root=root,
        inventory_document=inventory_document,
    )
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") in EXPECTED_PATH_COUNTS
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == "no_reviewed_spec_or_invariant_binding"
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError(
            "live runtime evaluator denominator does not equal reviewed rules"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventory lacks live runtime evaluator mapping debt"
        )
    for discovery_id, raw in selected.items():
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = _canonical_saved_inventories(
        root=root,
        inventory_document=inventory_document,
    )
    if set(REVIEWED_ORACLE_EXPRESSIONS) != set(TEST_BINDINGS):
        raise ValueError("reviewed runtime evaluator oracle table is not exhaustive")
    behavior_indexes = _invariant_behavior_indexes(root)
    rows = []
    seen: set[TestKey] = set()
    sibling_observations_seen: set[TestKey] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        key = (path, name)
        if key in seen or key not in TEST_BINDINGS:
            raise ValueError(f"unexpected or duplicate runtime evaluator test {path}:{name}")
        seen.add(key)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        assertion_rows = []
        setup_ids = []
        expected_oracles = Counter(REVIEWED_ORACLE_EXPRESSIONS[key])
        observed_oracles: Counter[str] = Counter()
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
                    f"{path}:{name} has an unreviewed product-oracle assertion "
                    f"{assertion_id}"
                )
            invariant_id, partition = _behavior_for_assertion(
                path=path,
                name=name,
                expression=expression,
            )
            if partition == "HOST_LVALUE_SIBLING_PRESERVATION":
                sibling_observations_seen.add(key)
            try:
                behavior_index = behavior_indexes[(invariant_id, partition)]
            except KeyError as error:
                raise ValueError(
                    f"{path}:{name} selects missing behavior "
                    f"{invariant_id}:{partition}"
                ) from error
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
            raise ValueError(
                f"{path}:{name} reviewed product-oracle assertions drifted"
            )
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
    if sibling_observations_seen != set(SIBLING_OBSERVATIONS):
        missing = sorted(set(SIBLING_OBSERVATIONS) - sibling_observations_seen)
        unexpected = sorted(sibling_observations_seen - set(SIBLING_OBSERVATIONS))
        raise ValueError(
            "reviewed sibling observations drifted: "
            f"missing={missing}, unexpected={unexpected}"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_evaluator_semantic_rules",
        "assignments": rows,
    }


def _looks_like_product_oracle(expression: str) -> bool:
    return expression.startswith("assert") or ". expect_err" in expression


def _behavior_for_assertion(
    *,
    path: str,
    name: str,
    expression: str,
) -> BehaviorKey:
    if SIBLING_OBSERVATIONS.get((path, name)) == expression:
        return (
            "RT_VARIABLE_STORAGE_REFERENCE_001",
            "HOST_LVALUE_SIBLING_PRESERVATION",
        )
    return TEST_BINDINGS[(path, name)]


def _canonical_saved_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(inventory_document) != {"schema_version", "inventories"}:
        raise ValueError("runtime evaluator assertion document fields drifted")
    if inventory_document.get("schema_version") != 2:
        raise ValueError("runtime evaluator assertion document schema drifted")
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("runtime evaluator assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("runtime evaluator inventory ID is invalid")
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append(
            {
                "discovery_id": discovery_id,
                "path": path,
                "name": name,
            }
        )
        batch_rows.append(
            {
                key: value
                for key, value in raw.items()
                if key != "inventory_digest"
            }
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError(
            "saved runtime evaluator assertion inventory is not canonical"
        )
    return canonical


def _invariant_behavior_indexes(root: Path) -> dict[BehaviorKey, int]:
    result: dict[BehaviorKey, int] = {}
    invariant_ids = {invariant_id for invariant_id, _ in TEST_BINDINGS.values()}
    for invariant_id in invariant_ids:
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant_id}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = (
                behavior.get("partition")
                if isinstance(behavior, Mapping)
                else None
            )
            name = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if isinstance(name, str) and name:
                key = (invariant_id, name)
                if key in result:
                    raise ValueError(f"duplicate behavior partition {key}")
                result[key] = index
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
    selected = select_live_inventories(
        root=root,
        inventory_document=combined,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(
        "runtime evaluator assignments generated: "
        f"{len(assignments['assignments'])}"
    )
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
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
            "runtime evaluator catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
