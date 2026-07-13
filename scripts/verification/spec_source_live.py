"""Live repository state and at-rest validation for source-audit reports."""

from __future__ import annotations

import json
import platform
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .spec_source_analysis import analyze_spec_sources
from .spec_source_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from .spec_source_scanner import discover_spec_documents
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


REPORT_SCHEMA_PATH = "verification/schemas/spec-source-audit-report.schema.json"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REPORT_CONTRACT_PATHS = {
    "docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-model.md",
    "docs/internal/testing/checklists/plc-verification-program/spec-matrix-model.md",
    "scripts/report_spec_source_audit.py",
    "scripts/validate_spec_source_audit_report.py",
    "scripts/verification/spec_source_analysis.py",
    "scripts/verification/spec_source_cli.py",
    "scripts/verification/spec_source_contract.py",
    "scripts/verification/spec_source_live.py",
    "scripts/verification/spec_source_markdown.py",
    "scripts/verification/spec_source_models.py",
    "scripts/verification/spec_source_report.py",
    "scripts/verification/spec_source_scanner.py",
    "scripts/verification/spec_source_scope.py",
    "verification/README.md",
    REPORT_SCHEMA_PATH,
    "verification/schemas/spec-gap.schema.json",
    "verification/schemas/spec-source.schema.json",
    "verification/spec-gaps.toml",
    "verification/spec-matrix.toml",
    "verification/spec-sources.toml",
}


@dataclass(frozen=True)
class LiveSpecSourceAuditState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    analysis: dict[str, Any]


def build_live_spec_source_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LiveSpecSourceAuditState:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("root does not identify the repository that loaded verification modules")
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "; ".join(
                f"{_display_path(root, failure.path)}: {failure.message}"
                for failure in validator.failures
            )
        )
    scan = discover_spec_documents(root)
    scanner_errors = [
        diagnostic
        for diagnostic in scan.diagnostics
        if getattr(diagnostic, "severity", None) == "error"
    ]
    if scanner_errors:
        raise ValueError(
            "; ".join(
                f"{item.path}:{item.line}: {item.kind}: {item.message}"
                for item in scanner_errors
            )
        )
    analysis = analyze_spec_sources(
        root,
        scan=scan,
        spec_sources=validator.spec_sources,
        required_specs=validator.required_specs,
        spec_gaps=validator.spec_gaps,
    )
    if analysis["summary"]["blocking_findings"]:
        blocking = [row for row in analysis["findings"] if row["severity"] == "error"]
        raise ValueError(
            "; ".join(
                f"{row['path']}: {row['code']}: {row['message']}" for row in blocking
            )
        )

    paths = set(REPORT_CONTRACT_PATHS) | validator_code_input_paths(root)
    paths.update(scan.input_paths)
    for source in validator.spec_sources.values():
        if source.get("locator_kind") == "tracked_file" and isinstance(source.get("path"), str):
            paths.add(source["path"])
    input_paths = tuple(sorted(paths))
    failures = validate_bound_input_paths(root, input_paths)
    if failures:
        raise ValueError("; ".join(failures))
    commit = _head_commit(root)
    if require_clean_commit:
        status = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
            check=False,
            capture_output=True,
        )
        if status.returncode != 0 or status.stdout:
            raise ValueError("source commit must identify a clean full Git SHA")
    report_timestamp = timestamp or datetime.now(timezone.utc).isoformat(timespec="seconds")
    return LiveSpecSourceAuditState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        analysis=analysis,
    )


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path | None = None,
) -> list[str]:
    root = root.resolve()
    failures: list[str] = []
    try:
        json_file = _workspace_path(root, json_path, must_exist=True)
        markdown_file = _workspace_path(root, markdown_path, must_exist=True)
        expected_schema = (root / REPORT_SCHEMA_PATH).resolve()
        schema_file = _workspace_path(
            root,
            schema_path or Path(REPORT_SCHEMA_PATH),
            must_exist=True,
        )
        if schema_file != expected_schema:
            failures.append("schema path does not identify the committed source-audit schema")
        json_bytes = json_file.read_bytes()
        markdown = markdown_file.read_text()
        payload = json.loads(json_bytes)
        schema = json.loads(schema_file.read_text())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        return [f"could not load specification-source report artifacts: {exc}"]
    if not isinstance(payload, dict):
        return ["specification-source report JSON root must be an object"]
    if not isinstance(schema, dict):
        return ["specification-source report schema root must be an object"]

    try:
        failures.extend(validate_schema_contract(schema))
        failures.extend(validate_json_schema_instance(payload, schema))
        failures.extend(validate_report_payload(payload))
        failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    except (AttributeError, KeyError, TypeError, ValueError) as exc:
        failures.append(
            "specification-source artifact validation rejected malformed field types: "
            f"{type(exc).__name__}: {exc}"
        )
    try:
        state = build_live_spec_source_state(root, timestamp=None, require_clean_commit=False)
    except (OSError, ValueError) as exc:
        failures.append(f"could not rebuild live specification-source analysis: {exc}")
        return sorted(set(failures))
    try:
        failures.extend(validate_report_payload(payload, expected_analysis=state.analysis))
    except (AttributeError, KeyError, TypeError, ValueError) as exc:
        failures.append(
            "specification-source live recomputation rejected malformed field types: "
            f"{type(exc).__name__}: {exc}"
        )
    reported_inputs = payload.get("input_paths")
    if reported_inputs != list(state.input_paths):
        failures.append("input_paths do not match the complete live source-audit closure")
    if payload.get("input_digest") != state.input_digest:
        failures.append("input_digest does not match current source-audit inputs")
    failures.extend(
        validate_source_revision(
            root,
            payload.get("commit"),
            tuple(reported_inputs) if isinstance(reported_inputs, list) else (),
        )
    )
    outputs = payload.get("output_paths")
    if isinstance(outputs, dict):
        if outputs.get("json") != json_file.relative_to(root).as_posix():
            failures.append("JSON output path does not match the validated artifact")
        if outputs.get("markdown") != markdown_file.relative_to(root).as_posix():
            failures.append("Markdown output path does not match the validated artifact")
    return sorted(set(failures))


