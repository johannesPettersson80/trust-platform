"""At-rest validation for the Phase 12 workflow and UI audit."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .phase12_audit import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    build_summary,
    canonical_json,
    render_markdown,
)
from .phase12_audit_live import build_live_phase12_state, validate_source_revision
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


TOP_FIELDS = {
    "schema_version",
    "generator",
    "generator_version",
    "report_status",
    "commit",
    "timestamp",
    "platform",
    "input_paths",
    "input_digest",
    "output_paths",
    "command",
    "boundaries",
    "workflow_rows",
    "journey_rows",
    "summary",
    "limitations",
}
WORKFLOW_FIELDS = {
    "discovery_id",
    "path",
    "heading_path",
    "disposition",
    "spec_source_id",
    "linked_journey_ids",
    "invariant_ids",
    "acceptance_status",
    "missing_spec_source",
    "missing_invariant_link",
    "missing_acceptance_evidence",
}
JOURNEY_FIELDS = {
    "id",
    "title",
    "surface",
    "status",
    "journey_source",
    "workflow_candidate_ids",
    "invariant_ids",
    "supporting_test_ids",
    "source_transformation",
    "fresh_visual_evidence",
    "backend_support_without_fresh_visual",
}
SUMMARY_FIELDS = {
    "workflow_candidates",
    "workflow_specs",
    "reviewed_nonworkflows",
    "workflow_missing_spec_source",
    "workflow_missing_invariant_link",
    "workflow_missing_acceptance_evidence",
    "journeys",
    "journeys_with_invariants",
    "journeys_with_supporting_tests",
    "journeys_with_fresh_visual_evidence",
    "backend_support_without_fresh_visual",
    "journey_status_counts",
}
OUTPUT_FIELDS = {"json", "markdown"}
COUNT_FIELDS = {"name", "count"}
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


def validate_payload(payload: Mapping[str, Any], *, expected_state=None) -> list[str]:
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
    if not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must identify a clean full Git SHA")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload["platform"]:
        failures.append("platform must be non-empty")
    inputs = payload.get("input_paths")
    if not _string_array(inputs) or inputs != sorted(set(inputs)):
        failures.append("input_paths must be a sorted unique non-empty string array")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping):
        failures.append("output_paths must be an object")
    else:
        _fields(outputs, OUTPUT_FIELDS, "output_paths", failures)
        for field in OUTPUT_FIELDS:
            if not is_safe_relative_path(outputs.get(field)):
                failures.append(f"output_paths.{field} must be workspace-relative")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries drift from the Phase 12 honesty contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations drift from the Phase 12 honesty contract")
    _validate_command(payload, failures)

    workflows = _rows(payload.get("workflow_rows"), WORKFLOW_FIELDS, "workflow_rows", failures)
    journeys = _rows(payload.get("journey_rows"), JOURNEY_FIELDS, "journey_rows", failures)
    if len(workflows) != 47:
        failures.append("workflow_rows must contain exactly 47 reviewed candidates")
    if len(journeys) != 30:
        failures.append("journey_rows must contain exactly 30 reviewed journeys")
    if len({row.get("discovery_id") for row in workflows}) != len(workflows):
        failures.append("workflow_rows contain duplicate discovery identities")
    if len({row.get("id") for row in journeys}) != len(journeys):
        failures.append("journey_rows contain duplicate IDs")
    for index, row in enumerate(workflows):
        if row.get("disposition") == "workflow_spec" and not row.get("spec_source_id"):
            failures.append(f"workflow_rows[{index}] workflow_spec lacks spec_source_id")
        if row.get("disposition") == "reviewed_nonworkflow" and row.get("spec_source_id") is not None:
            failures.append(f"workflow_rows[{index}] reviewed_nonworkflow claims a spec source")
    for index, row in enumerate(journeys):
        fresh = row.get("status") in {"provisional", "ux_accepted"}
        if row.get("fresh_visual_evidence") is not fresh:
            failures.append(f"journey_rows[{index}] fresh visual flag is inconsistent")
        backend_only = bool(row.get("supporting_test_ids")) and not fresh
        if row.get("backend_support_without_fresh_visual") is not backend_only:
            failures.append(f"journey_rows[{index}] backend-only flag is inconsistent")
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
        for index, count in enumerate(summary.get("journey_status_counts", [])):
            if isinstance(count, Mapping):
                _fields(count, COUNT_FIELDS, f"journey_status_counts[{index}]", failures)
        if dict(summary) != build_summary(workflows, journeys):
            failures.append("summary does not match report rows")
    if expected_state is not None:
        if workflows != list(expected_state.workflow_rows) or journeys != list(
            expected_state.journey_rows
        ):
            failures.append("report rows do not match current live Phase 12 state")
    return sorted(set(failures))


def validate_schema(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("report schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS or set(schema.get("properties", {})) != TOP_FIELDS:
        failures.append("report schema top-level fields drift from validator")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"report schema const for {field} drifts from validator")
    definitions = schema.get("$defs", {})
    for name, fields in (
        ("output_paths", OUTPUT_FIELDS),
        ("boundaries", set(BOUNDARIES)),
        ("workflow", WORKFLOW_FIELDS),
        ("journey", JOURNEY_FIELDS),
        ("summary", SUMMARY_FIELDS),
        ("count", COUNT_FIELDS),
    ):
        definition = definitions.get(name, {})
        if set(definition.get("required", [])) != fields or set(
            definition.get("properties", {})
        ) != fields:
            failures.append(f"report schema {name} fields drift from validator")
    _closed(schema, "$", failures)
    return sorted(set(failures))


def validate_files(root: Path, json_path: Path, markdown_path: Path, schema_path: Path) -> list[str]:
    root = root.resolve()
    json_file = json_path if json_path.is_absolute() else root / json_path
    markdown_file = markdown_path if markdown_path.is_absolute() else root / markdown_path
    schema_file = schema_path if schema_path.is_absolute() else root / schema_path
    try:
        raw = json_file.read_bytes()
        payload = json.loads(raw)
        markdown = markdown_file.read_text(encoding="utf-8")
        schema = json.loads(schema_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [f"Phase 12 audit files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict):
        return ["Phase 12 audit JSON and schema must be objects"]
    failures: list[str] = []
    if raw != canonical_json(payload).encode():
        failures.append("Phase 12 audit JSON must use canonical serialization")
    failures.extend(validate_schema(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    failures.extend(validate_payload(payload))
    outputs = payload.get("output_paths", {})
    try:
        expected_json = json_file.resolve().relative_to(root).as_posix()
        expected_markdown = markdown_file.resolve().relative_to(root).as_posix()
    except ValueError:
        failures.append("report outputs escape the workspace")
    else:
        if outputs.get("json") != expected_json or outputs.get("markdown") != expected_markdown:
            failures.append("output_paths do not identify the validated report pair")
    try:
        state = build_live_phase12_state(root, timestamp=payload.get("timestamp"))
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        failures.append(f"live Phase 12 audit failed: {exc}")
    else:
        failures.extend(validate_payload(payload, expected_state=state))
        if payload.get("input_paths") != list(state.input_paths):
            failures.append("input_paths do not match the complete live Phase 12 closure")
        failures.extend(validate_source_revision(root, payload.get("commit"), state.input_paths))
        if payload.get("input_digest") != input_digest(root, list(state.input_paths)):
            failures.append("input_digest does not match current Phase 12 inputs")
    digest = hashlib.sha256(raw).hexdigest()
    if markdown != render_markdown(payload, json_digest=digest):
        failures.append("Phase 12 audit Markdown does not exactly match JSON")
    return sorted(set(failures))


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, Mapping) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_phase12_workflow_ui_audit.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical Phase 12 report invocation")


def _rows(value: Any, fields: set[str], label: str, failures: list[str]) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return []
    rows: list[dict[str, Any]] = []
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"{label}[{index}] must be an object")
            continue
        _fields(row, fields, f"{label}[{index}]", failures)
        rows.append(dict(row))
    return rows


def _fields(value: Mapping[str, Any], expected: set[str], label: str, failures: list[str]) -> None:
    if set(value) != expected:
        failures.append(
            f"{label} fields drift: missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}"
        )


def _string_array(value: Any) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) and item for item in value)


def _timestamp(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).tzinfo is not None
    except ValueError:
        return False


def _closed(node: Any, path: str, failures: list[str]) -> None:
    if isinstance(node, Mapping):
        if node.get("type") == "object" and node.get("additionalProperties") is not False:
            failures.append(f"schema object {path} must be closed")
        for key, value in node.items():
            _closed(value, f"{path}.{key}", failures)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            _closed(value, f"{path}[{index}]", failures)
