import unittest

from scripts.verification.runtime_retain_assignments import (
    BEHAVIOR_BY_TEST,
    _is_non_oracle,
    _is_setup,
)


class RuntimeRetainAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_fifteen_tests(self) -> None:
        self.assertEqual(15, len(BEHAVIOR_BY_TEST))

    def test_harness_compilation_is_setup(self) -> None:
        self.assertTrue(
            _is_setup(
                name="retain_scalar_widening_migrates_with_runtime_event",
                expression='TestHarness :: from_source (source) . expect ("compile harness")',
            )
        )

    def test_migration_event_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_non_oracle(
                name="retain_scalar_widening_migrates_with_runtime_event",
                expression='format ! ("{event:?}") . contains ("RetainMigrationApplied")',
            )
        )

    def test_error_message_substring_is_not_product_oracle(self) -> None:
        self.assertTrue(
            _is_non_oracle(
                name="file_retain_store_open_and_read_failures_are_visible",
                expression="error . to_string () . contains (stage . label ())",
            )
        )


if __name__ == "__main__":
    unittest.main()
