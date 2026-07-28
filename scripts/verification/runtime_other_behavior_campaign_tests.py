"""Focused tests for the Phase 18 runtime-other exact campaign."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.runtime_other_behavior_campaign import (
    build_campaign_plan,
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


class RuntimeOtherBehaviorCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_campaign_plan(
            root=ROOT,
            gap_report=GAP_REPORT,
            scan_report=SCAN_REPORT,
        )

    def test_campaign_covers_the_complete_frozen_assignment_once(self) -> None:
        self.assertEqual(106, self.plan["behavior_count"])
        self.assertEqual(196, self.plan["test_count"])
        test_ids = [
            test_id
            for group in self.plan["groups"]
            for test_id in group["test_ids"]
        ]
        self.assertEqual(196, len(test_ids))
        self.assertEqual(196, len(set(test_ids)))

    def test_campaign_uses_a_small_number_of_exact_nextest_groups(self) -> None:
        self.assertGreater(self.plan["group_count"], 0)
        self.assertLess(self.plan["group_count"], 30)
        for group in self.plan["groups"]:
            command = group["command"]
            self.assertEqual(["cargo", "nextest", "run"], command[:3])
            self.assertIn("--no-fail-fast", command)
            self.assertIn("--message-format", command)
            self.assertIn("-E", command)
            self.assertEqual(group["test_count"], len(group["full_names"]))
            self.assertEqual(
                group["test_count"],
                len(group["tests_by_full_name"]),
            )
            self.assertTrue(
                all("test(=" in term for term in group["exact_terms"])
            )

    def test_new_behavior_locks_have_exact_target_identities(self) -> None:
        selected = {
            test_id: (group, full_name)
            for group in self.plan["groups"]
            for full_name, test_id in group["tests_by_full_name"].items()
        }
        self.assertEqual(
            (
                "trust-runtime::trust-runtime",
                "run::tests::play_project_resolution_distinguishes_absence_from_detection_failure",
            ),
            (
                selected["DISC_5DFF0A06A903D91AC7DD"][0]["binary_id"],
                selected["DISC_5DFF0A06A903D91AC7DD"][1],
            ),
        )
        self.assertEqual(
            (
                "trust-runtime",
                "io::gpio::tests::gpio_safe_state_handoff_writes_and_confirms_backend_output",
            ),
            (
                selected["DISC_F521BAD899EFD819471D"][0]["binary_id"],
                selected["DISC_F521BAD899EFD819471D"][1],
            ),
        )
        self.assertEqual(
            (
                "trust-dev::cli_smoke",
                "oscat_replaced_upstream_name_remains_absent",
            ),
            (
                selected["DISC_FD2311D3995B059E552D"][0]["binary_id"],
                selected["DISC_FD2311D3995B059E552D"][1],
            ),
        )


if __name__ == "__main__":
    unittest.main()
