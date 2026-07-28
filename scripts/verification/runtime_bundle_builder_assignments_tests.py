from __future__ import annotations

import unittest

from scripts.verification.runtime_bundle_builder_assignments import (
    TEST_BINDINGS,
    _disposition,
)


class RuntimeBundleBuilderAssignmentsTests(unittest.TestCase):
    def test_closed_batch_is_exactly_seven_tests(self) -> None:
        self.assertEqual(7, len(TEST_BINDINGS))
        self.assertNotIn(
            "dependency_resolution_order_is_deterministic",
            TEST_BINDINGS,
        )
        self.assertNotIn(
            "check_compiles_without_writing_program_stbc",
            TEST_BINDINGS,
        )

    def test_test_body_assertions_are_product_oracles(self) -> None:
        for kind in (
            "method:expect",
            "method:expect_err",
            "macro:assert",
            "macro:assert_eq",
        ):
            self.assertEqual(
                "oracle_bound",
                _disposition(
                    test_name="reviewed_test",
                    function="reviewed_test",
                    kind=kind,
                ),
            )

    def test_temp_and_write_helpers_are_setup_only(self) -> None:
        for function, kind in (
            ("temp_dir", "method:unwrap"),
            ("temp_dir", "method:expect"),
            ("write_file", "method:expect"),
        ):
            self.assertEqual(
                "setup_or_precondition",
                _disposition(
                    test_name="reviewed_test",
                    function=function,
                    kind=kind,
                ),
            )

    def test_unknown_helper_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            _disposition(
                test_name="reviewed_test",
                function="other_helper",
                kind="method:expect",
            )


if __name__ == "__main__":
    unittest.main()
