"""JSON-schema vocabulary consistency checks."""

from __future__ import annotations

from typing import Any

from ..area_routing import (
    AREA_FIELDS,
    AREA_OPTIONAL_FIELDS,
    INTENT_FIELDS,
    MATRIX_ROOT_FIELDS,
    MILESTONE_SUITE_IDS,
    ROUTE_FIELDS,
)
from ..test_catalog_intent import GENERATED_SOURCE_KINDS, SUBJECT_KINDS
from .ignored_tests import (
    IGNORED_SOURCE_KINDS,
    IGNORE_CLASSES,
    IGNORE_MECHANISMS,
    IGNORE_STATES,
)
from .constants import (
    AREAS,
    CONTRACT_KINDS,
    EVIDENCE_KINDS,
    GAP_CLASSES,
    PROOF_KINDS,
    PROOF_LEVELS,
    PROOF_SCOPES,
    RISKS,
    STATUSES,
    TEST_CLASSES,
)
from .case_trace_contract import CASE_PROVENANCE_KINDS


SCHEMA_ENUM_EXPECTATIONS = {
    "case-artifact.schema.json": {
        "case_provenance_kind": CASE_PROVENANCE_KINDS,
    },
    "case-file.schema.json": {
        "case_provenance_kind": CASE_PROVENANCE_KINDS,
    },
    "catalog.schema.json": {
        "subject_kind": SUBJECT_KINDS,
        "discovery_source_kind": GENERATED_SOURCE_KINDS,
        "test_class": TEST_CLASSES,
    },
    "evidence.schema.json": {
        "kind": EVIDENCE_KINDS,
        "proof_kind": PROOF_KINDS,
        "proof_scope": PROOF_SCOPES,
    },
    "ignored-test.schema.json": {
        "area": AREAS,
        "discovery_source_kind": IGNORED_SOURCE_KINDS,
        "ignore_class": IGNORE_CLASSES,
        "ignore_mechanism": IGNORE_MECHANISMS,
        "ignore_state": IGNORE_STATES,
        "status": STATUSES,
    },
    "invariant.schema.json": {
        "risk": RISKS,
        "contract_kind": CONTRACT_KINDS,
        "proof_level": PROOF_LEVELS,
    },
    "spec-gap.schema.json": {"gap_class": GAP_CLASSES},
}


def validate_schema_enums(name: str, schema: dict[str, Any]) -> list[str]:
    """Return drift failures for schema enums owned by Python vocabularies."""

    failures: list[str] = []
    properties = schema.get("properties", {})
    for field, expected_values in SCHEMA_ENUM_EXPECTATIONS.get(name, {}).items():
        actual = properties.get(field, {}).get("enum")
        if set(actual or []) != expected_values:
            failures.append(f"schema enum for {field} drifts from validator vocabulary")
    if name == "matrix.schema.json":
        failures.extend(_validate_matrix_schema_contract(schema))
    return failures


def _validate_matrix_schema_contract(schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    properties = schema.get("properties", {})
    definitions = schema.get("$defs", {})
    if properties.get("schema_version", {}).get("const") != 2:
        failures.append("matrix schema_version const must be 2")
    if set(schema.get("required", [])) != MATRIX_ROOT_FIELDS:
        failures.append("matrix schema root required fields drift")
    if schema.get("additionalProperties") is not False:
        failures.append("matrix schema root must be closed")
    if set(definitions.get("areaId", {}).get("enum", [])) != AREAS:
        failures.append("matrix schema areaId enum drifts from AREAS")
    if set(definitions.get("suiteId", {}).get("enum", [])) != MILESTONE_SUITE_IDS:
        failures.append("matrix schema suiteId enum drifts from milestone suites")
    for name, required, optional in (
        ("area", AREA_FIELDS, AREA_OPTIONAL_FIELDS),
        ("codeArea", ROUTE_FIELDS, set()),
        ("intentRequirement", INTENT_FIELDS, set()),
    ):
        definition = definitions.get(name, {})
        if set(definition.get("required", [])) != required:
            failures.append(f"matrix schema {name} required fields drift")
        if set(definition.get("properties", {})) != required | optional:
            failures.append(f"matrix schema {name} property fields drift")
        if definition.get("additionalProperties") is not False:
            failures.append(f"matrix schema {name} must be closed")
    return failures
