"""Focused tests for the Phase 18 ADS exact-behavior proof campaign."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.ads_behavior_proof_assignments import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-remaining-267/frozen-gaps.json"
)


class AdsBehaviorProofAssignmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(root=ROOT, gap_report=GAP_REPORT)

    def test_complete_ads_behavior_population_is_classified(self) -> None:
        self.assertEqual(71, self.plan["demand_count"])
        self.assertEqual(71, self.plan["assigned_behavior_count"])
        self.assertEqual([], self.plan["residual_behavior_refs"])

    def test_every_assignment_uses_an_exact_linked_catalog_test(self) -> None:
        for assignment in self.plan["assignments"]:
            self.assertTrue(assignment["test_ids"])
            self.assertTrue(
                all(
                    assignment["invariant_id"]
                    in test["invariants"]
                    for test in assignment["tests"]
                )
            )

    def test_assignments_cover_only_the_frozen_ads_demands(self) -> None:
        frozen = {
            row["behavior_ref"]
            for row in json.loads(GAP_REPORT.read_text())[
                "confirmed_missing_tests"
            ]
            if "ADS_" in row["invariant_id"]
        }
        assigned = {
            row["behavior_ref"] for row in self.plan["assignments"]
        }
        self.assertEqual(
            frozen,
            assigned | set(self.plan["residual_behavior_refs"]),
        )
        self.assertFalse(
            assigned & set(self.plan["residual_behavior_refs"])
        )

    def test_catalog_replacements_close_exactly_seventy_one_demands(self) -> None:
        after = self.plan["post_assignment_gap_summary"]
        self.assertEqual(0, after["missing_spec_authority_tests"])
        self.assertEqual(196, after["confirmed_missing_tests"])


if __name__ == "__main__":
    unittest.main()
