"""At-rest validation for the Phase 13 release-evidence audit."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .phase13_release import (
    BOUNDARIES, GENERATOR, GENERATOR_VERSION, LIMITATIONS, PLATFORM_IDS,
    PROOF_ORIGINS, REQUIRED_RELEASE_ASSETS, canonical_json, render_markdown,
)
from .phase13_release_live import build_live_phase13_state, validate_source_revision
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


TOP_FIELDS = {
    "schema_version", "generator", "generator_version", "report_status", "commit", "branch",
    "timestamp", "platform", "input_paths", "input_digest", "output_paths", "command",
    "boundaries", "candidate", "public_release", "proof_origins", "security", "platforms",
    "conformance", "hardware_labs", "ui_acceptance", "known_gaps", "limitations",
}
SECTION_FIELDS = {
    "candidate": {"version", "expected_tag", "version_sources", "versions_synchronized", "changelog_mentions_version", "annotated_tag_present", "release_complete"},
    "public_release": {"status", "checked_at", "tag", "commit", "published_at", "release_url", "workflow_run_id", "workflow_run_url", "workflow_conclusion", "assets", "required_assets", "missing_required_assets", "matches_candidate"},
    "proof_origin": {"origin", "evidence_count", "status", "limitation"},
    "security": {"owned_exceptions", "expired_exceptions", "maximum_exception_days", "cargo_policy_configured", "npm_audit_configured", "rust_commands", "node_commands", "gate_execution_claimed"},
    "platform": {"id", "target", "support_tier", "required_proof", "runtime_asset", "lsp_asset", "vsix_asset_template", "snapshot_tag", "expected_public_assets", "public_assets_present"},
    "conformance": {"catalog_cases", "linked_cases", "missing_links", "public_asset_present", "execution_claimed"},
    "hardware_lab": {"board_row", "status", "evidence_count"},
    "ui_acceptance": {"journeys", "accepted_journeys", "provisional_journeys", "missing_journeys", "stale_journeys"},
    "known_gap": {"id", "status", "detail"},
}
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


def validate_payload(payload: Mapping[str, Any], *, expected_state=None) -> list[str]:
    failures: list[str] = []
    _fields(payload, TOP_FIELDS, "report", failures)
    for field, expected in (
        ("schema_version", 1), ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION), ("report_status", "complete"),
    ):
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected!r}")
    if not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must identify a clean full Git SHA")
    if not isinstance(payload.get("branch"), str) or not payload["branch"]:
        failures.append("branch must be non-empty")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    inputs = payload.get("input_paths")
    if not _string_array(inputs) or inputs != sorted(set(inputs)):
        failures.append("input_paths must be a sorted unique non-empty string array")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or set(outputs) != {"json", "markdown"}:
        failures.append("output_paths must be a closed json/markdown object")
    elif not all(is_safe_relative_path(outputs.get(field)) for field in ("json", "markdown")):
        failures.append("output paths must be normalized workspace-relative paths")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries drift from the Phase 13 honesty contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations drift from the Phase 13 honesty contract")
    _validate_command(payload, failures)

    candidate = _section(payload, "candidate", failures)
    public = _section(payload, "public_release", failures)
    security = _section(payload, "security", failures)
    conformance = _section(payload, "conformance", failures)
    ui = _section(payload, "ui_acceptance", failures)
    if candidate and candidate.get("expected_tag") != f"v{candidate.get('version')}":
        failures.append("candidate expected_tag does not match version")
    if public:
        missing = sorted(set(public.get("required_assets", [])) - set(public.get("assets", [])))
        if public.get("required_assets") != list(REQUIRED_RELEASE_ASSETS):
            failures.append("public release required assets drift from the release guard")
        if public.get("missing_required_assets") != missing:
            failures.append("public release missing asset list is not derived")
        if candidate and public.get("matches_candidate") is not (public.get("tag") == candidate.get("expected_tag")):
            failures.append("public release candidate-match flag is not derived")
    if candidate and public:
        complete = bool(candidate.get("annotated_tag_present")) and bool(public.get("matches_candidate")) and not public.get("missing_required_assets")
        if candidate.get("release_complete") is not complete:
            failures.append("candidate release_complete is not derived from tag, Latest, and assets")
    origins = _rows(payload.get("proof_origins"), "proof_origin", failures)
    if [row.get("origin") for row in origins] != list(PROOF_ORIGINS):
        failures.append("proof origins drift from reviewed order")
    for row in origins:
        if row.get("status") == "recorded" and not isinstance(row.get("evidence_count"), int):
            failures.append(f"proof origin {row.get('origin')} has invalid evidence count")
        if row.get("status") == "recorded" and row.get("evidence_count", 0) <= 0:
            failures.append(f"proof origin {row.get('origin')} claims recorded without evidence")
        if row.get("status") in {"missing", "snapshot_only"} and row.get("evidence_count") != 0:
            failures.append(f"proof origin {row.get('origin')} status conflicts with evidence count")
    if security:
        if security.get("maximum_exception_days") != 90:
            failures.append("dependency exceptions must expire within 90 days")
        if security.get("gate_execution_claimed") is not False:
            failures.append("configured security policy cannot claim gate execution")
    platforms = _rows(payload.get("platforms"), "platform", failures)
    if [row.get("id") for row in platforms] != list(PLATFORM_IDS):
        failures.append("platform rows drift from reviewed order")
    for row in platforms:
        if row.get("support_tier") == "artifact_only" and "native_ci_test" in row.get("required_proof", []):
            failures.append(f"artifact-only platform {row.get('id')} claims native execution proof")
        if row.get("public_assets_present") is not (set(row.get("expected_public_assets", [])) <= set(public.get("assets", []) if public else [])):
            failures.append(f"platform {row.get('id')} public asset flag is not derived")
    if conformance:
        if conformance.get("linked_cases", 0) > conformance.get("catalog_cases", 0):
            failures.append("conformance linked count exceeds catalog count")
        if conformance.get("execution_claimed") is not False:
            failures.append("cataloged conformance inputs cannot claim execution")
    labs = _rows(payload.get("hardware_labs"), "hardware_lab", failures)
    if len(labs) != 5 or any(row.get("status") != "skipped_unproven" for row in labs):
        failures.append("hardware lab rows must remain five explicit skipped/unproven rows")
    if ui and sum(ui.get(field, 0) for field in ("accepted_journeys", "provisional_journeys", "missing_journeys", "stale_journeys")) != ui.get("journeys"):
        failures.append("UI journey status counts do not partition the denominator")
    gaps = _rows(payload.get("known_gaps"), "known_gap", failures)
    if len({row.get("id") for row in gaps}) != len(gaps):
        failures.append("known gaps contain duplicate IDs")

    if expected_state is not None:
        for field in (
            "candidate", "public_release", "security", "conformance", "ui_acceptance",
        ):
            if payload.get(field) != getattr(expected_state, field):
                failures.append(f"{field} does not match current live Phase 13 state")
        for field in ("proof_origins", "platforms", "hardware_labs", "known_gaps"):
            if payload.get(field) != list(getattr(expected_state, field)):
                failures.append(f"{field} does not match current live Phase 13 state")
    return sorted(set(failures))


def validate_schema(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("report schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS or set(schema.get("properties", {})) != TOP_FIELDS:
        failures.append("report schema top-level fields drift from validator")
    definitions = schema.get("$defs", {})
    definitions_fields = {
        "output_paths": {"json", "markdown"},
        "boundaries": set(BOUNDARIES),
        **SECTION_FIELDS,
    }
    for name, fields in definitions_fields.items():
        definition = definitions.get(name, {})
        if set(definition.get("required", [])) != fields or set(definition.get("properties", {})) != fields:
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
        return [f"Phase 13 audit files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict):
        return ["Phase 13 audit JSON and schema must be objects"]
    failures: list[str] = []
    if raw != canonical_json(payload).encode():
        failures.append("Phase 13 audit JSON must use canonical serialization")
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
        state = build_live_phase13_state(root, branch=payload.get("branch", ""), timestamp=payload.get("timestamp"))
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        failures.append(f"live Phase 13 audit failed: {exc}")
    else:
        failures.extend(validate_payload(payload, expected_state=state))
        if payload.get("input_paths") != list(state.input_paths):
            failures.append("input_paths do not match the complete live Phase 13 closure")
        failures.extend(validate_source_revision(root, payload.get("commit"), state.input_paths))
        if payload.get("input_digest") != input_digest(root, list(state.input_paths)):
            failures.append("input_digest does not match current Phase 13 inputs")
    digest = hashlib.sha256(raw).hexdigest()
    if markdown != render_markdown(payload, json_digest=digest):
        failures.append("Phase 13 audit Markdown does not exactly match JSON")
    return sorted(set(failures))


def _section(payload, name, failures):
    value = payload.get(name)
    if not isinstance(value, Mapping):
        failures.append(f"{name} must be an object")
        return {}
    _fields(value, SECTION_FIELDS[name], name, failures)
    return value


def _rows(value, definition, failures):
    if not isinstance(value, list):
        failures.append(f"{definition} rows must be an array")
        return []
    rows = []
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"{definition}[{index}] must be an object")
            continue
        _fields(row, SECTION_FIELDS[definition], f"{definition}[{index}]", failures)
        rows.append(row)
    return rows


def _fields(value, expected, label, failures):
    missing = sorted(expected - set(value))
    extra = sorted(set(value) - expected)
    if missing or extra:
        failures.append(f"{label} fields mismatch; missing={missing}, extra={extra}")


def _validate_command(payload, failures):
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping):
        return
    expected = [
        "python3", "scripts/report_phase13_release_evidence.py",
        "--json-out", outputs.get("json"), "--markdown-out", outputs.get("markdown"),
        "--branch", payload.get("branch"), "--timestamp", payload.get("timestamp"),
    ]
    if payload.get("command") != expected:
        failures.append("command does not match the canonical Phase 13 invocation")


def _timestamp(value):
    if not isinstance(value, str):
        return False
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _string_array(value):
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) and item for item in value)


def _closed(value: Any, path: str, failures: list[str]) -> None:
    if isinstance(value, Mapping):
        if value.get("type") == "object" and value.get("additionalProperties") is not False:
            failures.append(f"schema object at {path} must set additionalProperties=false")
        for key, item in value.items():
            _closed(item, f"{path}.{key}", failures)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            _closed(item, f"{path}[{index}]", failures)
