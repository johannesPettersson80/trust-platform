"""Focused contract tests for the Phase 18 runtime-cloud assignment."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.runtime_cloud_behavior_proof import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/frozen-catalog-gaps/report.json"
)
SCAN_REPORT = ROOT / (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-28/"
    "p18-runtime-other-exact-behavior-106/live-scan.json"
)


class RuntimeCloudBehaviorProofTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(
            root=ROOT,
            gap_report=GAP_REPORT,
            scan_report=SCAN_REPORT,
        )

    def test_complete_runtime_cloud_authority_group_is_assigned(self) -> None:
        self.assertEqual(23, self.plan["demand_count"])
        self.assertGreaterEqual(self.plan["assigned_test_count"], 40)
        self.assertEqual([], self.plan["unresolved"])
        self.assertEqual(67, self.plan["expected_post_transaction_missing_tests"])

    def test_every_selected_test_reuses_an_exact_live_catalog_identity(self) -> None:
        self.assertEqual(
            self.plan["assigned_test_count"],
            self.plan["existing_catalog_test_count"],
        )
        self.assertEqual(0, self.plan["new_catalog_test_count"])
        self.assertEqual({}, self.plan["new_denominator_dispositions"])
        for assignment in self.plan["assignments"]:
            self.assertEqual("existing_catalog", assignment["resolution"])
            self.assertRegex(assignment["discovery_id"], r"^DISC_[A-F0-9]{20}$")
            self.assertGreaterEqual(len(assignment["behavior_refs"]), 1)

    def test_all_seven_runtime_cloud_authorities_are_closed_together(self) -> None:
        self.assertEqual(
            {
                "RT_MANAGED_FLEET_LIFECYCLE_001",
                "RT_RUNTIME_CLOUD_ACTION_POLICY_001",
                "RT_RUNTIME_CLOUD_CONFIG_RECONCILE_001",
                "RT_RUNTIME_CLOUD_HA_FENCING_001",
                "RT_RUNTIME_CLOUD_PROXY_PLAN_001",
                "RT_RUNTIME_CLOUD_SCHEMA_KEYSPACE_001",
                "RT_RUNTIME_CLOUD_TOPOLOGY_LINK_POLICY_001",
            },
            {
                invariant_id
                for assignment in self.plan["assignments"]
                for invariant_id in assignment["invariant_ids"]
            },
        )


if __name__ == "__main__":
    unittest.main()
