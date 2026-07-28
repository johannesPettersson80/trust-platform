from __future__ import annotations

import unittest

from scripts.verification.ads_core_quality_assignments import (
    ERROR_TO_STALE,
    GOOD_TO_ERROR,
    STALE_TO_GOOD,
    TEST_BINDINGS,
    _behavior_for_assertion,
)


class AdsCoreQualityAssignmentsTests(unittest.TestCase):
    def test_closed_batch_is_exactly_three_tests(self) -> None:
        self.assertEqual(3, len(TEST_BINDINGS))

    def test_transition_assertions_split_by_observed_phase(self) -> None:
        name = "quality_transitions_clear_and_preserve_fields"
        for index in range(3):
            self.assertEqual(
                STALE_TO_GOOD,
                _behavior_for_assertion(name=name, index=index),
            )
        for index in range(3, 6):
            self.assertEqual(
                GOOD_TO_ERROR,
                _behavior_for_assertion(name=name, index=index),
            )
        for index in range(6, 9):
            self.assertEqual(
                ERROR_TO_STALE,
                _behavior_for_assertion(name=name, index=index),
            )

    def test_unreviewed_transition_assertion_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "drifted"):
            _behavior_for_assertion(
                name="quality_transitions_clear_and_preserve_fields",
                index=9,
            )


if __name__ == "__main__":
    unittest.main()
