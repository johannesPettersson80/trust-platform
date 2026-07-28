"""Focused tests for the Phase 18 protocol exact assignment."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.protocol_behavior_proof import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]
SCAN_REPORT = ROOT / (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-28/"
    "p18-runtime-other-exact-behavior-106/live-scan.json"
)


class ProtocolBehaviorProofTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(
            root=ROOT,
            gap_report=ROOT / "missing-generated-gap-report.json",
            scan_report=SCAN_REPORT,
        )

    def test_complete_protocol_population_is_resolved(self) -> None:
        self.assertEqual(29, self.plan["demand_count"])
        self.assertEqual(32, self.plan["route_name_count"])
        self.assertEqual(32, self.plan["assigned_test_count"])
        self.assertEqual(29, self.plan["existing_catalog_test_count"])
        self.assertEqual(3, self.plan["new_catalog_test_count"])
        self.assertEqual(35, self.plan["assignment_link_count"])
        self.assertEqual(
            {"reviewed_nonmapping": 3},
            self.plan["new_denominator_dispositions"],
        )
        self.assertEqual(
            {"RT_OPCUA_SERVER_SURFACE_001": 3},
            self.plan["new_tests_by_invariant"],
        )
        self.assertEqual(
            38,
            self.plan["expected_post_transaction_missing_tests"],
        )
        self.assertEqual([], self.plan["unresolved"])

    def test_every_assignment_is_a_live_exact_identity(self) -> None:
        discovery_ids = [
            row["discovery_id"] for row in self.plan["assignments"]
        ]
        self.assertEqual(32, len(discovery_ids))
        self.assertEqual(32, len(set(discovery_ids)))
        self.assertTrue(
            all(row["behavior_refs"] for row in self.plan["assignments"])
        )


if __name__ == "__main__":
    unittest.main()
