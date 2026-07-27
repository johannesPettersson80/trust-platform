import unittest

from scripts.verification.runtime_runner_loop_assignments import (
    BEHAVIOR_BY_TEST,
    SAFE_DEADLINE,
    _behavior_for_assertion,
    _is_non_oracle,
    _is_setup,
)


class RuntimeRunnerLoopAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_only_eight_clean_tests(self) -> None:
        self.assertEqual(8, len(BEHAVIOR_BY_TEST))
        self.assertNotIn(
            "state_and_error_helpers_recover_poisoned_mutexes",
            BEHAVIOR_BY_TEST,
        )

    def test_zero_timeout_expect_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                name=(
                    "cycle_deadline_from_zero_timeout_is_immediately_expired"
                ),
                expression=(
                    'cycle_deadline_from (start, Duration :: ZERO) '
                    '. expect ("zero timeout still creates a deadline")'
                ),
            )
        )

    def test_contained_panic_is_stimulus_not_oracle(self) -> None:
        self.assertTrue(
            _is_setup(
                name=(
                    "cycle_deadline_guard_restores_after_runtime_error_and_"
                    "contained_panic"
                ),
                expression='panic ! ("contained panic before deadline restore")',
            )
        )

    def test_helper_debug_assert_is_not_test_oracle(self) -> None:
        self.assertTrue(
            _is_non_oracle(
                name="scheduler_arms_execution_deadline_during_cycle",
                expression="debug_assert ! (sentinel . panicked_mid_cycle ())",
            )
        )

    def test_restore_harness_control_assertion_is_not_product_oracle(
        self,
    ) -> None:
        self.assertTrue(
            _is_non_oracle(
                name=(
                    "cycle_deadline_guard_restores_after_runtime_error_and_"
                    "contained_panic"
                ),
                expression=(
                    "assert ! (result . is_ok () || outcome == \"panic\")"
                ),
            )
        )

    def test_restore_precondition_maps_to_arm_behavior(self) -> None:
        self.assertEqual(
            (SAFE_DEADLINE, 2),
            _behavior_for_assertion(
                name=(
                    "cycle_deadline_guard_restores_after_runtime_error_and_"
                    "contained_panic"
                ),
                expression=(
                    "assert ! (runtime . execution_deadline () . is_some ())"
                ),
                default=(SAFE_DEADLINE, 5),
            ),
        )


if __name__ == "__main__":
    unittest.main()
