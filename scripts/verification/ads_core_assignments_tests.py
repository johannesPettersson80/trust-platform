from __future__ import annotations

import unittest

from scripts.verification.ads_core_assignments import (
    EXTENT_MATCH,
    EXTENT_MISMATCH,
    TEST_BINDINGS,
    _behavior_for_assertion,
)


class AdsCoreAssignmentsTests(unittest.TestCase):
    def test_closed_batch_is_exactly_six_tests(self) -> None:
        self.assertEqual(6, len(TEST_BINDINGS))
        self.assertEqual(
            {
                "crates/trust-ads-core/src/mapping.rs",
                "crates/trust-ads-core/src/symbols.rs",
            },
            {path for path, _name in TEST_BINDINGS},
        )

    def test_endpoint_extent_assertions_split_accept_and_reject(self) -> None:
        name = "validates_endpoint_byte_size_against_type_descriptor"
        self.assertEqual(
            EXTENT_MATCH,
            _behavior_for_assertion(
                name=name,
                default=EXTENT_MATCH,
                expression="assert_eq!(symbol.validate_byte_size(), Ok(()))",
            ),
        )
        self.assertEqual(
            EXTENT_MISMATCH,
            _behavior_for_assertion(
                name=name,
                default=EXTENT_MATCH,
                expression="assert!(matches!(..., ByteSizeMismatch { .. }))",
            ),
        )

    def test_other_assertions_keep_test_behavior(self) -> None:
        self.assertEqual(
            "FIXED_WIDTH_SCALAR_ROUNDTRIP",
            _behavior_for_assertion(
                name="scalar_mapping_round_trips_every_supported_type",
                default="FIXED_WIDTH_SCALAR_ROUNDTRIP",
                expression="assert_eq!(...)",
            ),
        )


if __name__ == "__main__":
    unittest.main()
