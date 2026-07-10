"""Live input loading and at-rest proof for specification-completeness reports."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .metadata_validator.spec_gap_closure import validate_spec_gap_closure
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .spec_completeness_contract import (
    CLEAN_COMMIT_RE,
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from .spec_completeness_report import REPORT_CONTRACT_PATHS, analyze_spec_completeness
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


def load_repository_inputs(
    root: Path,
    validator: Validator,
) -> tuple[dict[str, Any], list[str]]:
    """Build current analysis and the exact semantic/provenance input closure."""

    root = root.resolve()
    closure_failures = validate_spec_gap_closure(
        root=root,
        spec_gaps=validator.spec_gaps,
        spec_sources=validator.spec_sources,
        tests=validator.tests,
        evidence=validator.evidence,
        invariants=validator.invariants,
        required_specs=validator.required_specs,
        risks=validator.risks,
    )
    if closure_failures:
        raise ValueError("; ".join(closure_failures))
    analysis = analyze_spec_completeness(
        invariants=validator.invariants,
        tests=validator.tests,
        ignored_tests=validator.ignored_tests,
        spec_gaps=validator.spec_gaps,
        spec_sources=validator.spec_sources,
        matrix=validator.matrix,
    )
    paths = set(REPORT_CONTRACT_PATHS) | validator_code_input_paths(root)
    for record in validator.invariants.values():
        path = record.get("_path")
        if isinstance(path, Path):
            paths.add(path.relative_to(root).as_posix())
    for source in validator.spec_sources.values():
        path = source.get("path")
        if isinstance(path, str):
            paths.add(path)
    for path in (root / "verification/suites").glob("*.toml"):
        paths.add(path.relative_to(root).as_posix())
    input_paths = sorted(paths)
    failures = validate_bound_input_paths(root, input_paths)
    if failures:
        raise ValueError("; ".join(failures))
    return analysis, input_paths


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    """Recompute current state and bind source, payload, schema, and Markdown."""

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
        failures.append(f"spec-completeness schema cannot be read: {exc}")
    if isinstance(schema, dict):
        failures.extend(validate_schema_contract(schema))

    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
    except Exception as exc:
        return sorted(set([*failures, f"spec-completeness JSON cannot be read: {exc}"]))
    failures.extend(validate_report_payload(payload))
    if isinstance(schema, dict):
        failures.extend(validate_json_schema_instance(payload, schema))
    if not isinstance(payload, dict):
        return sorted(set(failures))

    relative_json = _relative(root, json_file, "JSON output", failures)
    relative_markdown = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths")
    if isinstance(outputs, Mapping):
        if relative_json is not None and outputs.get("json") != relative_json:
            failures.append("output_paths.json does not identify the validated JSON file")
        if relative_markdown is not None and outputs.get("markdown") != relative_markdown:
            failures.append("output_paths.markdown does not identify the validated Markdown file")

    if validator is not None and not validator.failures:
        try:
            expected_analysis, expected_inputs = load_repository_inputs(root, validator)
        except (OSError, ValueError) as exc:
            failures.append(f"spec-completeness analysis failed: {exc}")
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
        return sorted(set([*failures, f"spec-completeness Markdown cannot be read: {exc}"]))
    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    return sorted(set(failures))


def validate_input_binding(
    root: Path,
    payload: Mapping[str, Any],
    expected_inputs: list[str],
) -> list[str]:
    failures: list[str] = []
    if payload.get("input_paths") != expected_inputs:
        failures.append("input_paths do not match current specification-completeness inputs")
    failures.extend(validate_bound_input_paths(root, expected_inputs))
    if payload.get("input_digest") != input_digest(root, expected_inputs):
        failures.append("input_digest does not match current specification-completeness inputs")
    return failures


def validate_source_binding(
    root: Path,
    value: Any,
    input_paths: list[str],
) -> list[str]:
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(value, str) or not CLEAN_COMMIT_RE.fullmatch(value):
        return sorted(set([*failures, "commit must identify a clean full Git SHA for at-rest validation"]))
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
        failures.append("current specification-completeness inputs differ from source commit")
    elif diff.returncode != 0:
        failures.append(
            f"could not compare specification-completeness inputs with source commit: exit {diff.returncode}"
        )
    return sorted(set(failures))


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(
    root: Path,
    path: Path,
    label: str,
    failures: list[str],
) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        failures.append(f"{label} escapes workspace")
        return None


def _display_path(root: Path, path: Path) -> str:
    if not path.is_absolute():
        return path.as_posix()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
