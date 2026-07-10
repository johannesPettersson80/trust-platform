"""At-rest validation for the combined Phase 5 suite audit report."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .phase5_audit_live import build_live_phase5_state, validate_source_revision
from .phase5_audit_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    build_summary,
    render_markdown,
)
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
TOP_FIELDS = {
    "schema_version", "generator", "generator_version", "report_status", "input_digest",
    "command", "commit", "timestamp", "platform", "input_paths", "output_paths",
    "boundaries", "inventory", "suites", "areas", "routes", "summary", "limitations",
}
OUTPUT_FIELDS = {"json", "markdown"}
BOUNDARY_FIELDS = set(BOUNDARIES)
INVENTORY_FIELDS = {
    "schema_version", "id", "discovery_id", "source_kind", "path", "name", "command",
    "variant", "command_role", "disposition", "suite_ids", "owner", "duration_class", "environment",
    "artifact_kind", "artifact_paths", "artifact_retention", "enforcement", "required_env",
    "rationale",
}
SUITE_FIELDS = {
    "id", "title", "status", "owner", "duration_class", "environment", "direct_commands",
    "direct_command_bindings", "direct_inventory_refs", "evidence_destination", "includes", "excludes",
}
AREA_FIELDS = {
    "id", "status", "owner", "risk_default", "path_globs", "required_test_classes",
    "required_case_families", "direct_suite_tiers",
}
ROUTE_FIELDS = {
    "order", "id", "match_kind", "area_ids", "path_globs", "intents",
    "required_test_classes", "direct_suite_tiers", "conditional_suite_tiers", "notes",
}
SUMMARY_FIELDS = {
    "inventory_records", "live_inventory_records", "suite_records", "suite_direct_commands",
    "suite_inventory_refs", "canonical_areas", "taxonomy_routes", "path_routes", "intent_routes",
    "by_disposition", "by_source_kind", "by_suite", "by_enforcement", "by_artifact_kind",
    "route_direct_suite_tiers", "route_conditional_suite_tiers",
}
COUNT_FIELDS = {"name", "count"}


def validate_report_payload(payload: Mapping[str, Any], *, expected_state=None) -> list[str]:
    failures: list[str] = []
    _fields(payload, TOP_FIELDS, "report", failures)
    for field, expected in (
        ("schema_version", 1), ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION), ("report_status", "complete"),
    ):
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected!r}")
    if not isinstance(payload.get("commit"), str) or not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must identify a clean full Git SHA")
    if not isinstance(payload.get("input_digest"), str) or not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
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
            value = outputs.get(field)
            if not isinstance(value, str) or not is_safe_relative_path(value):
                failures.append(f"output_paths.{field} must be normalized and workspace-relative")
    _validate_command(payload, failures)
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries do not match the non-enforcement boundary contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the Phase 5 audit contract")

    inventory = _rows(payload, "inventory", INVENTORY_FIELDS, failures)
    suites = _rows(payload, "suites", SUITE_FIELDS, failures)
    areas = _rows(payload, "areas", AREA_FIELDS, failures)
    routes = _rows(payload, "routes", ROUTE_FIELDS, failures)
    if len(inventory) != 62 or sum(row.get("discovery_id") is not None for row in inventory) != 59:
        failures.append("inventory must contain 62 records with exactly 59 scanner-bound rows")
    if len(suites) != 6:
        failures.append("suites must contain exactly six suite records")
    if len(areas) != 11:
        failures.append("areas must contain exactly 11 canonical areas")
    if len(routes) != 29 or [row.get("order") for row in routes] != list(range(1, 30)):
        failures.append("routes must contain the 29 taxonomy rows in reviewed order")
    if [row.get("id") for row in inventory] != sorted(row.get("id") for row in inventory):
        failures.append("inventory rows must use canonical ID order")
    if [row.get("id") for row in suites] != sorted(row.get("id") for row in suites):
        failures.append("suite rows must use canonical ID order")
    report_only = [row for row in inventory if row.get("disposition") == "report_only"]
    if len(report_only) != 2 or any(row.get("enforcement") != "report_only" for row in report_only):
        failures.append("report_only inventory enforcement changed")
    for index, row in enumerate(routes):
        direct = row.get("direct_suite_tiers")
        conditional = row.get("conditional_suite_tiers")
        if not _string_array(direct) or not _string_array(conditional):
            failures.append(f"routes[{index}] direct/conditional suite tiers must be arrays")
        elif set(direct) & set(conditional):
            failures.append(f"routes[{index}] direct and conditional suite tiers overlap")

    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
        for field in SUMMARY_FIELDS - {
            "inventory_records", "live_inventory_records", "suite_records", "suite_direct_commands",
            "suite_inventory_refs", "canonical_areas", "taxonomy_routes", "path_routes", "intent_routes",
        }:
            counts = summary.get(field)
            if isinstance(counts, list):
                for index, item in enumerate(counts):
                    if isinstance(item, Mapping):
                        _fields(item, COUNT_FIELDS, f"summary.{field}[{index}]", failures)
        if dict(summary) != build_summary(inventory, suites, areas, routes):
            failures.append("summary does not match report rows")
    if expected_state is not None:
        expected = (
            list(expected_state.inventory_rows), list(expected_state.suite_rows),
            list(expected_state.area_rows), list(expected_state.route_rows), expected_state.boundaries,
        )
        actual = (inventory, suites, areas, routes, payload.get("boundaries"))
        if actual != expected:
            failures.append("report rows do not match current live Phase 5 state")
    return sorted(set(failures))


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("report schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS or set(schema.get("properties", {})) != TOP_FIELDS:
        failures.append("report schema top-level fields drift from validator")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1), ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION), ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"report schema const for {field} drifts from validator")
    expected_defs = {
        "output_paths": OUTPUT_FIELDS, "boundary": BOUNDARY_FIELDS, "inventory_row": INVENTORY_FIELDS,
        "suite_row": SUITE_FIELDS, "area_row": AREA_FIELDS, "route_row": ROUTE_FIELDS,
        "summary": SUMMARY_FIELDS, "count": COUNT_FIELDS,
    }
    definitions = schema.get("$defs", {})
    for name, fields in expected_defs.items():
        definition = definitions.get(name)
        if not isinstance(definition, Mapping) or set(definition.get("required", [])) != fields or set(definition.get("properties", {})) != fields:
            failures.append(f"report schema {name.replace('_', ' ')} fields drift from validator")
    _closed(schema, "$", failures)
    return sorted(set(failures))


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
    *,
    allow_external_test_outputs: bool = False,
) -> list[str]:
    root = root.resolve()
    json_file = json_path if json_path.is_absolute() else root / json_path
    markdown_file = markdown_path if markdown_path.is_absolute() else root / markdown_path
    schema_file = schema_path if schema_path.is_absolute() else root / schema_path
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
        markdown = markdown_file.read_text()
        schema = json.loads(schema_file.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [f"Phase 5 audit files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict):
        return ["Phase 5 audit JSON and schema must be objects"]
    failures: list[str] = []
    canonical = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("Phase 5 audit JSON must use canonical serialization")
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    failures.extend(validate_report_payload(payload))
    if not allow_external_test_outputs:
        outputs = payload.get("output_paths", {})
        try:
            expected_json = json_file.resolve().relative_to(root).as_posix()
            expected_markdown = markdown_file.resolve().relative_to(root).as_posix()
        except ValueError:
            failures.append("report outputs escape the workspace")
        else:
            if outputs.get("json") != expected_json:
                failures.append("output_paths.json does not identify the validated JSON")
            if outputs.get("markdown") != expected_markdown:
                failures.append("output_paths.markdown does not identify the validated Markdown")
    timestamp = payload.get("timestamp") if isinstance(payload.get("timestamp"), str) else None
    try:
        state = build_live_phase5_state(root, timestamp=timestamp)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        failures.append(f"live Phase 5 audit failed: {exc}")
    else:
        failures.extend(validate_report_payload(payload, expected_state=state))
        if payload.get("input_paths") != list(state.input_paths):
            failures.append("input_paths do not match the complete live Phase 5 closure")
        failures.extend(validate_source_revision(root, payload.get("commit"), state.input_paths))
        if payload.get("input_digest") != input_digest(root, list(state.input_paths)):
            failures.append("input_digest does not match current Phase 5 inputs")
    digest = hashlib.sha256(json_bytes).hexdigest()
    if markdown != render_markdown(payload, json_digest=digest):
        failures.append("Phase 5 audit Markdown does not exactly match JSON")
    if f"Generated JSON SHA-256: `{digest}`" not in markdown:
        failures.append("Phase 5 audit Markdown has a stale JSON digest")
    return sorted(set(failures))


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or not isinstance(payload.get("timestamp"), str):
        return
    expected = [
        "python3", "scripts/report_phase5_suite_audit.py", "--json-out", outputs.get("json"),
        "--markdown-out", outputs.get("markdown"), "--timestamp", payload["timestamp"],
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical Phase 5 report invocation")


def _rows(payload, field, expected_fields, failures):
    value = payload.get(field)
    if not isinstance(value, list):
        failures.append(f"{field} must be an array")
        return []
    rows = []
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"{field}[{index}] must be an object")
            continue
        _fields(row, expected_fields, f"{field}[{index}]", failures)
        rows.append(dict(row))
    return rows


def _fields(value, expected, label, failures):
    actual = set(value)
    if actual != expected:
        failures.append(f"{label} fields drift: missing={sorted(expected-actual)}, extra={sorted(actual-expected)}")


def _string_array(value) -> bool:
    return isinstance(value, list) and all(isinstance(item, str) and item for item in value)


def _timestamp(value) -> bool:
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
