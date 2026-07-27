from __future__ import annotations

import hashlib
import tempfile
import tomllib
import unittest
from pathlib import Path

from scripts.verification.runtime_evaluator_assignments import (
    EXPECTED_PATH_COUNTS,
    REVIEWED_ORACLE_EXPRESSIONS,
    SIBLING_OBSERVATIONS,
    TEST_BINDINGS,
    combine_inventory_documents,
    generate_assignments,
    select_live_inventories,
)
from scripts.verification.mapping_assertions import (
    _assertion_id,
    validate_assertion_batch,
)

EXPECTED_BINDING_SNAPSHOT = """
crates/trust-runtime/src/host/eval/tests/errors.rs|datetime_range|RT_CORE_VALUE_PROGRAM_MODEL_001|DATETIME_TICK_RANGE_REJECTION
crates/trust-runtime/src/host/eval/tests/errors.rs|evaluator_unknown_assignment_fails_without_creating_global|RT_VARIABLE_STORAGE_REFERENCE_001|UNKNOWN_LVALUE_WRITE_REJECTION
crates/trust-runtime/src/host/eval/tests/errors.rs|index_and_null_ref|RT_CORE_VALUE_PROGRAM_MODEL_001|ARRAY_AND_REFERENCE_ACCESS_REJECTION
crates/trust-runtime/src/host/eval/tests/errors.rs|neg_overflow_returns_runtime_error|RT_CORE_VALUE_PROGRAM_MODEL_001|SIGNED_NEGATION_OVERFLOW_REJECTION
crates/trust-runtime/src/host/eval/tests/errors.rs|type_errors|RT_CORE_VALUE_PROGRAM_MODEL_001|UNSUPPORTED_UNARY_OPERAND_REJECTION
crates/trust-runtime/src/host/eval/tests/expr_ops.rs|bitwise_ops|RT_CORE_VALUE_PROGRAM_MODEL_001|REPRESENTATIVE_BITWISE_RESULTS
crates/trust-runtime/src/host/eval/tests/expr_ops.rs|power_operator|RT_CORE_VALUE_PROGRAM_MODEL_001|REPRESENTATIVE_EXPONENT_RESULTS
crates/trust-runtime/src/host/eval/tests/expr_ops.rs|precedence|RT_CORE_VALUE_PROGRAM_MODEL_001|PRE_GROUPED_AST_RECURSION
crates/trust-runtime/src/host/eval/tests/expr_ops.rs|string_and_bool_compare|RT_CORE_VALUE_PROGRAM_MODEL_001|REPRESENTATIVE_STRING_BOOL_ORDERING_RESULTS
crates/trust-runtime/src/host/eval/tests/pou_fb.rs|fb_omitted_var_input_reuses_stored_value_after_explicit_update|RT_HOST_POU_EXECUTION_001|FUNCTION_BLOCK_OMITTED_INPUT_REUSE
crates/trust-runtime/src/host/eval/tests/pou_fb.rs|fb_stateful|RT_HOST_POU_EXECUTION_001|FUNCTION_BLOCK_PERSISTENT_STATE
crates/trust-runtime/src/host/eval/tests/pou_fb.rs|pointer_to_wildcard_array_writes_through_correctly|RT_HOST_POU_EXECUTION_001|POINTER_TO_WILDCARD_ARRAY_WRITE_THROUGH
crates/trust-runtime/src/host/eval/tests/pou_fb.rs|var_input_pointer_deref_write_mutates_callers_storage|RT_HOST_POU_EXECUTION_001|VAR_INPUT_POINTER_POINTEE_WRITE_THROUGH
crates/trust-runtime/src/host/eval/tests/pou_fb.rs|wildcard_array_var_in_out_writes_through_correctly|RT_HOST_POU_EXECUTION_001|WILDCARD_ARRAY_IN_OUT_WRITE_THROUGH
crates/trust-runtime/src/host/eval/tests/pou_function.rs|call_function_exec|RT_HOST_POU_EXECUTION_001|FUNCTION_NAMED_INPUT_AND_RESULT
crates/trust-runtime/src/host/eval/tests/pou_function.rs|function_input_interface_defaults_to_null|RT_HOST_POU_EXECUTION_001|FUNCTION_INTERFACE_NULL_DEFAULTS
crates/trust-runtime/src/host/eval/tests/pou_function.rs|function_interface_return_defaults_to_null|RT_HOST_POU_EXECUTION_001|FUNCTION_INTERFACE_NULL_DEFAULTS
crates/trust-runtime/src/host/eval/tests/pou_function.rs|function_local_interface_defaults_to_null|RT_HOST_POU_EXECUTION_001|FUNCTION_INTERFACE_NULL_DEFAULTS
crates/trust-runtime/src/host/helper_eval/const_expr.rs|array_repetition_initializer_uses_expanded_value_shape|RT_HOST_HELPER_EVALUATION_001|ARRAY_REPETITION_EXPANSION
crates/trust-runtime/src/host/helper_eval/const_expr.rs|evaluates_nested_const_expression|RT_HOST_HELPER_EVALUATION_001|NESTED_CONSTANT_EVALUATION
crates/trust-runtime/src/host/helper_eval/const_expr.rs|rejects_non_const_access|RT_HOST_HELPER_EVALUATION_001|RESOLVERLESS_NON_CONSTANT_REJECTION
crates/trust-runtime/src/host/helper_eval/const_expr.rs|resolves_named_const_with_resolver|RT_HOST_HELPER_EVALUATION_001|RESOLVER_BACKED_NAMED_CONSTANT
crates/trust-runtime/src/host/helper_eval/storage_expr.rs|allows_pure_stdlib_calls_when_stdlib_is_provided|RT_HOST_HELPER_EVALUATION_001|PURE_STDLIB_WITH_CAPABILITY
crates/trust-runtime/src/host/helper_eval/storage_expr.rs|array_repetition_initializer_uses_expanded_value_shape|RT_HOST_HELPER_EVALUATION_001|ARRAY_REPETITION_EXPANSION
crates/trust-runtime/src/host/helper_eval/storage_expr.rs|rejects_calls_without_stdlib_surface|RT_HOST_HELPER_EVALUATION_001|ABSENT_STDLIB_CAPABILITY_REJECTION
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|unknown_name_write_fails_without_creating_global|RT_VARIABLE_STORAGE_REFERENCE_001|UNKNOWN_LVALUE_WRITE_REJECTION
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|writes_array_element_without_eval_context|RT_VARIABLE_STORAGE_REFERENCE_001|DIRECT_HOST_LVALUE_TARGETING
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|writes_deref_target_without_eval_context|RT_VARIABLE_STORAGE_REFERENCE_001|DIRECT_HOST_LVALUE_TARGETING
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|writes_local_name_without_eval_context|RT_VARIABLE_STORAGE_REFERENCE_001|DIRECT_HOST_LVALUE_TARGETING
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|writes_nested_array_of_struct_field_without_eval_context|RT_VARIABLE_STORAGE_REFERENCE_001|DIRECT_HOST_LVALUE_TARGETING
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|writes_nested_struct_array_element_without_eval_context|RT_VARIABLE_STORAGE_REFERENCE_001|DIRECT_HOST_LVALUE_TARGETING
crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs|writes_struct_field_without_eval_context|RT_VARIABLE_STORAGE_REFERENCE_001|DIRECT_HOST_LVALUE_TARGETING
""".strip()

