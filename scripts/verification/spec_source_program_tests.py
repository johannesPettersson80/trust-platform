"""Regression locks for the pre-scanner Phase 1A source rules."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.metadata_validator.core import Validator
from scripts.verification.metadata_validator.constants import AREAS
from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.spec_source_live import build_live_spec_source_state


class SpecSourceProgramTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = Validator()
        self.validator.load_records()
        self.assertEqual(self.validator.failures, [])

    def test_bytecode_required_sources_are_explicit_sources_or_open_gaps(self) -> None:
        bytecode_rows = [
            row
            for row in self.validator.required_specs.values()
            if row.get("area") == "bytecode_vm"
        ]

        self.assertTrue(bytecode_rows)
        for row in bytecode_rows:
            with self.subTest(required_spec=row["id"]):
                source_ref = row.get("source_ref")
                gap_ref = row.get("spec_gap_ref")
                self.assertNotEqual(bool(source_ref), bool(gap_ref))
                if source_ref:
                    source = self.validator.spec_sources[source_ref]
                    self.assertEqual(source["source_status"], "active")
                else:
                    gap = self.validator.spec_gaps[gap_ref]
                    self.assertEqual(gap["status"], "spec_gap")
                    self.assertNotEqual(gap["resolution_status"], "closed")

    def test_every_canonical_area_has_an_active_test_mapping_specification(self) -> None:
        rows_by_area = {
            area: [
                row
                for row in self.validator.required_specs.values()
                if row.get("area") == area and row.get("blocks") == "test_mapping"
            ]
            for area in AREAS
        }

        self.assertEqual(
            {area for area, rows in rows_by_area.items() if not rows},
            set(),
        )
        for area, rows in rows_by_area.items():
            for row in rows:
                with self.subTest(area=area, required_spec=row["id"]):
                    source_ref = row.get("source_ref")
                    self.assertIsInstance(source_ref, str)
                    source = self.validator.spec_sources[source_ref]
                    self.assertEqual(source["source_status"], "active")
                    self.assertIs(source["oracle_eligible"], True)

    def test_mapped_catalog_row_without_source_or_gap_fails_full_validation(self) -> None:
        validator = copy.deepcopy(self.validator)
        row = validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"]
        row.pop("oracle_ref", None)
        row.pop("spec_gap_ref", None)

        validator.validate()

        messages = [failure.message for failure in validator.failures]
        self.assertTrue(
            any("mapped test must name oracle_ref or spec_gap_ref" in item for item in messages),
            messages,
        )

    def test_live_source_and_public_prose_denominators_are_fully_reviewed(self) -> None:
        analysis = build_live_spec_source_state(ROOT).analysis

        for field in (
            "source_classification_complete",
            "semantic_claim_review_complete",
            "conflict_review_complete",
            "checklist_row_staleness_complete",
            "removed_behavior_reference_review_complete",
        ):
            self.assertTrue(analysis["scope"][field], field)
        self.assertEqual(0, analysis["summary"]["unreviewed_documents"])
        self.assertEqual(0, analysis["summary"]["unreviewed_public_blocks"])


if __name__ == "__main__":
    unittest.main()
