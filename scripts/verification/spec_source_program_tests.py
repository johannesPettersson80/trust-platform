"""Regression locks for the pre-scanner Phase 1A source rules."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.metadata_validator.core import Validator


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


if __name__ == "__main__":
    unittest.main()