EXPECTED_SIBLING_EXPRESSIONS = {
    (
        "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs",
        "writes_array_element_without_eval_context",
    ): (
        "assert_eq ! (array . elements () , & [Value :: DInt (1) , "
        "Value :: DInt (9) , Value :: DInt (3)])"
    ),
    (
        "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs",
        "writes_struct_field_without_eval_context",
    ): 'assert_eq ! (st . field ("y") , Some (& Value :: DInt (2)))',
    (
        "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs",
        "writes_nested_struct_array_element_without_eval_context",
    ): (
        "assert_eq ! (array . elements () , & [Value :: DInt (1) , "
        "Value :: DInt (9) , Value :: DInt (3)])"
    ),
    (
        "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs",
        "writes_nested_array_of_struct_field_without_eval_context",
    ): (
        'assert_eq ! (sibling . field ("value") , '
        "Some (& Value :: DInt (1)))"
    ),
}


class RuntimeEvaluatorAssignmentsTests(unittest.TestCase):
    def _inventory(self) -> dict[str, object]:
        inventories = {}
        for index, (path, name) in enumerate(TEST_BINDINGS):
            oracle_expressions = REVIEWED_ORACLE_EXPRESSIONS[(path, name)]
            inventories[f"DISC_{index:02d}"] = {
                "schema_version": 2,
                "path": path,
                "test_name": name,
                "unresolved_assertion_helpers": [],
                "assertions": [
                    {
                        "assertion_id": f"SETUP_{index:02d}",
                        "expression": "construct () . unwrap ()",
                    },
                    *[
                        {
                            "assertion_id": f"ORACLE_{index:02d}_{oracle_index}",
                            "expression": expression,
                        }
                        for oracle_index, expression in enumerate(
                            oracle_expressions
                        )
                    ],
                ],
            }
        return {
            "schema_version": 2,
            "inventories": inventories,
        }

    def _root(self, inventory: dict[str, object]) -> tempfile.TemporaryDirectory:
        temp = tempfile.TemporaryDirectory()
        root = Path(temp.name)
        for source_path in EXPECTED_PATH_COUNTS:
            source = root / source_path
            source.parent.mkdir(parents=True, exist_ok=True)
            content = f"// reviewed fixture for {source_path}\n".encode()
            source.write_bytes(content)
            digest = "sha256:" + hashlib.sha256(content).hexdigest()
            for row in inventory["inventories"].values():
                if row["path"] == source_path:
                    row["source_digest"] = digest
                    row["source_digests"] = {source_path: digest}
                    for line, assertion in enumerate(
                        row["assertions"],
                        start=1,
                    ):
                        expression = assertion["expression"]
                        expression_digest = (
                            "sha256:"
                            + hashlib.sha256(expression.encode()).hexdigest()
                        )
                        kind = (
                            "macro:assert"
                            if expression.startswith("assert")
                            else "call:expect_err"
                            if ". expect_err" in expression
                            else "call:unwrap"
                        )
                        assertion.update(
                            {
                                "kind": kind,
                                "source_path": source_path,
                                "function": row["test_name"],
                                "line": line,
                                "expression_digest": expression_digest,
                                "assertion_id": _assertion_id(
                                    path=source_path,
                                    name=row["test_name"],
                                    source_path=source_path,
                                    function=row["test_name"],
                                    kind=kind,
                                    line=line,
                                    expression_digest=expression_digest,
                                ),
                            }
                        )
        requested = [
            {
                "discovery_id": discovery_id,
                "path": row["path"],
                "name": row["test_name"],
            }
            for discovery_id, row in inventory["inventories"].items()
        ]
        inventory["inventories"] = validate_assertion_batch(
            root,
            {
                "schema_version": 2,
                "inventories": list(inventory["inventories"].values()),
            },
            requested,
        )
        partitions: dict[str, set[str]] = {}
        for invariant_id, partition in TEST_BINDINGS.values():
            partitions.setdefault(invariant_id, set()).add(partition)
        partitions.setdefault(
            "RT_VARIABLE_STORAGE_REFERENCE_001",
            set(),
        ).add("HOST_LVALUE_SIBLING_PRESERVATION")
        for invariant_id, names in partitions.items():
            path = (
                root
                / "verification"
                / "invariants"
                / "runtime_safety"
                / f"{invariant_id}.toml"
            )
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(
                f'id = "{invariant_id}"\n'
                + "\n".join(
                    "[[behavior]]\n"
                    f'partition = {{ equals = "{name}" }}'
                    for name in sorted(names)
                )
                + "\n"
            )
        reviews = []
        for discovery_id, row in inventory["inventories"].items():
            reviews.append(
                "[[reviews]]\n"
                f'discovery_id = "{discovery_id}"\n'
                f'path = "{row["path"]}"\n'
                f'name = "{row["test_name"]}"\n'
                'disposition = "reviewed_nonmapping"\n'
                'reason_code = "no_reviewed_spec_or_invariant_binding"\n'
            )
        denominator = root / "verification/test-catalog-denominator.toml"
        denominator.write_text("schema_version = 1\n" + "\n".join(reviews))
        return temp

    def test_live_selection_is_exact_across_seven_paths(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        selected = select_live_inventories(
            root=Path(temp.name),
            inventory_document=inventory,
        )
        self.assertEqual(32, len(selected["inventories"]))
        self.assertEqual(
            dict(EXPECTED_PATH_COUNTS),
            {
                path: sum(
                    row["path"] == path
                    for row in selected["inventories"].values()
                )
                for path in EXPECTED_PATH_COUNTS
            },
        )

    def test_assignments_resolve_partitions_and_setup_assertions(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        assignments = generate_assignments(
            root=Path(temp.name),
            inventory_document=inventory,
        )
        self.assertEqual(32, len(assignments["assignments"]))
        self.assertEqual(
            set(TEST_BINDINGS),
            {(row["path"], row["name"]) for row in assignments["assignments"]},
        )
        self.assertTrue(
            all(
                len(row["setup_assertion_ids"]) == 1
                and len(row["assertion_assignments"])
                == 1 + len(
                    REVIEWED_ORACLE_EXPRESSIONS[(row["path"], row["name"])]
                )
                for row in assignments["assignments"]
            )
        )

    def test_reviewed_binding_table_matches_independent_literal_snapshot(self) -> None:
        actual = "\n".join(
            f"{path}|{name}|{invariant_id}|{partition}"
            for (path, name), (invariant_id, partition) in sorted(
                TEST_BINDINGS.items()
            )
        )
        self.assertEqual(EXPECTED_BINDING_SNAPSHOT, actual)

    def test_four_reviewed_sibling_assertions_map_to_exact_partition(self) -> None:
        self.assertEqual(EXPECTED_SIBLING_EXPRESSIONS, SIBLING_OBSERVATIONS)
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        assignments = generate_assignments(
            root=root,
            inventory_document=inventory,
        )
        invariant = next(
            (root / "verification/invariants").glob(
                "**/RT_VARIABLE_STORAGE_REFERENCE_001.toml"
            )
        )
        behavior_names = [
            row["partition"]["equals"]
            for row in tomllib.loads(invariant.read_text())["behavior"]
        ]
        by_test = {
            (row["path"], row["name"]): row
            for row in assignments["assignments"]
        }
        for key in EXPECTED_SIBLING_EXPRESSIONS:
            sibling_assignments = [
                assignment
                for assignment in by_test[key]["assertion_assignments"]
                if behavior_names[assignment["behavior_index"]]
                == "HOST_LVALUE_SIBLING_PRESERVATION"
            ]
            self.assertEqual(
                1,
                len(sibling_assignments),
            )

    def test_reviewed_sibling_assertion_expression_drift_is_rejected(self) -> None:
        inventory = self._inventory()
        key = next(iter(EXPECTED_SIBLING_EXPRESSIONS))
        row = next(
            item
            for item in inventory["inventories"].values()
            if (item["path"], item["test_name"]) == key
        )
        row["assertions"][1]["expression"] += " "
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        with self.assertRaisesRegex(ValueError, "unreviewed product-oracle"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_unresolved_helpers_are_rejected(self) -> None:
        inventory = self._inventory()
        first = next(iter(inventory["inventories"].values()))
        first["unresolved_assertion_helpers"] = ["helper"]
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        with self.assertRaisesRegex(ValueError, "unresolved assertion helpers"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_stale_source_digest_is_rejected(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        first = next(iter(inventory["inventories"].values()))
        (Path(temp.name) / first["path"]).write_text("// drifted source\n")
        with self.assertRaisesRegex(ValueError, "source digest is stale"):
            select_live_inventories(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_tampered_expression_digest_is_rejected(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        first = next(iter(inventory["inventories"].values()))
        first["assertions"][0]["expression"] = "tampered ()"
        with self.assertRaisesRegex(ValueError, "expression digest is stale"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_fresh_unreviewed_product_oracle_is_rejected(self) -> None:
        inventory = self._inventory()
        first = next(iter(inventory["inventories"].values()))
        first["assertions"].append(
            {
                "assertion_id": "NEW_ORACLE",
                "expression": "assert ! (new_behavior)",
            }
        )
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        with self.assertRaisesRegex(ValueError, "unreviewed product-oracle"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_missing_behavior_partition_is_rejected(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        invariant_id, partition = next(iter(TEST_BINDINGS.values()))
        invariant = next(
            (Path(temp.name) / "verification/invariants").glob(
                f"**/{invariant_id}.toml"
            )
        )
        invariant.write_text(
            invariant.read_text().replace(
                f'partition = {{ equals = "{partition}" }}',
                'partition = { equals = "REMOVED_BY_TEST" }',
                1,
            )
        )
        with self.assertRaisesRegex(ValueError, "selects missing behavior"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_inventory_with_only_setup_assertions_is_rejected(self) -> None:
        inventory = self._inventory()
        first = next(iter(inventory["inventories"].values()))
        first["assertions"] = [
            {
                "assertion_id": "SETUP_ONLY",
                "expression": "construct () . unwrap ()",
            }
        ]
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        with self.assertRaisesRegex(ValueError, "product-oracle assertions drifted"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )

    def test_duplicate_inventory_documents_are_rejected(self) -> None:
        inventory = self._inventory()
        with self.assertRaisesRegex(ValueError, "duplicate runtime evaluator"):
            combine_inventory_documents([inventory, inventory])


if __name__ == "__main__":
    unittest.main()
