"""Focused tests for protocol assertion selection."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.mapping_assertions import RUST_SOURCE_KINDS
from scripts.verification.protocol_behavior_assertions import (
    build_assertion_facts,
)
from scripts.verification.protocol_behavior_campaign import (
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
)
from scripts.verification.protocol_behavior_proof import (
    build_assignment_plan,
)


ROOT = Path(__file__).resolve().parents[2]


class ProtocolBehaviorAssertionsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_assignment_plan(
            root=ROOT,
            gap_report=ROOT / DEFAULT_GAP_REPORT,
            scan_report=ROOT / DEFAULT_SCAN_REPORT,
        )

    def test_assertion_selection_equals_the_exact_assignment(self) -> None:
        facts = build_assertion_facts(self.plan["assignments"])
        self.assertEqual(32, len(facts))
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
