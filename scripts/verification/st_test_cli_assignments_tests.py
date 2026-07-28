from __future__ import annotations

import unittest

from scripts.verification.st_test_cli_assignments import (
    CASE_DEADLINE,
    HUMAN_REPORTS,
    REPORT,
    TEST_BINDINGS,
    _behavior_for_assertion,
    _disposition,
)


class StTestCliAssignmentsTests(unittest.TestCase):
    def test_closed_batch_is_exactly_seven_tests(self) -> None:
        self.assertEqual(7, len(TEST_BINDINGS))
        self.assertNotIn("trust_runtime_test_alias_forwards_to_trust_dev", TEST_BINDINGS)

    def test_macro_assertions_are_product_oracles(self) -> None:
        self.assertEqual(
            "oracle_bound",
            _disposition(
                test_name="json_output_includes_duration_fields",
                function="json_output_includes_duration_fields",
                kind="macro:assert",
                expression="assert!(...)",
            ),
        )

    def test_json_shape_expectations_are_product_oracles(self) -> None:
        for expression in (
            "serde_json :: from_slice (& output . stdout) . expect (...)",
            'payload ["tests"] . as_array () . expect (...)',
        ):
            self.assertEqual(
                "oracle_bound",
                _disposition(
                    test_name="json_output_includes_duration_fields",
                    function="json_output_includes_duration_fields",
                    kind="method:expect",
                    expression=expression,
                ),
            )

    def test_process_and_filesystem_expectations_are_setup(self) -> None:
        self.assertEqual(
            "setup_or_precondition",
            _disposition(
                test_name="filter_zero_message_is_clear_in_human_output",
                function="filter_zero_message_is_clear_in_human_output",
                kind="method:expect",
                expression="command.output().expect(...)",
            ),
        )

    def test_timeout_prose_routes_separately_from_timeout_status(self) -> None:
        name = "timeout_flag_reports_error_for_infinite_loop_test"
        self.assertEqual(
            (REPORT, HUMAN_REPORTS),
            _behavior_for_assertion(
                test_name=name,
                expression="assert!(text.contains(\"test timed out after 1 second\"))",
                disposition="oracle_bound",
            ),
        )
        self.assertEqual(
            ("DEV_TEST_EXECUTION_001", CASE_DEADLINE),
            _behavior_for_assertion(
                test_name=name,
                expression="assert!(!output.status.success())",
                disposition="oracle_bound",
            ),
        )


if __name__ == "__main__":
    unittest.main()
