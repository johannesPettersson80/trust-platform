"""Focused tests for the Phase 18 runtime-cloud mapping transaction."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.runtime_cloud_behavior_mapping import (
    compile_runtime_cloud_mapping,
)


ROOT = Path(__file__).resolve().parents[2]


class RuntimeCloudBehaviorMappingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.compilation = compile_runtime_cloud_mapping(root=ROOT)

    def test_transaction_closes_the_complete_23_behavior_group(self) -> None:
        self.assertEqual(
            {
                "behavior_demand_count": 23,
                "campaign_test_count": 52,
                "existing_catalog_test_count": 52,
                "confirmed_missing_test_count": 67,
                "touched_invariant_count": 7,
            },
            self.compilation.summary,
        )

    def test_transaction_is_one_recoverable_metadata_replacement_set(self) -> None:
        self.assertIn(
            "verification/test-catalog.toml",
            self.compilation.replacements,
        )
        self.assertIn(
            "verification/p18/review-locks/"
            "runtime-cloud-23-semantic-lock.json",
            self.compilation.replacements,
        )
        lock = json.loads(
            self.compilation.replacements[
                "verification/p18/review-locks/"
                "runtime-cloud-23-semantic-lock.json"
            ]
        )
        self.assertEqual(7, len(lock["touched_invariant_paths"]))


if __name__ == "__main__":
    unittest.main()
