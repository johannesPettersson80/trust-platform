import unittest

from scripts.verification.ads_notify_assignments import (
    BEHAVIOR_BY_TEST,
    _is_setup,
)


class AdsNotifyAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_seven_tests(self) -> None:
        self.assertEqual(7, len(BEHAVIOR_BY_TEST))

    def test_sampling_success_extraction_is_setup(self) -> None:
        self.assertTrue(
            _is_setup(
                name="read_error_invalidates_handle",
                expression=(
                    "sampler . sample (& [registration] , & symbols , "
                    '& FailingValues , 10 , Duration :: from_millis (1)) '
                    '. expect ("sample")'
                ),
            )
        )

    def test_panicking_source_is_negative_side_effect_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                name="system_bytes_notification_samples_without_runtime_read",
                expression=(
                    'panic ! ("static system notifications must not read '
                    'runtime storage")'
                ),
            )
        )


if __name__ == "__main__":
    unittest.main()
