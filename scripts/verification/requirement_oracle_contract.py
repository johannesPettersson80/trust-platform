"""Fail-closed payload, schema, and Markdown contract for the Phase 6 audit."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from typing import Any

from .metadata_validator.constants import HIGH_RISKS
from .metadata_validator.oracle_refs import ORACLE_AUTHORITIES
from .requirement_oracle_mapping import MAPPING_GROUP_AREAS, board_row_for_area
from .requirement_oracle_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    SCOPE,
    render_markdown,
)
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
TOP_FIELDS = {
    "schema_version",
    "generator",
    "generator_version",
    "report_status",
    "input_digest",
    "command",
    "commit",
    "timestamp",
    "platform",
    "input_paths",
    "output_paths",
    "scope",
    "boundaries",
    "mapping_groups",
    "invariants",
    "missing_oracles",
    "summary",
    "limitations",
}
OUTPUT_FIELDS = {"json", "markdown"}
SCOPE_FIELDS = set(SCOPE)
GROUP_FIELDS = {
    "board_row",
    "area_ids",
    "invariant_count",
    "invariant_ids",
    "eligible_oracle_count",
    "spec_gap_blocked_count",
}
INVARIANT_FIELDS = {
    "invariant_id",
    "area",
    "risk",
    "invariant_status",
    "proof_level",
    "spec_status",
    "mapping_board_row",
    "oracle_kind",
    "oracle_ref",
    "oracle_state",
    "oracle_source_authority",
    "spec_source_refs",
    "eligible_context_source_refs",
    "public_claim_source_refs",
    "spec_gap_refs",
    "tests",
    "gates",
    "evidence_refs",
    "future_enforcement_candidate",
}
SUMMARY_FIELDS = {
    "invariants_total",
    "mapped_phase6_invariants",
    "other_area_invariants",
    "eligible_oracles",
    "missing_oracles",
    "future_enforcement_candidates",
}


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_analysis: Mapping[str, Any] | None = None,
) -> list[str]:
    failures: list[str] = []
    _fields(payload, TOP_FIELDS, "report", failures)
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected!r}")
    if not isinstance(payload.get("commit"), str) or not COMMIT_RE.fullmatch(
        str(payload.get("commit", ""))
    ):
        failures.append("commit must identify a clean full Git SHA")
    if not isinstance(payload.get("input_digest"), str) or not DIGEST_RE.fullmatch(
        str(payload.get("input_digest", ""))
    ):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")

    inputs = payload.get("input_paths")
    if not _string_array(inputs) or inputs != sorted(set(inputs)):
        failures.append("input_paths must be a sorted unique non-empty string array")
    elif any(not is_safe_relative_path(path) for path in inputs):
        failures.append("input_paths must be normalized workspace-relative paths")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping):
        failures.append("output_paths must be an object")
    else:
        _fields(outputs, OUTPUT_FIELDS, "output_paths", failures)
        if any(
            not isinstance(outputs.get(field), str)
            or not is_safe_relative_path(str(outputs.get(field)))
            for field in OUTPUT_FIELDS
        ):
            failures.append("output paths must be normalized and workspace-relative")
    _validate_command(payload, failures)
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries do not match the report-only Phase 6 contract")
    if payload.get("scope") != SCOPE:
        failures.append("scope does not match the conservative Phase 6 audit contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the requirement/oracle audit contract")

    groups = _rows(payload, "mapping_groups", GROUP_FIELDS, failures)
    invariants = _rows(payload, "invariants", INVARIANT_FIELDS, failures)
    missing = _rows(payload, "missing_oracles", INVARIANT_FIELDS, failures)
    _validate_invariants(invariants, failures)
    _validate_groups(groups, invariants, failures)
    expected_missing = [
        row for row in invariants if row.get("oracle_state") == "spec_gap_blocked"
    ]
    if missing != expected_missing:
        failures.append("missing_oracles must exactly match spec-gap-blocked invariant rows")
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
        if dict(summary) != _summary(invariants):
            failures.append("summary does not match invariant rows")

    if expected_analysis is not None:
        actual_analysis = {
            "mapping_groups": groups,
            "invariants": invariants,
            "missing_oracles": missing,
            "summary": dict(summary) if isinstance(summary, Mapping) else summary,
        }
        if actual_analysis != dict(expected_analysis):
            failures.append("report rows do not match current requirement/oracle analysis")
    return sorted(set(failures))


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("requirement/oracle schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS:
        failures.append("requirement/oracle schema root required fields drift")
    properties = schema.get("properties", {})
    if not isinstance(properties, Mapping) or set(properties) != TOP_FIELDS:
        failures.append("requirement/oracle schema root properties drift")
        properties = {}
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"requirement/oracle schema const for {field} drifts")
    if properties.get("commit", {}).get("pattern") != "^[0-9a-f]{40}$":
        failures.append("requirement/oracle schema clean-commit pattern drifts")
    definitions = schema.get("$defs", {})
    if not isinstance(definitions, Mapping):
        failures.append("requirement/oracle schema definitions must be an object")
        definitions = {}
    expected_defs = {
        "output_paths": OUTPUT_FIELDS,
        "scope": SCOPE_FIELDS,
        "boundaries": set(BOUNDARIES),
        "mapping_group": GROUP_FIELDS,
        "invariant": INVARIANT_FIELDS,
        "summary": SUMMARY_FIELDS,
    }
    for name, fields in expected_defs.items():
        definition = definitions.get(name, {})
        if not isinstance(definition, Mapping):
            failures.append(f"requirement/oracle schema {name} must be an object")
            definition = {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"requirement/oracle schema {name} must be a closed object")
        if set(definition.get("required", [])) != fields:
            failures.append(f"requirement/oracle schema {name} required fields drift")
        if set(definition.get("properties", {})) != fields:
            failures.append(f"requirement/oracle schema {name} properties drift")
    scope_properties = _schema_properties(definitions, "scope")
    expected_mapping_rows = list(MAPPING_GROUP_AREAS)
    mapping_rows = _schema_property(scope_properties, "mapping_rows")
    mapping_items = mapping_rows.get("items", {})
    mapping_enum = mapping_items.get("enum") if isinstance(mapping_items, Mapping) else None
    if mapping_enum != expected_mapping_rows:
        failures.append("requirement/oracle schema scope mapping-row enum drifts")
    boundary_properties = _schema_properties(definitions, "boundaries")
    for field, expected in BOUNDARIES.items():
        if _schema_property(boundary_properties, field).get("const") is not expected:
            failures.append(f"requirement/oracle schema boundary const for {field} drifts")
    group_properties = _schema_properties(definitions, "mapping_group")
    if _schema_property(group_properties, "board_row").get("enum") != expected_mapping_rows:
        failures.append("requirement/oracle schema mapping-group board-row enum drifts")
    invariant_properties = _schema_properties(definitions, "invariant")
    if _schema_property(invariant_properties, "oracle_state").get("enum") != [
        "eligible_oracle",
        "spec_gap_blocked",
    ]:
        failures.append("requirement/oracle schema oracle-state enum drifts")
    if _schema_property(invariant_properties, "mapping_board_row").get("enum") != [
        *expected_mapping_rows,
        None,
    ]:
        failures.append("requirement/oracle schema invariant mapping-row enum drifts")
    if _schema_property(invariant_properties, "oracle_source_authority").get("enum") != [
        *sorted(ORACLE_AUTHORITIES),
        None,
    ]:
        failures.append("requirement/oracle schema oracle-authority enum drifts")
    _closed_objects(schema, "$", failures)
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    failures: list[str] = []
    canonical = (json.dumps(dict(payload), indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("requirement/oracle JSON is not canonical")
    try:
        expected = render_markdown(
            payload,
            json_digest=hashlib.sha256(json_bytes).hexdigest(),
        )
    except (KeyError, TypeError, ValueError) as exc:
        failures.append(f"requirement/oracle Markdown cannot be reconstructed: {exc}")
    else:
        if markdown != expected:
            failures.append("requirement/oracle Markdown does not exactly match JSON payload")
    return sorted(set(failures))


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or not isinstance(payload.get("timestamp"), str):
        return
    expected = [
        "python3",
        "scripts/report_requirement_oracle_audit.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        payload["timestamp"],
    ]
    if payload.get("command") != expected:
        failures.append("command does not match the canonical report invocation")


def _validate_invariants(rows: list[dict[str, Any]], failures: list[str]) -> None:
    ids = [row.get("invariant_id") for row in rows]
    if ids != sorted(ids) or len(ids) != len(set(ids)):
        failures.append("invariant rows must use unique canonical ID order")
    for index, row in enumerate(rows):
        label = f"invariants[{index}]"
        for field in (
            "spec_source_refs",
            "eligible_context_source_refs",
            "public_claim_source_refs",
            "spec_gap_refs",
            "tests",
            "gates",
            "evidence_refs",
        ):
            if not _string_array_allow_empty(row.get(field)):
                failures.append(f"{label}.{field} must be a string array")
        state = row.get("oracle_state")
        if state not in {"eligible_oracle", "spec_gap_blocked"}:
            failures.append(f"{label}.oracle_state is invalid")
        if state == "eligible_oracle":
            if not isinstance(row.get("oracle_source_authority"), str):
                failures.append(f"{label} eligible oracle requires source authority")
            if row.get("future_enforcement_candidate") is not False:
                failures.append(f"{label} eligible oracle cannot be an enforcement candidate")
        elif row.get("oracle_source_authority") is not None:
            failures.append(f"{label} blocked oracle forbids source authority")
        expected_candidate = state == "spec_gap_blocked" and row.get("risk") in HIGH_RISKS
        if row.get("future_enforcement_candidate") is not expected_candidate:
            failures.append(f"{label} future-enforcement classification drifts")
        board_row = row.get("mapping_board_row")
        if board_row is not None and board_row not in MAPPING_GROUP_AREAS:
            failures.append(f"{label} mapping_board_row is invalid")
        expected_board_row = board_row_for_area(str(row.get("area", "")))
        if board_row != expected_board_row:
            failures.append(
                f"{label} mapping_board_row must be {expected_board_row!r} for its area"
            )


def _validate_groups(
    groups: list[dict[str, Any]],
    invariants: list[dict[str, Any]],
    failures: list[str],
) -> None:
    if [row.get("board_row") for row in groups] != list(MAPPING_GROUP_AREAS):
        failures.append("mapping groups must use canonical Phase 6 board-row order")
    for group in groups:
        board_row = group.get("board_row")
        if board_row not in MAPPING_GROUP_AREAS:
            continue
        area_ids = list(MAPPING_GROUP_AREAS[board_row])
        selected = [row for row in invariants if row.get("area") in area_ids]
        expected = {
            "board_row": board_row,
            "area_ids": area_ids,
            "invariant_count": len(selected),
            "invariant_ids": [row.get("invariant_id") for row in selected],
            "eligible_oracle_count": sum(
                row.get("oracle_state") == "eligible_oracle" for row in selected
            ),
            "spec_gap_blocked_count": sum(
                row.get("oracle_state") == "spec_gap_blocked" for row in selected
            ),
        }
        if group != expected:
            failures.append(f"mapping group {board_row} does not match invariant rows")


def _summary(rows: list[dict[str, Any]]) -> dict[str, int]:
    mapped = sum(row.get("mapping_board_row") is not None for row in rows)
    return {
        "invariants_total": len(rows),
        "mapped_phase6_invariants": mapped,
        "other_area_invariants": len(rows) - mapped,
        "eligible_oracles": sum(row.get("oracle_state") == "eligible_oracle" for row in rows),
        "missing_oracles": sum(row.get("oracle_state") == "spec_gap_blocked" for row in rows),
        "future_enforcement_candidates": sum(
            row.get("future_enforcement_candidate") is True for row in rows
        ),
    }


def _rows(
    payload: Mapping[str, Any],
    field: str,
    fields: set[str],
    failures: list[str],
) -> list[dict[str, Any]]:
    value = payload.get(field)
    if not isinstance(value, list):
        failures.append(f"{field} must be an array")
        return []
    result: list[dict[str, Any]] = []
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"{field}[{index}] must be an object")
            continue
        _fields(row, fields, f"{field}[{index}]", failures)
        result.append(dict(row))
    return result


def _fields(
    value: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    actual = set(value)
    if actual != expected:
        failures.append(
            f"{label} fields drift: missing {sorted(expected - actual)}, unknown {sorted(actual - expected)}"
        )


def _closed_objects(value: Any, path: str, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        return
    if value.get("type") == "object" and value.get("additionalProperties") is not False:
        failures.append(f"schema object {path} must set additionalProperties = false")
    for key, item in value.items():
        if isinstance(item, Mapping):
            _closed_objects(item, f"{path}.{key}", failures)


def _schema_properties(
    definitions: Mapping[str, Any],
    name: str,
) -> Mapping[str, Any]:
    definition = definitions.get(name)
    if not isinstance(definition, Mapping):
        return {}
    properties = definition.get("properties")
    return properties if isinstance(properties, Mapping) else {}


def _schema_property(properties: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    value = properties.get(name)
    return value if isinstance(value, Mapping) else {}


def _string_array(value: Any) -> bool:
    return bool(value) and _string_array_allow_empty(value) and value == sorted(set(value))


def _string_array_allow_empty(value: Any) -> bool:
    return isinstance(value, list) and all(isinstance(item, str) and item for item in value)


def _timestamp(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None
