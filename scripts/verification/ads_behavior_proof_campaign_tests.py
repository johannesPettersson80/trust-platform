"""Focused tests for the unified Phase 18 ADS execution campaign."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.ads_behavior_proof_campaign import (
    build_campaign_plan,
    normalize_group_results,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-remaining-267/frozen-gaps.json"
)


class AdsBehaviorProofCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_campaign_plan(root=ROOT, gap_report=GAP_REPORT)

    def test_campaign_is_one_closed_ads_population(self) -> None:
        self.assertEqual(71, self.plan["behavior_count"])
        self.assertEqual(145, self.plan["test_count"])
        self.assertEqual(24, self.plan["path_count"])
        self.assertEqual(6, self.plan["group_count"])
        self.assertEqual(
            {
                ("trust-ads-server", "lib", None, ()): 10,
                ("trust-runtime", "lib", None, ()): 101,
                (
                    "trust-runtime",
                    "lib",
                    None,
                    ("--no-default-features",),
                ): 19,
                (
                    "trust-runtime",
                    "lib",
                    None,
                    ("--no-default-features", "--features", "ads-wire"),
                ): 9,
                (
                    "trust-runtime",
                    "test",
                    "ads_cli_command",
                    ("--no-default-features",),
                ): 5,
                ("trust-runtime", "test", "ads_cli_command", ()): 1,
            },
            {
                (
                    group["package"],
                    group["target_kind"],
                    group["target_name"],
                    tuple(group["feature_args"]),
                ): group["test_count"]
                for group in self.plan["groups"]
            },
        )

    def test_every_selected_test_has_one_exact_group_identity(self) -> None:
        test_ids = [
            test_id
            for group in self.plan["groups"]
            for test_id in group["test_ids"]
        ]
        self.assertEqual(self.plan["test_count"], len(test_ids))
        self.assertEqual(self.plan["test_count"], len(set(test_ids)))
        for group in self.plan["groups"]:
            self.assertIn("--no-fail-fast", group["command"])
            self.assertIn("--message-format", group["command"])
            self.assertIn("-E", group["command"])
            self.assertEqual(group["test_count"], len(group["full_names"]))

    def test_normalizer_requires_every_exact_passing_terminal(self) -> None:
        group = {
            "encoded_test_name_prefix": "pkg::target$",
            "tests_by_full_name": {
                "module::first": "TEST_FIRST",
                "module::second": "TEST_SECOND",
            },
        }
        raw = "\n".join(
            json.dumps(row)
            for row in (
                {
                    "type": "test",
                    "event": "started",
                    "name": "pkg::target$module::first",
                },
                {
                    "type": "test",
                    "event": "ok",
                    "name": "pkg::target$module::first",
                    "exec_time": 0.002,
                },
                {
                    "type": "test",
                    "event": "ok",
                    "name": "pkg::target$module::second",
                    "exec_time": 0.003,
                },
            )
        )

        normalized = normalize_group_results(group=group, raw=raw)
        self.assertEqual(
            ["TEST_FIRST", "TEST_SECOND"],
            [row["test_id"] for row in normalized],
        )
        self.assertTrue(all(row["status"] == "passed" for row in normalized))

        with self.assertRaisesRegex(ValueError, "missing terminal"):
            normalize_group_results(
                group=group,
                raw=raw.rsplit("\n", 1)[0],
            )
        with self.assertRaisesRegex(ValueError, "did not pass"):
            normalize_group_results(
                group=group,
                raw=raw.replace('"event": "ok"', '"event": "failed"', 1),
            )


if __name__ == "__main__":
    unittest.main()
