"""Focused tests for the verification-owned mapping transaction."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.verification_owned_behavior_mapping import (
    compile_verification_owned_mapping,
)


ROOT = Path(__file__).resolve().parents[2]


class VerificationOwnedBehaviorMappingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.compilation = compile_verification_owned_mapping(root=ROOT)

    def test_transaction_closes_the_complete_behavior_group(self) -> None:
        self.assertEqual(
            {
                "behavior_demand_count": 11,
                "campaign_test_count": 15,
                "existing_catalog_test_count": 3,
                "new_catalog_test_count": 12,
                "updated_mapping_provenance_count": 3,
                "confirmed_missing_test_count": 27,
                "touched_invariant_count": 2,
                "catalog_mapped_count": 3933,
                "reviewed_nonmapping_count": 468,
            },
            self.compilation.summary,
        )

    def test_transaction_is_one_recoverable_replacement_set(self) -> None:
        lock_path = (
            "verification/p18/review-locks/"
            "verification-owned-11-semantic-lock.json"
        )
        self.assertLessEqual(
            {
                "verification/test-catalog.toml",
                "verification/test-catalog-denominator.toml",
                "verification/test-catalog-proposals.toml",
                "verification/test-catalog-mapping-decisions.toml",
                lock_path,
            },
            set(self.compilation.replacements),
        )
        lock = json.loads(self.compilation.replacements[lock_path])
        self.assertEqual(2, len(lock["touched_invariant_paths"]))
        self.assertEqual(3, len(lock["refreshed_catalog_test_ids"]))
        self.assertEqual(
            12,
            sum(
                row["resolution"] == "new_catalog"
                for row in lock["assignments"]
            ),
        )


if __name__ == "__main__":
    unittest.main()
