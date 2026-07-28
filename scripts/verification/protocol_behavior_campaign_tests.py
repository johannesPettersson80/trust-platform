"""Focused tests for the Phase 18 exact protocol campaign."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.protocol_behavior_campaign import (
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
    build_campaign_plan,
)


ROOT = Path(__file__).resolve().parents[2]


class ProtocolBehaviorCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_campaign_plan(
            root=ROOT,
            gap_report=ROOT / DEFAULT_GAP_REPORT,
            scan_report=ROOT / DEFAULT_SCAN_REPORT,
        )

    def test_campaign_executes_every_selected_test_once(self) -> None:
        self.assertEqual(29, self.plan["behavior_count"])
        self.assertEqual(32, self.plan["test_count"])
        test_ids = [
            test_id
            for group in self.plan["groups"]
            for test_id in group["test_ids"]
        ]
        self.assertEqual(32, len(test_ids))
        self.assertEqual(32, len(set(test_ids)))

    def test_campaign_uses_four_exact_nextest_groups(self) -> None:
        self.assertEqual(4, self.plan["group_count"])
        for group in self.plan["groups"]:
            self.assertEqual(
                ["cargo", "nextest", "run"],
                group["command"][:3],
            )
            self.assertEqual(group["test_count"], len(group["full_names"]))
            self.assertTrue(
                all("test(=" in term for term in group["exact_terms"])
            )

    def test_opcua_server_promotions_use_the_wire_integration_target(self) -> None:
        server_group = next(
            group
            for group in self.plan["groups"]
            if group["target_name"] == "opcua_integration"
        )
        self.assertEqual(3, server_group["test_count"])
        self.assertEqual("test", server_group["target_kind"])


if __name__ == "__main__":
    unittest.main()
