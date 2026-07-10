"""Tests for JSON-schema vocabulary drift contracts."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.schema_contracts import validate_schema_enums


ROOT = Path(__file__).resolve().parents[3]


class SchemaContractsTests(unittest.TestCase):
    def test_committed_catalog_schema_matches_validator_vocabularies(self) -> None:
        schema = load_catalog_schema()

        self.assertEqual(validate_schema_enums("catalog.schema.json", schema), [])

    def test_subject_kind_schema_drift_is_rejected(self) -> None:
        schema = load_catalog_schema()
        schema["properties"]["subject_kind"]["enum"].remove("generated_test")

        failures = validate_schema_enums("catalog.schema.json", schema)

        self.assertIn("schema enum for subject_kind drifts from validator vocabulary", failures)

    def test_discovery_source_kind_schema_drift_is_rejected(self) -> None:
        schema = load_catalog_schema()
        schema["properties"]["discovery_source_kind"]["enum"].remove("vscode_test")

        failures = validate_schema_enums("catalog.schema.json", schema)

        self.assertIn("schema enum for discovery_source_kind drifts from validator vocabulary", failures)

    def test_existing_test_class_schema_drift_remains_rejected(self) -> None:
        schema = copy.deepcopy(load_catalog_schema())
        schema["properties"]["test_class"]["enum"].remove("mutation")

        failures = validate_schema_enums("catalog.schema.json", schema)

        self.assertIn("schema enum for test_class drifts from validator vocabulary", failures)


def load_catalog_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/catalog.schema.json").read_text())


if __name__ == "__main__":
    unittest.main()
