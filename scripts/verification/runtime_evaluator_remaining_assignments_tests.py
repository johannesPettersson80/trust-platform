from __future__ import annotations

import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification import runtime_evaluator_remaining_assignments as subject


class RuntimeEvaluatorRemainingAssignmentsTests(unittest.TestCase):
    def test_reviewed_binding_snapshot_is_independent_and_exhaustive(self) -> None:
        call_path = "crates/trust-runtime/src/host/eval/expr/call/tests.rs"
        eval_tests = "crates/trust-runtime/src/host/eval/tests"
        instance_path = "crates/trust-runtime/src/host/instance.rs"
        expected = {
            (call_path, "bind_split_args_rejects_unnamed_named_call_without_panic", "RT_HOST_POU_EXECUTION_001", "SPLIT_UNNAMED_AFTER_NAMED_REJECTION"),
            (call_path, "bind_stdlib_named_args_rejects_unnamed_arg_without_panic", "RT_HOST_POU_EXECUTION_001", "STDLIB_UNNAMED_NAMED_ARGUMENT_REJECTION"),
            (f"{eval_tests}/expr_access.rs", "index_and_field", "RT_HOST_EXPRESSION_EVALUATION_001", "INDEX_AND_FIELD_ACCESS"),
            (f"{eval_tests}/expr_access.rs", "nested_index_and_field_chains", "RT_HOST_EXPRESSION_EVALUATION_001", "NESTED_INDEX_FIELD_CHAINS"),
            (f"{eval_tests}/expr_bool.rs", "short_circuit", "RT_HOST_EXPRESSION_EVALUATION_001", "BOOLEAN_SHORT_CIRCUIT"),
            (f"{eval_tests}/expr_coercion.rs", "mixed_numeric_ops", "RT_HOST_EXPRESSION_EVALUATION_001", "MIXED_NUMERIC_OPERATIONS"),
            (f"{eval_tests}/expr_full.rs", "iec_7_3_2", "RT_HOST_EXPRESSION_EVALUATION_001", "REPRESENTATIVE_EXPRESSION_PIPELINE"),
            (f"{eval_tests}/expr_full.rs", "super_uses_parent_instance", "RT_HOST_EXPRESSION_EVALUATION_001", "SUPER_PARENT_INSTANCE"),
            (f"{eval_tests}/expr_literals.rs", "literal_eval", "RT_HOST_EXPRESSION_EVALUATION_001", "LITERAL_EVALUATION"),
            (f"{eval_tests}/expr_literals.rs", "name_ref_eval", "RT_HOST_EXPRESSION_EVALUATION_001", "NAME_REFERENCE_EVALUATION"),
            (f"{eval_tests}/expr_time_ops.rs", "time_arithmetic_and_compare", "RT_HOST_EXPRESSION_EVALUATION_001", "TIME_ARITHMETIC_AND_COMPARISON"),
            (f"{eval_tests}/mod.rs", "debug_hook_fires_once_per_statement", "DEBUG_CONTROL_EXECUTION_001", "STATEMENT_HOOK_ONCE_PER_STATEMENT"),
            (f"{eval_tests}/pou_en_eno.rs", "en_eno_semantics", "RT_HOST_POU_EXECUTION_001", "EN_FALSE_SKIPS_BODY"),
            (f"{eval_tests}/pou_params.rs", "param_binding", "RT_HOST_POU_EXECUTION_001", "FUNCTION_OUTPUT_INOUT_BINDING"),
            (f"{eval_tests}/reference.rs", "default_null_reference", "RT_CORE_VALUE_PROGRAM_MODEL_001", "REFERENCE_NULL_DEFAULT"),
            (f"{eval_tests}/reference.rs", "ref_and_deref", "RT_HOST_EXPRESSION_EVALUATION_001", "REFERENCE_DEREFERENCE_WRITE"),
            (f"{eval_tests}/stmt_basic.rs", "assignment", "RT_HOST_STATEMENT_EXECUTION_001", "ASSIGNMENT_CONTINUE"),
            (f"{eval_tests}/stmt_case.rs", "case_labels", "RT_HOST_STATEMENT_EXECUTION_001", "CASE_NUMERIC_SELECTION"),
            (f"{eval_tests}/stmt_case.rs", "case_string_labels", "RT_HOST_STATEMENT_EXECUTION_001", "CASE_STRING_SELECTION"),
            (f"{eval_tests}/stmt_if.rs", "if_branches", "RT_HOST_STATEMENT_EXECUTION_001", "IF_BRANCH_SELECTION"),
            (f"{eval_tests}/stmt_loops.rs", "loop_control", "RT_HOST_STATEMENT_EXECUTION_001", "FOR_WHILE_REPEAT_CONTROL"),
            (f"{eval_tests}/stmt_return.rs", "default_result", "RT_HOST_STATEMENT_EXECUTION_001", "RETURN_VALUE"),
            (instance_path, "create_fb_instance_honors_declared_var_input_initializer", "RT_HOST_POU_EXECUTION_001", "FB_DECLARED_INPUT_INITIALIZER"),
        }
        actual = {
            (binding[0], name, binding[1], binding[2])
            for name, binding in subject.TEST_BINDINGS.items()
        }
        self.assertEqual(actual, expected)

    def test_oracle_snapshot_covers_every_test_and_49_expressions(self) -> None:
        self.assertEqual(
            set(subject.REVIEWED_ORACLE_EXPRESSIONS),
            set(subject.TEST_BINDINGS),
        )
        self.assertEqual(
            sum(map(len, subject.REVIEWED_ORACLE_EXPRESSIONS.values())),
            49,
        )
        self.assertEqual(
            subject.ASSERTION_BEHAVIOR_OVERRIDES,
            {
                (
                    "mixed_numeric_ops",
                    "assert_eq ! (eval_expr (& mut ctx , & expr) , Err (RuntimeError :: TypeMismatch))",
                ): "SIGNED_UNSIGNED_CROSS_FAMILY_REJECTION"
            },
        )

    def test_fresh_unreviewed_oracle_replacement_is_rejected(self) -> None:
        path = "crate/tests.rs"
        name = "case"
        inventory = {
            "DISC": {
                "path": path,
                "test_name": name,
                "unresolved_assertion_helpers": [],
                "assertions": [
                    {
                        "assertion_id": "ASSERT",
                        "expression": "assert ! (changed)",
                    }
                ],
            }
        }
        bindings = {name: (path, "INV", "PART")}
        reviewed = {name: ("assert ! (original)",)}
        with (
            patch.object(subject, "TEST_BINDINGS", bindings),
            patch.object(subject, "REVIEWED_ORACLE_EXPRESSIONS", reviewed),
            patch.object(subject, "_canonical_inventories", return_value=inventory),
            patch.object(subject, "_behavior_indexes", return_value={("INV", "PART"): 0}),
        ):
            with self.assertRaisesRegex(ValueError, "unreviewed product-oracle"):
                subject.generate_assignments(Path("."), {"inventories": inventory})

    def test_setup_assertions_are_separated_from_reviewed_oracles(self) -> None:
        path = "crate/tests.rs"
        name = "case"
        inventory = {
            "DISC": {
                "path": path,
                "test_name": name,
                "unresolved_assertion_helpers": [],
                "assertions": [
                    {"assertion_id": "SETUP", "expression": "make () . unwrap ()"},
                    {"assertion_id": "ORACLE", "expression": "assert_eq ! (value , 1)"},
                ],
            }
        }
        bindings = {name: (path, "INV", "PART")}
        reviewed = {name: ("assert_eq ! (value , 1)",)}
        with (
            patch.object(subject, "TEST_BINDINGS", bindings),
            patch.object(subject, "REVIEWED_ORACLE_EXPRESSIONS", reviewed),
            patch.object(subject, "_canonical_inventories", return_value=inventory),
            patch.object(subject, "_behavior_indexes", return_value={("INV", "PART"): 0}),
        ):
            document = subject.generate_assignments(Path("."), {"inventories": inventory})
        self.assertEqual(document["assignments"][0]["setup_assertion_ids"], ["SETUP"])


if __name__ == "__main__":
    unittest.main()
