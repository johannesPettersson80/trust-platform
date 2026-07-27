import unittest

from scripts.verification.runtime_safety_fail_closed_assignments import (
    BEHAVIOR_BY_TEST,
    SAFE_PANIC,
    _is_non_oracle,
    _is_setup,
)


class RuntimeSafetyFailClosedAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_twelve_tests(self) -> None:
        self.assertEqual(12, len(BEHAVIOR_BY_TEST))

    def test_runner_lifecycle_is_setup(self) -> None:
        self.assertTrue(
            _is_setup(
                expression='runner . spawn ("fault") . expect ("spawn runner")'
            )
        )
        self.assertTrue(
            _is_setup(
                expression='handle . join () . expect ("runner join")'
            )
        )

    def test_pending_output_assertion_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                expression=(
                    'assert ! (writes . lock () . expect ("driver writes lock") '
                    '. is_empty (), "watchdog breach before output commit must '
                    'prevent physical output writes")'
                )
            )
        )

    def test_weak_error_substring_is_not_product_oracle(self) -> None:
        self.assertTrue(
            _is_non_oracle(
                name="requested_stop_retain_save_failure_is_visible",
                expression='error . to_string () . contains ("retain")',
            )
        )

    def test_mixed_test_starts_with_deliberate_stop_behavior(self) -> None:
        self.assertEqual(
            ("RT_SAFE_STOP_001", 1),
            BEHAVIOR_BY_TEST[
                "deliberate_stop_and_fault_safe_state_paths_are_explicit"
            ],
        )
        self.assertEqual("RT_SAFE_PANIC_001", SAFE_PANIC)


if __name__ == "__main__":
    unittest.main()