def validate_source_revision(
    root: Path,
    commit: object,
    input_paths: tuple[str, ...],
) -> list[str]:
    root = root.resolve()
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        return sorted(set([*failures, "commit must identify a clean full Git SHA"]))
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in repository: {commit}"]
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "-z", commit],
        check=False,
        capture_output=True,
    )
    if tree.returncode != 0:
        return [f"could not inspect source commit: {commit}"]
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append("source commit lacks report inputs: " + ", ".join(missing[:8]))
    changed = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", commit, "--", *input_paths],
        check=False,
        capture_output=True,
    )
    if changed.returncode == 1:
        failures.append("report inputs differ from the claimed source commit")
    elif changed.returncode != 0:
        failures.append(f"could not compare report inputs with source commit: exit {changed.returncode}")
    return sorted(set(failures))


def _workspace_path(root: Path, path: Path, *, must_exist: bool) -> Path:
    candidate = path if path.is_absolute() else root / path
    try:
        lexical = candidate.absolute().relative_to(root)
    except ValueError as exc:
        raise ValueError(f"artifact path escapes workspace: {path}") from exc
    probe = root
    for part in lexical.parts:
        probe /= part
        if probe.is_symlink():
            raise ValueError(f"artifact path contains a symlink component: {path}")
    resolved = candidate.resolve(strict=must_exist)
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"artifact path escapes workspace: {path}") from exc
    if must_exist and not resolved.is_file():
        raise ValueError(f"artifact path is not a regular file: {path}")
    return resolved


def _head_commit(root: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    commit = result.stdout.strip()
    if result.returncode != 0 or not COMMIT_RE.fullmatch(commit):
        raise ValueError("source commit must identify a clean full Git SHA")
    return commit


def _display_path(root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        return path.as_posix()
