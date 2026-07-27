import unittest

from scripts.verification.ads_listener_assignments import (
    DEFAULT_BEHAVIOR,
    ONBOARDING,
    _behavior_for_assertion,
    _is_setup,
)


class AdsListenerAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_only_eight_clean_tests(self) -> None:
        self.assertEqual(8, len(DEFAULT_BEHAVIOR))
        self.assertNotIn(
            "tcp_listener_serves_router_tcpip_metadata_read",
            DEFAULT_BEHAVIOR,
        )

    def test_net_id_rejection_maps_to_malformed_behavior(self) -> None:
        self.assertEqual(
            (ONBOARDING, 2),
            _behavior_for_assertion(
                name="ams_net_id_text_conversion_round_trips",
                expression=(
                    'assert ! (ams_net_id_text_to_bytes ("bad") . is_err ())'
                ),
                default=(ONBOARDING, 0),
            ),
        )

    def test_bind_failure_panic_sentinel_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                name="tcp_listener_reports_bind_conflict",
                expression='panic ! ("bind conflict should fail")',
            )
        )


if __name__ == "__main__":
    unittest.main()
