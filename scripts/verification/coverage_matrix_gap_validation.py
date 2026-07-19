"""At-rest validation for report-only coverage-matrix gap reports."""

from __future__ import annotations

import hashlib
import json
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .coverage_matrix_gap_contract import (
    ANALYSIS_FIELDS,
    CLEAN_COMMIT_RE,
    validate_report_payload,
    validate_schema_contract,
)
from .coverage_matrix_gaps import (
    CoverageMatrixGapProvenance,
    CoverageMatrixGapReport,
    load_repository_inputs,
)
from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    summary = payload.get("summary", {})
    digest = hashlib.sha256(json_bytes).hexdigest()
    markers = [
        f"Generated JSON SHA-256: `{digest}`",
        f"Input SHA-256: `{payload.get('input_digest')}`",
        f"Source revision: `{payload.get('commit')}`",
        f"Generated: `{payload.get('timestamp')}`",
        f"- Required family slots: {summary.get('required_family_slots')}",
        f"- Missing required slots: {summary.get('missing_required_slots')}",
    ]
    failures = [
        f"coverage-matrix gap Markdown is missing bound marker: {marker}"
        for marker in markers
        if marker not in markdown
    ]
    canonical_json = (json.dumps(dict(payload), indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical_json:
        failures.append("coverage-matrix gap JSON is not canonical")
    expected = _expected_markdown(payload, json_bytes)
    if expected is not None and markdown != expected:
        failures.append("coverage-matrix gap Markdown does not exactly match JSON")
    return failures


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    """Recompute current metadata and bind inputs, source, JSON, and Markdown."""

    root = root.resolve()
    failures: list[str] = []
    validator: Validator | None = None
    if root != METADATA_ROOT.resolve():
        failures.append("root does not identify the repository that loaded verification modules")
    else:
        validator = Validator()
        validator.load_records()
        validator.validate()
        failures.extend(
            f"metadata: {_display_path(root, failure.path)}: {failure.message}"
            for failure in validator.failures
        )

    schema_file = _absolute(root, schema_path)
    try:
        schema = json.loads(schema_file.read_text())
    except Exception as exc:
        schema = None
        failures.append(f"coverage-matrix gap schema cannot be read: {exc}")
    if isinstance(schema, dict):
        failures.extend(validate_schema_contract(schema))

    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
    except Exception as exc:
        return sorted(set([*failures, f"coverage-matrix gap JSON cannot be read: {exc}"]))
    failures.extend(validate_report_payload(payload))
    if isinstance(schema, dict):
        failures.extend(validate_json_schema_instance(payload, schema))
    if not isinstance(payload, dict):
        return sorted(set(failures))

    relative_json = _relative(root, json_file, "JSON output", failures)
    relative_markdown = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths")
    if isinstance(outputs, dict):
        if relative_json is not None and outputs.get("json") != relative_json:
            failures.append("output_paths.json does not identify the validated JSON file")
        if relative_markdown is not None and outputs.get("markdown") != relative_markdown:
            failures.append("output_paths.markdown does not identify the validated Markdown file")

    if validator is not None and not validator.failures:
        try:
            expected_analysis, expected_inputs = load_repository_inputs(root, validator)
        except (OSError, ValueError) as exc:
            failures.append(f"coverage-matrix gap analysis failed: {exc}")
        else:
            failures.extend(
                validate_report_payload(payload, expected_analysis=expected_analysis)
            )
            failures.extend(validate_input_binding(root, payload, expected_inputs))
            failures.extend(
                validate_source_binding(root, payload.get("commit"), expected_inputs)
            )

    try:
        markdown = markdown_file.read_text()
    except Exception as exc:
        return sorted(
            set([*failures, f"coverage-matrix gap Markdown cannot be read: {exc}"])
        )
    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    return sorted(set(failures))


def validate_input_binding(
    root: Path,
    payload: Mapping[str, Any],
    expected_inputs: list[str],
) -> list[str]:
    failures: list[str] = []
    if payload.get("input_paths") != expected_inputs:
        failures.append("input_paths do not match current metadata, tool, case, and schema inputs")
    failures.extend(validate_bound_input_paths(root, expected_inputs))
    if payload.get("input_digest") != input_digest(root, expected_inputs):
        failures.append("input_digest does not match current report inputs")
    return failures


def validate_source_binding(root: Path, value: Any, input_paths: list[str]) -> list[str]:
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(value, str) or not CLEAN_COMMIT_RE.fullmatch(value):
        return sorted(
            set([*failures, "commit must identify a clean full Git SHA for at-rest validation"])
        )
    commit = value
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in the repository: {commit}"]
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "-z", commit],
        check=False,
        capture_output=True,
    )
    if tree.returncode != 0:
        return [f"could not read source commit tree: {commit}"]
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append(f"source commit lacks report inputs: {', '.join(missing[:5])}")
    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", commit, "--", *input_paths],
        check=False,
    )
    if diff.returncode == 1:
        failures.append("current report inputs differ from the clean source commit")
    elif diff.returncode != 0:
        failures.append(f"could not compare report inputs with source commit: exit {diff.returncode}")
    return failures


def _expected_markdown(payload: Mapping[str, Any], json_bytes: bytes) -> str | None:
    try:
        report = CoverageMatrixGapReport(
            provenance=CoverageMatrixGapProvenance(
                command=tuple(payload["command"]),
                commit=payload["commit"],
                timestamp=payload["timestamp"],
                platform=payload["platform"],
                input_paths=tuple(payload["input_paths"]),
                output_json=payload["output_paths"]["json"],
                output_markdown=payload["output_paths"]["markdown"],
            ),
            input_digest=payload["input_digest"],
            analysis={field: payload[field] for field in ANALYSIS_FIELDS},
        )
    except (KeyError, TypeError):
        return None
    return report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(root: Path, path: Path, label: str, failures: list[str]) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        failures.append(f"{label} escapes the workspace")
        return None


def _display_path(root: Path, path: Path) -> str:
    if not path.is_absolute():
        return path.as_posix()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
