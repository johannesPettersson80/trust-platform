import unittest

from scripts.verification.ads_ams_assignments import (
    BEHAVIOR_BY_TEST,
    _is_setup,
)


class AdsAmsAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_eleven_tests(self) -> None:
        self.assertEqual(11, len(BEHAVIOR_BY_TEST))

    def test_request_parse_is_setup_for_response_header(self) -> None:
        self.assertTrue(
            _is_setup(
                name="response_header_swaps_source_and_target",
                expression=(
                    'AmsTcpFrame :: parse (& read_request_bytes (), 4096) '
                    '. expect ("parse request")'
                ),
            )
        )

    def test_exact_roundtrip_bytes_are_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                name="ams_frame_round_trips_valid_read_request_bytes",
                expression=(
                    'assert_eq ! (frame . to_bytes () . expect ("serialize") , '
                    "bytes)"
                ),
            )
        )


if __name__ == "__main__":
    unittest.main()
