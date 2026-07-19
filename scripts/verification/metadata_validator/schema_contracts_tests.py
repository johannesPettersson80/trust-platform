"""Tests for JSON-schema vocabulary drift contracts."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.schema_contracts import validate_schema_enums
from scripts.verification.area_routing import (
    AREA_FIELDS,
    AREA_OPTIONAL_FIELDS,
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

    def test_committed_spec_source_schema_matches_validator_contract(self) -> None:
        schema = load_spec_source_schema()

        self.assertEqual(validate_schema_enums("spec-source.schema.json", schema), [])

    def test_spec_source_schema_vocabulary_version_and_closure_drift_are_rejected(
        self,
    ) -> None:
        mutations: list[tuple[dict, str]] = []
        for field, value in (
            ("area", "verification"),
            ("status", "mapped"),
            ("authority", "reviewed_decision"),
            ("source_status", "active"),
            ("visibility", "internal"),
            ("locator_kind", "tracked_file"),
        ):
            schema = load_spec_source_schema()
            schema["properties"][field]["enum"].remove(value)
            mutations.append(
                (schema, f"schema enum for {field} drifts from validator vocabulary")
            )

        wrong_version = load_spec_source_schema()
        wrong_version["properties"]["schema_version"]["const"] = 1
        mutations.append((wrong_version, "spec-source schema_version const drift"))

        missing_required = load_spec_source_schema()
        missing_required["required"].remove("conflicts_with")
        mutations.append((missing_required, "spec-source schema root required fields drift"))

        missing_property = load_spec_source_schema()
        missing_property["properties"].pop("acceptance_evidence")
        mutations.append((missing_property, "spec-source schema root property fields drift"))

        missing_visible_steps = load_spec_source_schema()
        missing_visible_steps["properties"].pop("visible_steps")
        mutations.append(
            (missing_visible_steps, "spec-source schema root property fields drift")
        )

        open_root = load_spec_source_schema()
        open_root["additionalProperties"] = True
        mutations.append((open_root, "spec-source schema root must be closed"))

        wrong_tracked_branch = load_spec_source_schema()
        wrong_tracked_branch["oneOf"][0]["properties"]["locator_kind"]["const"] = (
            "external_reference"
        )
        mutations.append((wrong_tracked_branch, "spec-source tracked_file branch drift"))

        missing_external_field = load_spec_source_schema()
        missing_external_field["oneOf"][1]["required"].remove("absence_blocks_proof")
        mutations.append(
            (missing_external_field, "spec-source external_reference branch drift")
        )

        for schema, expected in mutations:
            with self.subTest(expected=expected):
                self.assertIn(
                    expected,
                    validate_schema_enums("spec-source.schema.json", schema),
                )

    def test_case_file_provenance_schema_drift_is_rejected(self) -> None:
        schema = load_case_file_schema()
        self.assertEqual(validate_schema_enums("case-file.schema.json", schema), [])
        schema["properties"]["case_provenance_kind"]["enum"].append(
            "unreviewed_generator_v9"
        )

        failures = validate_schema_enums("case-file.schema.json", schema)

        self.assertIn(
            "schema enum for case_provenance_kind drifts from validator vocabulary",
            failures,
        )

    def test_case_file_and_artifact_schemas_pin_closed_field_contracts(self) -> None:
        case_schema = load_case_file_schema()
        artifact_schema = load_case_artifact_schema()

        self.assertEqual(validate_schema_enums("case-file.schema.json", case_schema), [])
        self.assertEqual(
            validate_schema_enums("case-artifact.schema.json", artifact_schema), []
        )

        mutations = []
        open_root = copy.deepcopy(case_schema)
        open_root["additionalProperties"] = True
        mutations.append(("case-file.schema.json", open_root))
        open_case = copy.deepcopy(case_schema)
        open_case["$defs"]["generatedBlockedCase"]["additionalProperties"] = True
        mutations.append(("case-file.schema.json", open_case))
        implicit_input = copy.deepcopy(case_schema)
        implicit_input["$defs"]["inputMap"].pop("additionalProperties")
        mutations.append(("case-file.schema.json", implicit_input))
        open_artifact_case = copy.deepcopy(artifact_schema)
        open_artifact_case["$defs"]["caseResult"]["additionalProperties"] = True
        mutations.append(("case-artifact.schema.json", open_artifact_case))
        missing_artifact_field = copy.deepcopy(artifact_schema)
        missing_artifact_field["required"].remove("helper_version")
        mutations.append(("case-artifact.schema.json", missing_artifact_field))

        for name, schema in mutations:
            with self.subTest(name=name):
                self.assertTrue(validate_schema_enums(name, schema))

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
        self.assertEqual(
            set(schema["$defs"]["area"]["properties"]),
            AREA_FIELDS | AREA_OPTIONAL_FIELDS,
        )
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
        missing_optional_area_field = load_matrix_schema()
        missing_optional_area_field["$defs"]["area"]["properties"].pop(
            "decision_ref"
        )
        mutations.append(missing_optional_area_field)

        for schema in mutations:
            with self.subTest(schema=schema):
                self.assertTrue(validate_schema_enums("matrix.schema.json", schema))


def load_catalog_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/catalog.schema.json").read_text())


def load_suite_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/suite.schema.json").read_text())


def load_case_file_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/case-file.schema.json").read_text())


def load_case_artifact_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/case-artifact.schema.json").read_text())


def load_spec_source_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/spec-source.schema.json").read_text())


def load_matrix_schema() -> dict:
    return json.loads((ROOT / "verification/schemas/matrix.schema.json").read_text())


if __name__ == "__main__":
    unittest.main()
