import unittest

from scripts.verification.ads_symbols_assignments import (
    BEHAVIOR_BY_TEST,
    _is_non_oracle,
    _is_setup,
)


class AdsSymbolsAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_five_tests(self) -> None:
        self.assertEqual(5, len(BEHAVIOR_BY_TEST))

    def test_duplicate_expect_err_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                name="duplicate_symbol_names_are_rejected",
                expression='build_server_symbol_snapshot (...) . expect_err ("duplicate")',
            )
        )

    def test_terminal_newline_is_non_oracle_formatting(self) -> None:
        self.assertTrue(
            _is_non_oracle(
                name="deterministic_snapshot_json_is_stable",
                expression="assert ! (first . ends_with ('\\n'))",
            )
        )


if __name__ == "__main__":
    unittest.main()
