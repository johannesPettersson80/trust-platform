"""Focused tests for the Phase 18 verification-owned assignment."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.verification_owned_behavior_campaign import (
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
)
from scripts.verification.verification_owned_behavior_proof import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]


class VerificationOwnedBehaviorProofTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(
            root=ROOT,
            gap_report=ROOT / DEFAULT_GAP_REPORT,
            scan_report=ROOT / DEFAULT_SCAN_REPORT,
        )

    def test_complete_population_is_resolved(self) -> None:
        self.assertEqual(11, self.plan["demand_count"])
        self.assertEqual(15, self.plan["route_name_count"])
        self.assertEqual(15, self.plan["assigned_test_count"])
        self.assertEqual(3, self.plan["existing_catalog_test_count"])
        self.assertEqual(12, self.plan["new_catalog_test_count"])
        self.assertEqual(17, self.plan["assignment_link_count"])
        self.assertEqual(
            {
                "absent_from_denominator": 2,
                "reviewed_nonmapping": 10,
            },
            self.plan["new_denominator_dispositions"],
        )
        self.assertEqual(27, self.plan["expected_post_transaction_missing_tests"])
        self.assertEqual([], self.plan["unresolved"])

    def test_every_assignment_is_one_live_exact_identity(self) -> None:
        discovery_ids = [
            row["discovery_id"] for row in self.plan["assignments"]
        ]
        self.assertEqual(15, len(discovery_ids))
        self.assertEqual(15, len(set(discovery_ids)))
        self.assertTrue(
            all(row["behavior_refs"] for row in self.plan["assignments"])
        )


if __name__ == "__main__":
    unittest.main()
