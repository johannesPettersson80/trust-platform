"""Tests for JSON-schema vocabulary drift contracts."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.schema_contracts import validate_schema_enums
from scripts.verification.area_routing import (
    AREA_FIELDS,
    INTENT_FIELDS,
    MATRIX_ROOT_FIELDS,
    MILESTONE_SUITE_IDS,
    ROUTE_FIELDS,
)
from scripts.verification.metadata_validator.constants import AREAS
from scripts.verification.metadata_validator.suites import (
    DURATION_CLASSES,
    ENVIRONMENTS,
    SUITE_FIELDS,
)


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

    def test_suite_v2_schema_matches_validator_contract(self) -> None:
        schema = load_suite_schema()

        self.assertEqual(schema["properties"]["schema_version"]["const"], 2)
        self.assertEqual(set(schema["required"]), SUITE_FIELDS)
        self.assertEqual(set(schema["properties"]["duration_class"]["enum"]), DURATION_CLASSES)
        self.assertEqual(set(schema["properties"]["environment"]["enum"]), ENVIRONMENTS)
        self.assertFalse(schema["additionalProperties"])

    def test_matrix_v2_schema_matches_routing_contract(self) -> None:
        schema = load_matrix_schema()

        self.assertEqual(validate_schema_enums("matrix.schema.json", schema), [])
        self.assertEqual(set(schema["required"]), MATRIX_ROOT_FIELDS)
        self.assertEqual(set(schema["$defs"]["area"]["required"]), AREA_FIELDS)
        self.assertEqual(set(schema["$defs"]["codeArea"]["required"]), ROUTE_FIELDS)
        self.assertEqual(set(schema["$defs"]["intentRequirement"]["required"]), INTENT_FIELDS)
        self.assertEqual(set(schema["$defs"]["areaId"]["enum"]), AREAS)
        self.assertEqual(
            set(schema["$defs"]["suiteId"]["enum"]), MILESTONE_SUITE_IDS
        )

    def test_matrix_schema_const_enums_and_closure_are_drift_pinned(self) -> None:
        mutations = []
        wrong_version = load_matrix_schema()
        wrong_version["properties"]["schema_version"]["const"] = 1
        mutations.append(wrong_version)
        wrong_area = load_matrix_schema()
        wrong_area["$defs"]["areaId"]["enum"].remove("verification")
        mutations.append(wrong_area)
        wrong_suite = load_matrix_schema()
        wrong_suite["$defs"]["suiteId"]["enum"].remove("hardware_lab")
        mutations.append(wrong_suite)
        open_route = load_matrix_schema()
        open_route["$defs"]["codeArea"]["additionalProperties"] = True
        mutations.append(open_route)
        missing_intent_field = load_matrix_schema()
        missing_intent_field["$defs"]["intentRequirement"]["required"].remove(
            "lock_required"
        )
        mutations.append(missing_intent_field)

        for schema in mutations:
            with self.subTest(schema=schema):
                self.assertTrue(validate_schema_enums("matrix.schema.json", schema))


def load_catalog_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/catalog.schema.json").read_text())


def load_suite_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/suite.schema.json").read_text())


def load_matrix_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/matrix.schema.json").read_text())


if __name__ == "__main__":
    unittest.main()
