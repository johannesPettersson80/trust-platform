"""Closed variant contract for the Phase 8 restart/time-base review."""

from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any


RESTART_GAP_ID = "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001"
RESTART_SOURCE_REF_RE = re.compile(r"^SPEC_[A-Z0-9_]+$")
OPEN_RESTART_FIELDS = {"outcome", "spec_gap_ref", "rationale"}
RESOLVED_RESTART_FIELDS = {
    "outcome",
    "source_ref",
    "source_path",
    "superseded_gap_id",
    "rationale",
}
RESTART_VARIANT_REFS = (
    "#/$defs/restart_existing_open_gap_v1",
    "#/$defs/restart_resolved_source_v1",
)


def validate_restart_review_shape(value: Any, *, label: str) -> list[str]:
    """Validate the discriminated review shape without consulting live metadata."""

    if not isinstance(value, Mapping):
        return [f"{label} review must be an object"]
    outcome = value.get("outcome")
    if outcome == "existing_open_gap":
        failures = _exact_fields(value, OPEN_RESTART_FIELDS, label)
        if value.get("spec_gap_ref") != RESTART_GAP_ID:
            failures.append(f"{label} spec_gap_ref must equal {RESTART_GAP_ID!r}")
    elif outcome == "resolved_source":
        failures = _exact_fields(value, RESOLVED_RESTART_FIELDS, label)
        source_ref = value.get("source_ref")
        if not isinstance(source_ref, str) or not RESTART_SOURCE_REF_RE.fullmatch(source_ref):
            failures.append(f"{label} source_ref must identify a spec source")
        if not _text(value.get("source_path")):
            failures.append(f"{label} source_path must be non-empty")
        if value.get("superseded_gap_id") != RESTART_GAP_ID:
            failures.append(f"{label} superseded_gap_id must equal {RESTART_GAP_ID!r}")
    else:
        failures = [
            f"{label} outcome must be 'existing_open_gap' or 'resolved_source'"
        ]
    if not _text(value.get("rationale")):
        failures.append(f"{label} rationale must be non-empty")
    return sorted(set(failures))


def validate_restart_union_schema(
    root_schema: Mapping[str, Any],
    union_schema: Mapping[str, Any],
    definitions: Mapping[str, Any],
    *,
    label: str,
) -> list[str]:
    """Drift-pin the exact schema-v1 branches used by taxonomy and reports."""

    failures: list[str] = []
    branches = union_schema.get("oneOf")
    expected_branches = [{"$ref": reference} for reference in RESTART_VARIANT_REFS]
    if branches != expected_branches or set(union_schema) != {"oneOf"}:
        failures.append(f"{label} union drifts")

    open_schema = _definition(definitions, "restart_existing_open_gap_v1")
    resolved_schema = _definition(definitions, "restart_resolved_source_v1")
    _closed_schema(open_schema, OPEN_RESTART_FIELDS, f"{label} existing_open_gap schema", failures)
    _closed_schema(
        resolved_schema,
        RESOLVED_RESTART_FIELDS,
        f"{label} resolved_source schema",
        failures,
    )
    open_properties = _properties(open_schema)
    resolved_properties = _properties(resolved_schema)
    expected_open_properties = {
        "outcome": {"const": "existing_open_gap"},
        "spec_gap_ref": {"const": RESTART_GAP_ID},
        "rationale": {"type": "string", "minLength": 1},
    }
    for field, expected in expected_open_properties.items():
        if open_properties.get(field) != expected:
            if field in {"outcome", "spec_gap_ref"}:
                failures.append(f"{label} existing restart const for {field} drifts")
            else:
                failures.append(f"{label} existing restart {field} schema drifts")
    expected_resolved_properties = {
        "outcome": {"const": "resolved_source"},
        "source_ref": {
            "type": "string",
            "pattern": RESTART_SOURCE_REF_RE.pattern,
        },
        "source_path": {"type": "string", "minLength": 1},
        "superseded_gap_id": {"const": RESTART_GAP_ID},
        "rationale": {"type": "string", "minLength": 1},
    }
    for field, expected in expected_resolved_properties.items():
        if resolved_properties.get(field) != expected:
            if field in {"outcome", "superseded_gap_id"}:
                failures.append(f"{label} resolved restart const for {field} drifts")
            else:
                failures.append(f"{label} resolved restart {field} schema drifts")
    if resolved_properties.get("source_ref") != expected_resolved_properties["source_ref"]:
        failures.append(f"{label} resolved restart source_ref pattern drifts")
    return sorted(set(failures))


def restart_reference_text(review: Mapping[str, Any]) -> str:
    """Render the valid variant's source binding without inferring semantics."""

    if review.get("outcome") == "resolved_source":
        return (
            f"`{review.get('source_ref')}` (`{review.get('source_path')}`), "
            f"superseding `{review.get('superseded_gap_id')}`"
        )
    return f"`{review.get('spec_gap_ref')}`"


def _exact_fields(
    value: Mapping[str, Any], fields: set[str], label: str
) -> list[str]:
    return [] if set(value) == fields else [f"{label} review fields drift from contract"]


def _closed_schema(
    schema: Mapping[str, Any], fields: set[str], label: str, failures: list[str]
) -> None:
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append(f"{label} must be a closed object")
    if set(schema.get("required", [])) != fields:
        failures.append(f"{label} required fields drift")
    properties = schema.get("properties")
    if not isinstance(properties, Mapping) or set(properties) != fields:
        failures.append(f"{label} properties drift")


def _properties(schema: Mapping[str, Any]) -> Mapping[str, Any]:
    properties = schema.get("properties")
    return properties if isinstance(properties, Mapping) else {}


def _definition(definitions: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    value = definitions.get(name)
    return value if isinstance(value, Mapping) else {}


def _text(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())
