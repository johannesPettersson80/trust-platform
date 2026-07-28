"""Focused contract tests for the Phase 18 runtime-other assignment plan."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.runtime_other_behavior_proof import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-ads-71/"
    "final-frozen-gaps.json"
)
SCAN_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-runtime-other-106/"
    "live-scan.json"
)


class RuntimeOtherBehaviorProofTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(
            root=ROOT,
            gap_report=GAP_REPORT,
            scan_report=SCAN_REPORT,
        )

    def test_frozen_population_is_exact_and_exhaustive(self) -> None:
        self.assertEqual(106, self.plan["demand_count"])
        self.assertEqual(196, self.plan["route_name_count"])
        self.assertEqual(196, self.plan["assigned_test_count"])
        self.assertEqual(230, self.plan["assignment_link_count"])
        self.assertEqual([], self.plan["unresolved"])

    def test_exact_live_resolution_separates_reuse_from_promotion(self) -> None:
        self.assertEqual(163, self.plan["existing_catalog_test_count"])
        self.assertEqual(33, self.plan["new_catalog_test_count"])
        self.assertEqual(
            {
                "absent_from_denominator": 4,
                "reviewed_nonmapping": 29,
            },
            self.plan["new_denominator_dispositions"],
        )
        self.assertEqual(
            {
                "OSCAT_CLASSIC_LIBRARY_PROFILE_001": 1,
                "RT_GPIO_DRIVER_001": 1,
                "RT_OPENOT_CONSUMER_RECONCILIATION_001": 1,
                "RT_OPENOT_EVENT_MATERIALIZATION_001": 16,
                "RT_OPENOT_ST_PRODUCER_CONFORMANCE_001": 9,
                "RT_OPENOT_TELEMETRY_PUBLICATION_001": 3,
                "RT_PLAY_LAUNCH_001": 2,
            },
            self.plan["new_tests_by_invariant"],
        )

    def test_every_assignment_is_bound_to_an_exact_scanner_identity(self) -> None:
        for assignment in self.plan["assignments"]:
            self.assertRegex(
                assignment["discovery_id"],
                r"^DISC_[A-F0-9]{20}$",
            )
            self.assertNotEqual("", assignment["path"])
            self.assertNotEqual("", assignment["name"])
            self.assertIn(
                assignment["resolution"],
                {"existing_catalog", "new_catalog"},
            )
            self.assertGreaterEqual(len(assignment["behavior_refs"]), 1)


if __name__ == "__main__":
    unittest.main()
