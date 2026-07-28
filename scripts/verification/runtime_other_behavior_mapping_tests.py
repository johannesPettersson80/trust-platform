"""Focused contract tests for the runtime-other mapping transaction."""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

from scripts.verification.runtime_other_behavior_mapping import (
    compile_runtime_other_mapping,
)


ROOT = Path(__file__).resolve().parents[2]


class RuntimeOtherBehaviorMappingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.compilation = compile_runtime_other_mapping(root=ROOT)
        cls.summary = cls.compilation.summary
        cls.documents = {
            path: tomllib.loads(content.decode())
            for path, content in cls.compilation.replacements.items()
            if path.endswith(".toml")
        }

    def test_compiles_the_complete_frozen_runtime_other_transaction(self) -> None:
        self.assertEqual(106, self.summary["behavior_demand_count"])
        self.assertEqual(196, self.summary["campaign_test_count"])
        self.assertEqual(34, self.summary["new_catalog_test_count"])
        self.assertEqual(163, self.summary["existing_catalog_test_count"])
        self.assertEqual(12, self.summary["refreshed_catalog_test_count"])
        self.assertEqual(7, self.summary["source_closure_refresh_count"])
        self.assertEqual(
            15,
            self.summary["updated_mapping_provenance_count"],
        )
        self.assertEqual(5, self.summary["denominator_addition_count"])
        self.assertEqual(90, self.summary["confirmed_missing_test_count"])

    def test_replaces_every_mapping_control_plane_document(self) -> None:
        required = {
            "verification/test-catalog.toml",
            "verification/test-catalog-denominator.toml",
            "verification/test-catalog-proposals.toml",
            "verification/test-catalog-mapping-decisions.toml",
        }
        self.assertTrue(
            required <= set(self.compilation.replacements),
            required - set(self.compilation.replacements),
        )

    def test_maps_the_live_breakpoint_identity_to_its_exact_behavior(self) -> None:
        catalog = self.documents["verification/test-catalog.toml"]
        selected = [
            row
            for row in catalog["tests"]
            if row.get("discovery_id") == "DISC_1D27FB7DCC4AAEE15C39"
        ]
        self.assertEqual(1, len(selected))
        self.assertEqual(
            ["DEBUG_CONTROL_EXECUTION_001#behavior:2"],
            selected[0]["mapping_behavior_refs"],
        )

    def test_leaves_no_pending_proposals_and_closes_live_denominator(self) -> None:
        proposals = self.documents[
            "verification/test-catalog-proposals.toml"
        ]
        denominator = self.documents[
            "verification/test-catalog-denominator.toml"
        ]
        self.assertEqual([], proposals["proposals"])
        self.assertEqual(4399, len(denominator["reviews"]))
        self.assertEqual(
            4399,
            len(
                {
                    row["discovery_id"]
                    for row in denominator["reviews"]
                }
            ),
        )


if __name__ == "__main__":
    unittest.main()
