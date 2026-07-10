"""At-rest validation for the Phase 4 invariant-seed audit report."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .invariant_seed_contract import BOARD_ROW_AREAS, SeedAuditRow
from .invariant_seed_live import build_live_seed_audit_state, validate_source_revision
from .invariant_seed_report import (
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    SCOPE,
    build_summary,
    render_markdown,
)
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import (
    check_supported_schema_keywords,
    is_safe_relative_path,
)


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
DATED_EVIDENCE_RE = re.compile(
    r"^docs/internal/testing/evidence/plc-verification-program/\d{4}-\d{2}-\d{2}/[^/]+\.md$"
)
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
    "rows",
    "summary",
    "limitations",
}
ROW_FIELDS = {
    "seed_id",
    "seed_title",
    "source_section",
    "source_line",
    "canonical_invariant_id",
    "invariant_path",
    "invariant_area",
    "board_row",
    "origin",
    "status",
    "proof_level",
    "risk",
    "oracle_ref",
    "spec_gap_refs",
    "source_refs",
    "test_ids",
    "evidence_ids",
    "p4_000_risk_id",
}
SUMMARY_FIELDS = {
    "seeds",
    "canonical_invariants",
    "merged_seed_aliases",
    "phase4_records",
    "preexisting_seed_mappings",
    "gap_open",
    "spec_gap",
    "p4_000_risks",
    "by_board_row",
}
BOARD_COUNT_FIELDS = {"board_row", "seeds"}


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_rows: tuple[SeedAuditRow, ...] | None = None,
) -> list[str]:
    failures: list[str] = []
    _check_fields(payload, TOP_FIELDS, "report", failures)
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
        failures.append("commit must be a clean full Git SHA")
    if not isinstance(payload.get("input_digest"), str) or not DIGEST_RE.fullmatch(
        str(payload.get("input_digest", ""))
    ):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _is_iso_timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")
    inputs = payload.get("input_paths")
    if not isinstance(inputs, list) or not all(isinstance(item, str) and item for item in inputs):
        failures.append("input_paths must be a non-empty string array")
    elif inputs != sorted(set(inputs)):
        failures.append("input_paths must be sorted and duplicate-free")
    _validate_outputs(payload.get("output_paths"), failures)
    _validate_command(payload, failures)
    if payload.get("scope") != SCOPE:
        failures.append("scope does not match the non-proof invariant-seed audit contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the invariant-seed audit contract")

    rows = payload.get("rows")
    if not isinstance(rows, list):
        failures.append("rows must be an array")
        rows = []
    else:
        for index, row in enumerate(rows):
            if not isinstance(row, Mapping):
                failures.append(f"rows[{index}] must be an object")
                continue
            _check_fields(row, ROW_FIELDS, f"rows[{index}]", failures)
            if row.get("status") not in {"gap_open", "spec_gap"}:
                failures.append(f"rows[{index}].status must remain gap_open or spec_gap")
            if row.get("proof_level") != "S0":
                failures.append(f"rows[{index}].proof_level must remain S0")
            if row.get("origin") == "phase4" and (row.get("test_ids") or row.get("evidence_ids")):
                failures.append(f"rows[{index}] phase4 record carries premature associations")
            for field in ("spec_gap_refs", "source_refs", "test_ids", "evidence_ids"):
                value = row.get(field)
                if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
                    failures.append(f"rows[{index}].{field} must be a string array")
                elif value != sorted(set(value)):
                    failures.append(f"rows[{index}].{field} must be canonical and duplicate-free")
    expected = [row.to_dict() for row in expected_rows] if expected_rows is not None else None
    if expected is not None and rows != expected:
        failures.append("rows do not match current live seed audit")
    if len(rows) != 44 or len({row.get("canonical_invariant_id") for row in rows if isinstance(row, Mapping)}) != 43:
        failures.append("rows must represent 44 written seeds and 43 canonical invariants")
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
    else:
        _check_fields(summary, SUMMARY_FIELDS, "summary", failures)
        board_counts = summary.get("by_board_row")
        if isinstance(board_counts, list):
            for index, item in enumerate(board_counts):
                if isinstance(item, Mapping):
                    _check_fields(item, BOARD_COUNT_FIELDS, f"summary.by_board_row[{index}]", failures)
        if dict(summary) != build_summary(rows):
            failures.append("summary does not match rows")
        if {item.get("board_row") for item in summary.get("by_board_row", []) if isinstance(item, Mapping)} != set(BOARD_ROW_AREAS):
            failures.append("summary must represent every VERIF-P4-001 through VERIF-P4-008 row")
    return sorted(set(failures))


def validate_schema_contract(
    schema: Mapping[str, Any],
    *,
    manifest_schema: Mapping[str, Any] | None = None,
) -> list[str]:
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
    expected_defs = {
        "output_paths": {"json", "markdown"},
        "scope": set(SCOPE),
        "row": ROW_FIELDS,
        "summary": SUMMARY_FIELDS,
        "board_count": BOARD_COUNT_FIELDS,
    }
    for name, fields in expected_defs.items():
        definition = definitions.get(name)
        if not isinstance(definition, Mapping):
            failures.append(f"report schema lacks {name} definition")
        elif set(definition.get("required", [])) != fields or set(definition.get("properties", {})) != fields:
            failures.append(f"report schema {name} fields drift from validator")
    row = definitions.get("row", {}).get("properties", {})
    if set(row.get("board_row", {}).get("enum", [])) != set(BOARD_ROW_AREAS):
        failures.append("report schema board-row enum drifts from validator")
    if set(row.get("status", {}).get("enum", [])) != {"gap_open", "spec_gap"}:
        failures.append("report schema status enum drifts from validator")
    _require_closed_objects(schema, "$", failures)
    if manifest_schema is not None:
        _validate_manifest_schema_contract(manifest_schema, failures)
    return sorted(set(failures))


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    root = root.resolve()
    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    schema_file = _absolute(root, schema_path)
    failures: list[str] = []
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
        markdown = markdown_file.read_text()
        schema = json.loads(schema_file.read_text())
        manifest_schema = json.loads(
            (root / "verification/schemas/invariant-seed-manifest.schema.json").read_text()
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [f"invariant-seed audit files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict) or not isinstance(manifest_schema, dict):
        return ["invariant-seed audit JSON and schemas must be objects"]
    canonical = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("invariant-seed audit JSON must use canonical serialization")
    failures.extend(validate_schema_contract(schema, manifest_schema=manifest_schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    failures.extend(validate_report_payload(payload))

    expected_json = _relative(root, json_file, "JSON output", failures)
    expected_markdown = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths")
    if isinstance(outputs, Mapping):
        if expected_json is not None and outputs.get("json") != expected_json:
            failures.append("output_paths.json does not identify the validated JSON file")
        if expected_markdown is not None and outputs.get("markdown") != expected_markdown:
            failures.append("output_paths.markdown does not identify the validated Markdown file")
    timestamp = payload.get("timestamp") if isinstance(payload.get("timestamp"), str) else None
    try:
        state = build_live_seed_audit_state(root, timestamp=timestamp)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        failures.append(f"live invariant-seed audit failed: {exc}")
    else:
        failures.extend(validate_report_payload(payload, expected_rows=state.audit.rows))
        if payload.get("input_paths") != list(state.input_paths):
            failures.append("input_paths do not match the complete live seed-audit closure")
        failures.extend(validate_source_revision(root, payload.get("commit"), state.input_paths))
        if payload.get("input_digest") != input_digest(root, list(state.input_paths)):
            failures.append("input_digest does not match current seed-audit inputs")
    digest = hashlib.sha256(json_bytes).hexdigest()
    expected_markdown_text = render_markdown(payload, json_digest=digest)
    if markdown != expected_markdown_text:
        failures.append("invariant-seed audit Markdown does not exactly match JSON")
    if f"Generated JSON SHA-256: `{digest}`" not in markdown:
        failures.append("invariant-seed audit Markdown has a stale JSON digest")
    return sorted(set(failures))


def _validate_manifest_schema_contract(schema: Mapping[str, Any], failures: list[str]) -> None:
    check_supported_schema_keywords(schema, "$manifest", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("manifest schema root must be a closed object")
    if set(schema.get("required", [])) != {"schema_version", "seeds"}:
        failures.append("manifest schema required fields drift from validator")
    if set(schema.get("properties", {})) != {"schema_version", "seeds"}:
        failures.append("manifest schema properties drift from validator")
    seed = schema.get("$defs", {}).get("seed", {})
    required = {"seed_id", "canonical_invariant_id", "board_row", "origin"}
    properties = required | {"p4_000_risk_id"}
    if set(seed.get("required", [])) != required or set(seed.get("properties", {})) != properties:
        failures.append("manifest schema seed fields drift from validator")
    if seed.get("additionalProperties") is not False:
        failures.append("manifest schema seed object must be closed")
    if set(seed.get("properties", {}).get("board_row", {}).get("enum", [])) != set(BOARD_ROW_AREAS):
        failures.append("manifest schema board-row enum drifts from validator")
    if set(seed.get("properties", {}).get("origin", {}).get("enum", [])) != {"phase4", "preexisting"}:
        failures.append("manifest schema origin enum drifts from validator")
    _require_closed_objects(schema, "$manifest", failures)


def _validate_outputs(value: Any, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        failures.append("output_paths must be an object")
        return
    _check_fields(value, {"json", "markdown"}, "output_paths", failures)
    json_path = value.get("json")
    markdown_path = value.get("markdown")
    if not isinstance(json_path, str) or not json_path.startswith("target/gate-artifacts/verification/") or not is_safe_relative_path(json_path):
        failures.append("output_paths.json must be a normalized verification gate-artifact path")
    if (
        not isinstance(markdown_path, str)
        or not (
            markdown_path.startswith("target/gate-artifacts/verification/")
            or DATED_EVIDENCE_RE.fullmatch(markdown_path)
        )
        or not is_safe_relative_path(markdown_path)
    ):
        failures.append("output_paths.markdown must be a normalized gate-artifact or dated evidence path")


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or not isinstance(payload.get("timestamp"), str):
        return
    expected = [
        "python3",
        "scripts/report_invariant_seed_audit.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        payload["timestamp"],
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical invariant-seed audit invocation")


def _check_fields(value: Mapping[str, Any], expected: set[str], label: str, failures: list[str]) -> None:
    missing = sorted(expected - set(value))
    extra = sorted(set(value) - expected)
    if missing:
        failures.append(f"{label} is missing fields: {', '.join(missing)}")
    if extra:
        failures.append(f"{label} has unknown fields: {', '.join(extra)}")


def _require_closed_objects(value: Any, path: str, failures: list[str]) -> None:
    if isinstance(value, Mapping):
        if value.get("type") == "object" and value.get("additionalProperties") is not False:
            failures.append(f"schema object {path} must set additionalProperties false")
        for key, child in value.items():
            _require_closed_objects(child, f"{path}.{key}", failures)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _require_closed_objects(child, f"{path}[{index}]", failures)


def _is_iso_timestamp(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(root: Path, path: Path, label: str, failures: list[str]) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except ValueError:
        failures.append(f"{label} escapes the workspace")
        return None
