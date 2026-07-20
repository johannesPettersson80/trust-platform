"""At-rest validation for Phase 7 conformance alignment artifacts."""

from __future__ import annotations

import json
from collections.abc import Mapping
from pathlib import Path

from .conformance_alignment_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from .conformance_alignment_live import (
    build_live_conformance_alignment_state,
    validate_source_revision,
)
from .test_catalog_json_schema import validate_json_schema_instance


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
        return [f"conformance alignment files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict):
        return ["conformance alignment JSON and schema must be objects"]

    failures: list[str] = []
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    failures.extend(validate_report_payload(payload))
    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    outputs = payload.get("output_paths")
    if not allow_external_test_outputs and isinstance(outputs, Mapping):
        try:
            expected_json = json_file.resolve().relative_to(root).as_posix()
            expected_markdown = markdown_file.resolve().relative_to(root).as_posix()
        except ValueError:
            failures.append("conformance alignment outputs escape the workspace")
        else:
            if outputs.get("json") != expected_json:
                failures.append("output_paths.json does not identify the validated JSON")
            if outputs.get("markdown") != expected_markdown:
                failures.append("output_paths.markdown does not identify the validated Markdown")

    timestamp = payload.get("timestamp") if isinstance(payload.get("timestamp"), str) else None
    try:
        state = build_live_conformance_alignment_state(root, timestamp=timestamp)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        failures.append(f"live conformance alignment failed: {exc}")
    else:
        failures.extend(validate_report_payload(payload, expected_analysis=state.analysis))
        if payload.get("input_paths") != list(state.input_paths):
            failures.append("input_paths do not match the complete live conformance closure")
        if payload.get("input_digest") != state.input_digest:
            failures.append("input_digest does not match current conformance alignment inputs")
        failures.extend(validate_source_revision(root, payload.get("commit"), state.input_paths))
    return sorted(set(failures))
