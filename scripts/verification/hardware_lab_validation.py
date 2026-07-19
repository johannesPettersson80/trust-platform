"""At-rest validation for the Phase 11 hardware-lab report."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .hardware_lab import CASE_FIELDS, CASE_IDS
from .hardware_lab_live import HardwareLabState, build_live_hardware_lab_state, validate_source_revision
from .hardware_lab_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    canonical_json,
    render_markdown,
)
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


TOP_FIELDS = {
    "schema_version", "generator", "generator_version", "report_status", "commit", "branch",
    "timestamp", "platform", "input_paths", "input_digest", "output_paths", "command",
    "boundaries", "summary", "public_claim", "cases", "limitations",
}
SUMMARY_FIELDS = {
    "cases", "protocols", "strict_harness_cases", "manual_script_cases",
    "skipped_unproven", "evidence_records",
}
PUBLIC_CLAIM_FIELDS = {"status", "spec_source_id", "hardware_qualified", "limitation"}
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


def validate_payload(payload: Mapping[str, Any], *, expected_state: HardwareLabState | None = None) -> list[str]:
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
    if not isinstance(payload.get("branch"), str) or not payload["branch"]:
        failures.append("branch must be non-empty")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload["platform"]:
        failures.append("platform must be non-empty")
    inputs = payload.get("input_paths")
    if not _strings(inputs) or inputs != sorted(set(inputs)):
        failures.append("input_paths must be a sorted unique non-empty string array")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or set(outputs) != {"json", "markdown"}:
        failures.append("output_paths must be a closed json/markdown object")
    elif not all(is_safe_relative_path(outputs.get(field)) for field in ("json", "markdown")):
        failures.append("output paths must be normalized workspace-relative paths")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries drift from the hardware-lab honesty contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations drift from the hardware-lab honesty contract")

    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
        summary = {}
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
    claim = payload.get("public_claim")
    if not isinstance(claim, Mapping):
        failures.append("public_claim must be an object")
        claim = {}
    else:
        _fields(claim, PUBLIC_CLAIM_FIELDS, "public_claim", failures)
    if claim.get("status") != "preview_unverified" or claim.get("hardware_qualified") is not False:
        failures.append("public hardware claims must remain preview_unverified and unqualified")

    cases = payload.get("cases")
    if not isinstance(cases, list):
        failures.append("cases must be an array")
        cases = []
    if tuple(row.get("id") for row in cases if isinstance(row, Mapping)) != CASE_IDS:
        failures.append("report cases drift from the reviewed hardware-lab order")
    for row in cases:
        if not isinstance(row, Mapping):
            failures.append("hardware-lab report case must be an object")
            continue
        _fields(row, CASE_FIELDS, f"case {row.get('id')}", failures)
        if row.get("proof_status") != "skipped_unproven":
            failures.append(f"case {row.get('id')} must remain skipped_unproven")
        if row.get("evidence_ids") != []:
            failures.append(f"case {row.get('id')} cannot carry hardware evidence")
    derived = {
        "cases": len(cases),
        "protocols": len({row.get("protocol") for row in cases if isinstance(row, Mapping)}),
        "strict_harness_cases": sum(row.get("binding_kind") == "strict_harness" for row in cases if isinstance(row, Mapping)),
        "manual_script_cases": sum(row.get("binding_kind") == "manual_script" for row in cases if isinstance(row, Mapping)),
        "skipped_unproven": sum(row.get("proof_status") == "skipped_unproven" for row in cases if isinstance(row, Mapping)),
        "evidence_records": sum(len(row.get("evidence_ids", [])) for row in cases if isinstance(row, Mapping) and isinstance(row.get("evidence_ids"), list)),
    }
    if dict(summary) != derived:
        failures.append("summary is not derived from hardware-lab cases")
    _validate_command(payload, failures)

    if expected_state is not None:
        if cases != list(expected_state.cases):
            failures.append("cases do not match current live hardware-lab state")
        if dict(summary) != expected_state.summary:
            failures.append("summary does not match current live hardware-lab state")
        if dict(claim) != expected_state.public_claim:
            failures.append("public claim does not match current live hardware-lab state")
    return sorted(set(failures))


def validate_schema(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("hardware-lab report schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS or set(schema.get("properties", {})) != TOP_FIELDS:
        failures.append("hardware-lab report schema top-level fields drift from validator")
    definitions = schema.get("$defs", {})
    for name, fields in (
        ("output_paths", {"json", "markdown"}),
        ("boundaries", set(BOUNDARIES)),
        ("summary", SUMMARY_FIELDS),
        ("public_claim", PUBLIC_CLAIM_FIELDS),
        ("case", CASE_FIELDS),
    ):
        definition = definitions.get(name, {})
        if set(definition.get("required", [])) != fields or set(definition.get("properties", {})) != fields:
            failures.append(f"hardware-lab report schema {name} fields drift from validator")
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
        return [f"hardware-lab report files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict):
        return ["hardware-lab report JSON and schema must be objects"]
    failures: list[str] = []
    if raw != canonical_json(payload).encode("utf-8"):
        failures.append("hardware-lab report JSON must use canonical serialization")
    failures.extend(validate_schema(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    failures.extend(validate_payload(payload))
    outputs = payload.get("output_paths", {})
    try:
        expected_json = json_file.resolve().relative_to(root).as_posix()
        expected_markdown = markdown_file.resolve().relative_to(root).as_posix()
    except ValueError:
        failures.append("hardware-lab report outputs escape the workspace")
    else:
        if outputs.get("json") != expected_json or outputs.get("markdown") != expected_markdown:
            failures.append("output_paths do not identify the validated hardware-lab report pair")
    try:
        state = build_live_hardware_lab_state(
            root,
            branch=str(payload.get("branch", "")),
            timestamp=str(payload.get("timestamp", "")),
            json_path=Path(str(outputs.get("json", ""))),
            markdown_path=Path(str(outputs.get("markdown", ""))),
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        failures.append(f"live hardware-lab validation failed: {exc}")
    else:
        failures.extend(validate_payload(payload, expected_state=state))
        if payload.get("input_paths") != list(state.input_paths):
            failures.append("input_paths do not match the complete live hardware-lab closure")
        failures.extend(validate_source_revision(root, payload.get("commit"), state.input_paths))
        if payload.get("input_digest") != input_digest(root, list(state.input_paths)):
            failures.append("input_digest does not match current hardware-lab inputs")
    digest = hashlib.sha256(raw).hexdigest()
    if markdown != render_markdown(payload, json_digest=digest):
        failures.append("hardware-lab Markdown does not exactly match JSON")
    return sorted(set(failures))


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths", {})
    expected = [
        "python3", "scripts/report_hardware_lab.py",
        "--json-out", outputs.get("json"),
        "--markdown-out", outputs.get("markdown"),
        "--branch", payload.get("branch"),
        "--timestamp", payload.get("timestamp"),
    ]
    if payload.get("command") != expected:
        failures.append("command does not match the canonical hardware-lab generator invocation")


def _fields(value: Mapping[str, Any], expected: set[str], label: str, failures: list[str]) -> None:
    if set(value) != expected:
        failures.append(f"{label} fields drift from the closed contract")


def _strings(value: Any) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) and item for item in value)


def _timestamp(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        return datetime.fromisoformat(value).tzinfo is not None
    except ValueError:
        return False


def _closed(value: Any, path: str, failures: list[str]) -> None:
    if isinstance(value, Mapping):
        if value.get("type") == "object" and value.get("additionalProperties") is not False:
            failures.append(f"hardware-lab report schema object {path} must be closed")
        for key, child in value.items():
            _closed(child, f"{path}/{key}", failures)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _closed(child, f"{path}/{index}", failures)
