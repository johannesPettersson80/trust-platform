"""Focused tests for the Phase 18 protocol mapping transaction."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.protocol_behavior_mapping import (
    compile_protocol_mapping,
)


ROOT = Path(__file__).resolve().parents[2]


class ProtocolBehaviorMappingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.compilation = compile_protocol_mapping(root=ROOT)

    def test_transaction_closes_the_complete_29_behavior_group(self) -> None:
        self.assertEqual(
            {
                "behavior_demand_count": 29,
                "campaign_test_count": 32,
                "existing_catalog_test_count": 29,
                "new_catalog_test_count": 3,
                "updated_mapping_provenance_count": 1,
                "confirmed_missing_test_count": 38,
                "touched_invariant_count": 7,
                "catalog_mapped_count": 3921,
                "reviewed_nonmapping_count": 478,
            },
            self.compilation.summary,
        )

    def test_transaction_is_one_recoverable_metadata_replacement_set(self) -> None:
        required = {
            "verification/test-catalog.toml",
            "verification/test-catalog-denominator.toml",
            "verification/test-catalog-proposals.toml",
            "verification/test-catalog-mapping-decisions.toml",
            "verification/p18/review-locks/"
            "protocol-29-semantic-lock.json",
        }
        self.assertLessEqual(
            required,
            set(self.compilation.replacements),
        )
        lock = json.loads(
            self.compilation.replacements[
                "verification/p18/review-locks/"
                "protocol-29-semantic-lock.json"
            ]
        )
        self.assertEqual(7, len(lock["touched_invariant_paths"]))
        self.assertEqual(
            3,
            sum(
                row["resolution"] == "new_catalog"
                for row in lock["assignments"]
            ),
        )


if __name__ == "__main__":
    unittest.main()
