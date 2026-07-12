"""JSON-schema vocabulary consistency checks."""

from __future__ import annotations

from typing import Any

from ..case_contract_fields import (
    CASE_ARTIFACT_CASE_FIELDS,
    CASE_ARTIFACT_HELPER_VERSION,
    CASE_ARTIFACT_ROOT_FIELDS,
    CASE_ARTIFACT_SNAPSHOT_FIELDS,
    CASE_FILE_REQUIRED_ROOT_FIELDS,
    CASE_FILE_ROOT_FIELDS,
    GENERATED_BLOCKED_CASE_FIELDS,
    GENERATED_RUNNABLE_CASE_FIELDS,
    HAND_AUTHORED_RUNNABLE_CASE_FIELDS,
    TRACE_STEP_FIELDS,
)

from ..area_routing import (
    AREA_FIELDS,
    AREA_OPTIONAL_FIELDS,
    INTENT_FIELDS,
    MATRIX_ROOT_FIELDS,
    MILESTONE_SUITE_IDS,
    ROUTE_FIELDS,
)
from ..test_catalog_intent import GENERATED_SOURCE_KINDS, SUBJECT_KINDS
from ..execution_contract import PROOF_CONTRACT_VERSION
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
        "proof_contract_version": {PROOF_CONTRACT_VERSION},
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
    elif name == "case-file.schema.json":
        failures.extend(_validate_case_file_schema_contract(schema))
    elif name == "case-artifact.schema.json":
        failures.extend(_validate_case_artifact_schema_contract(schema))
    return failures


def _validate_case_file_schema_contract(schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    _check_closed_object(
        schema,
        CASE_FILE_REQUIRED_ROOT_FIELDS,
        CASE_FILE_ROOT_FIELDS,
        "case-file schema root",
        failures,
    )
    definitions = schema.get("$defs", {})
    for name, fields in (
        ("generatedBlockedCase", GENERATED_BLOCKED_CASE_FIELDS),
        ("generatedRunnableCase", GENERATED_RUNNABLE_CASE_FIELDS),
        ("handAuthoredRunnableCase", HAND_AUTHORED_RUNNABLE_CASE_FIELDS),
        ("traceStep", TRACE_STEP_FIELDS),
    ):
        _check_closed_object(
            definitions.get(name, {}),
            fields,
            fields,
            f"case-file schema {name}",
            failures,
        )
    if definitions.get("handAuthoredBlockedCase") != {
        "$ref": "#/$defs/generatedBlockedCase"
    }:
        failures.append("case-file schema handAuthoredBlockedCase contract drift")
    for name in ("inputMap", "expectMap", "traceValueMap"):
        definition = definitions.get(name, {})
        if (
            definition.get("type") != "object"
            or definition.get("minProperties") != 1
            or definition.get("additionalProperties") != {}
        ):
            failures.append(
                f"case-file schema {name} must be an explicitly dynamic non-empty map"
            )
    branches = schema.get("oneOf", [])
    if not isinstance(branches, list) or len(branches) != 2:
        failures.append("case-file schema provenance branches drift")
    else:
        generated, hand_authored = branches
        if set(generated.get("required", [])) != {"generator", "generator_digest"}:
            failures.append("case-file schema generated required fields drift")
        if set(hand_authored.get("required", [])) != {
            "case_provenance_kind",
            "trace_definition_digest",
        }:
            failures.append("case-file schema hand-authored required fields drift")
    return failures


def _validate_case_artifact_schema_contract(schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    _check_closed_object(
        schema,
        CASE_ARTIFACT_ROOT_FIELDS,
        CASE_ARTIFACT_ROOT_FIELDS,
        "case-artifact schema root",
        failures,
    )
    properties = schema.get("properties", {})
    if properties.get("helper_version", {}).get("const") != CASE_ARTIFACT_HELPER_VERSION:
        failures.append("case-artifact schema helper_version const drift")
    cases = properties.get("cases", {})
    if cases.get("minItems") != 1 or cases.get("items") != {
        "$ref": "#/$defs/caseResult"
    }:
        failures.append("case-artifact schema cases binding drift")
    definitions = schema.get("$defs", {})
    _check_closed_object(
        definitions.get("caseResult", {}),
        CASE_ARTIFACT_CASE_FIELDS,
        CASE_ARTIFACT_CASE_FIELDS,
        "case-artifact schema caseResult",
        failures,
    )
    _check_closed_object(
        definitions.get("stateSnapshot", {}),
        CASE_ARTIFACT_SNAPSHOT_FIELDS,
        CASE_ARTIFACT_SNAPSHOT_FIELDS,
        "case-artifact schema stateSnapshot",
        failures,
    )
    snapshot_properties = definitions.get("stateSnapshot", {}).get("properties", {})
    if snapshot_properties.get("target") != {}:
        failures.append("case-artifact schema target must remain explicitly dynamic")
    if snapshot_properties.get("siblings", {}).get("additionalProperties") != {}:
        failures.append("case-artifact schema siblings must remain explicitly dynamic")
    return failures


def _check_closed_object(
    schema: Any,
    required: frozenset[str],
    properties: frozenset[str],
    label: str,
    failures: list[str],
) -> None:
    if not isinstance(schema, dict):
        failures.append(f"{label} must be an object schema")
        return
    if set(schema.get("required", [])) != required:
        failures.append(f"{label} required fields drift")
    if set(schema.get("properties", {})) != properties:
        failures.append(f"{label} property fields drift")
    if schema.get("additionalProperties") is not False:
        failures.append(f"{label} must be closed")


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
