"""Production file-level validation for Phase 9 fuzz-program reports."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath

from .fuzz_program_live import (
    build_live_fuzz_program_state,
    validate_source_revision,
)
from .fuzz_program_report import render_markdown
from .fuzz_program_report_contract import validate_report_payload, validate_schema_contract
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    root = root.resolve()
    failures: list[str] = []
    if schema_path.as_posix() != "verification/schemas/fuzz-program-audit-report.schema.json":
        failures.append("schema path must identify the committed Phase 9 report schema")
    json_file = _safe_report_path(root, json_path, "JSON", failures)
    markdown_file = _safe_report_path(root, markdown_path, "Markdown", failures)
    schema_file = _safe_report_path(root, schema_path, "schema", failures)
    if json_file is None or markdown_file is None or schema_file is None:
        return sorted(set(failures))
    if json_file == markdown_file:
        return sorted(set([*failures, "JSON and Markdown paths must be distinct"]))
    try:
        json_text = json_file.read_text()
        payload = json.loads(json_text)
    except (OSError, json.JSONDecodeError) as exc:
        return [f"fuzz-program JSON cannot be read: {exc}"]
    canonical = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if json_text != canonical:
        failures.append("fuzz-program JSON must use canonical sorted-key formatting")
    try:
        schema = json.loads(schema_file.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        return [f"fuzz-program report schema cannot be read: {exc}"]
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    try:
        expected_state = build_live_fuzz_program_state(root, require_clean_commit=False)
    except (OSError, ValueError) as exc:
        failures.append(f"current live Phase 9 state cannot be built: {exc}")
        expected_state = None
    failures.extend(validate_report_payload(payload, expected_state=expected_state))
    input_paths_value = payload.get("input_paths") if isinstance(payload, dict) else None
    if isinstance(input_paths_value, list) and all(isinstance(item, str) for item in input_paths_value):
        paths = tuple(input_paths_value)
        failures.extend(validate_bound_input_paths(root, paths))
        if payload.get("input_digest") != input_digest(root, list(paths)):
            failures.append("input_digest does not match current bound input contents")
        failures.extend(validate_source_revision(root, payload.get("commit"), paths))
    output_paths = payload.get("output_paths") if isinstance(payload, dict) else None
    if isinstance(output_paths, dict):
        if output_paths.get("json") != json_path.as_posix():
            failures.append("JSON path does not match report output_paths")
        if output_paths.get("markdown") != markdown_path.as_posix():
            failures.append("Markdown path does not match report output_paths")
    try:
        markdown_text = markdown_file.read_text()
    except OSError as exc:
        failures.append(f"fuzz-program Markdown cannot be read: {exc}")
    else:
        digest = hashlib.sha256(json_text.encode()).hexdigest()
        try:
            expected_markdown = render_markdown(payload, json_digest=digest)
        except Exception as exc:
            failures.append(f"fuzz-program Markdown cannot be reconstructed: {exc}")
        else:
            if markdown_text != expected_markdown:
                failures.append("Markdown does not exactly match the canonical JSON render")
    return sorted(set(failures))


def _safe_report_path(
    root: Path,
    value: Path,
    label: str,
    failures: list[str],
) -> Path | None:
    raw = value.as_posix()
    relative = PurePosixPath(raw)
    if (
        not relative.parts
        or value.is_absolute()
        or "\\" in raw
        or ".." in relative.parts
        or "." in relative.parts
    ):
        failures.append(f"{label} path must be normalized and workspace-relative")
        return None
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            failures.append(f"{label} path must not contain a symlink")
            return None
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError:
        failures.append(f"{label} path escapes the workspace")
        return None
    return candidate
