"""Focused tests for the Phase 18 runtime-cloud exact campaign."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.runtime_cloud_behavior_campaign import (
    build_campaign_plan,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/frozen-catalog-gaps/report.json"
)
SCAN_REPORT = ROOT / (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-28/"
    "p18-runtime-other-exact-behavior-106/live-scan.json"
)


class RuntimeCloudBehaviorCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_campaign_plan(
            root=ROOT,
            gap_report=GAP_REPORT,
            scan_report=SCAN_REPORT,
        )

    def test_campaign_executes_every_selected_test_once(self) -> None:
        self.assertEqual(23, self.plan["behavior_count"])
        self.assertEqual(52, self.plan["test_count"])
        test_ids = [
            test_id
            for group in self.plan["groups"]
            for test_id in group["test_ids"]
        ]
        self.assertEqual(52, len(test_ids))
        self.assertEqual(52, len(set(test_ids)))

    def test_campaign_uses_only_exact_nextest_groups(self) -> None:
        self.assertGreater(self.plan["group_count"], 0)
        self.assertLessEqual(self.plan["group_count"], 4)
        for group in self.plan["groups"]:
            self.assertEqual(
                ["cargo", "nextest", "run"],
                group["command"][:3],
            )
            self.assertEqual(group["test_count"], len(group["full_names"]))
            self.assertTrue(
                all("test(=" in term for term in group["exact_terms"])
            )

    def test_split_web_integration_tests_use_their_module_names(self) -> None:
        selected = {
            test_id: full_name
            for group in self.plan["groups"]
            for full_name, test_id in group["tests_by_full_name"].items()
        }
        expected = {
            "TEST_RUNTIME_CLOUD_CONFIG_AGENT_RECONCILES_DESIRED_REPORTED_META_AND_STATUS_001": (
                "web_io_config_integration_part_08::"
                "runtime_cloud_config_agent_reconciles_desired_reported_meta_and_status"
            ),
            "TEST_RUNTIME_CLOUD_PREFLIGHT_ALLOWS_CROSS_SITE_CFG_APPLY_WITH_ALLOWLIST_001": (
                "web_io_config_integration_part_14::"
                "runtime_cloud_preflight_allows_cross_site_cfg_apply_with_allowlist"
            ),
            "TEST_RUNTIME_CLOUD_PREFLIGHT_DEV_ALLOWS_CROSS_SITE_CFG_APPLY_WITHOUT_ALLOWLIST_001": (
                "web_io_config_integration_part_14::"
                "runtime_cloud_preflight_dev_allows_cross_site_cfg_apply_without_allowlist"
            ),
        }
        for test_id, full_name in expected.items():
            self.assertEqual(full_name, selected[test_id])


if __name__ == "__main__":
    unittest.main()
