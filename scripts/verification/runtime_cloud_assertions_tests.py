"""Focused tests for runtime-cloud assertion selection."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.runtime_cloud_assertions import (
    build_assertion_facts,
)
from scripts.verification.mapping_assertions import RUST_SOURCE_KINDS
from scripts.verification.runtime_cloud_behavior_proof import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]
SCAN_REPORT = ROOT / (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-28/"
    "p18-runtime-other-exact-behavior-106/live-scan.json"
)


class RuntimeCloudAssertionsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(
            root=ROOT,
            gap_report=ROOT / "missing-generated-gap-report.json",
            scan_report=SCAN_REPORT,
        )

    def test_assertion_selection_equals_the_exact_assignment(self) -> None:
        facts = build_assertion_facts(self.plan["assignments"])
        self.assertEqual(52, len(facts))
        self.assertEqual(
            {
                row["discovery_id"]
                for row in self.plan["assignments"]
            },
            {row["discovery_id"] for row in facts},
        )
        self.assertTrue(
            all(
                row["discovery_source_kind"] in RUST_SOURCE_KINDS
                for row in facts
            )
        )


if __name__ == "__main__":
    unittest.main()
