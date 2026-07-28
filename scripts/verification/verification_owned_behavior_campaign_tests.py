"""Focused tests for the exact Phase 18 verification-owned campaign."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.verification_owned_behavior_campaign import (
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
    build_campaign_plan,
)


ROOT = Path(__file__).resolve().parents[2]


class VerificationOwnedBehaviorCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_campaign_plan(
            root=ROOT,
            gap_report=ROOT / DEFAULT_GAP_REPORT,
            scan_report=ROOT / DEFAULT_SCAN_REPORT,
        )

    def test_campaign_executes_every_selected_test_once(self) -> None:
        self.assertEqual(11, self.plan["behavior_count"])
        self.assertEqual(15, self.plan["test_count"])
        test_ids = [
            test_id
            for group in self.plan["groups"]
            for test_id in group["test_ids"]
        ]
        self.assertEqual(15, len(test_ids))
        self.assertEqual(15, len(set(test_ids)))

    def test_campaign_uses_two_exact_nextest_groups(self) -> None:
        self.assertEqual(2, self.plan["group_count"])
        self.assertEqual(
            {
                ("trust-hir", "test", "iec_table_coverage", 3),
                ("verification-cases", "lib", None, 12),
            },
            {
                (
                    group["package"],
                    group["target_kind"],
                    group["target_name"],
                    group["test_count"],
                )
                for group in self.plan["groups"]
            },
        )
        for group in self.plan["groups"]:
            self.assertEqual(
                ["cargo", "nextest", "run"],
                group["command"][:3],
            )
            self.assertTrue(
                all("test(=" in term for term in group["exact_terms"])
            )


if __name__ == "__main__":
    unittest.main()
