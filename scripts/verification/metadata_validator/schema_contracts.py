"""JSON-schema vocabulary consistency checks."""

from __future__ import annotations

from typing import Any

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
    RISKS,
    STATUSES,
    TEST_CLASSES,
)


SCHEMA_ENUM_EXPECTATIONS = {
    "catalog.schema.json": {
        "subject_kind": SUBJECT_KINDS,
        "discovery_source_kind": GENERATED_SOURCE_KINDS,
        "test_class": TEST_CLASSES,
    },
    "evidence.schema.json": {
        "kind": EVIDENCE_KINDS,
        "proof_kind": PROOF_KINDS,
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
    return failures
