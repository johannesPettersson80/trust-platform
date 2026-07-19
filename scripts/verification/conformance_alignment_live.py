"""Live repository inputs and provenance for the Phase 7 alignment audit."""

from __future__ import annotations

import platform
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .conformance_alignment import (
    COMMS_REVIEWED_SOURCE_PATHS,
    REPORT_KEEP_PATH,
    RUNNER_REVIEWED_SOURCE_PATHS,
    analyze_conformance_alignment,
)
from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest


REPORT_SCHEMA_PATH = "verification/schemas/conformance-alignment-report.schema.json"
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
POLICY_PATH = "docs/internal/testing/checklists/plc-verification-program/policy.md"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_OPEN_ROWS = (
    "VERIF-P1A-003",
    "VERIF-P1A-006",
    "VERIF-P4A-005",
)
REPORT_CONTRACT_PATHS = {
    ".gitignore",
    ".github/workflows/ci.yml",
    POLICY_PATH,
    "docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-model.md",
    "docs/internal/testing/checklists/plc-verification-program/spec-matrix-model.md",
    "docs/public/reference/conformance.md",
    *RUNNER_REVIEWED_SOURCE_PATHS,
    *COMMS_REVIEWED_SOURCE_PATHS,
    "crates/trust-runtime/tests/conformance_cli_command.rs",
    "scripts/report_conformance_alignment.py",
    "scripts/validate_conformance_alignment_report.py",
    "scripts/verification/conformance_alignment.py",
    "scripts/verification/conformance_alignment_cli.py",
    "scripts/verification/conformance_alignment_contract.py",
    "scripts/verification/conformance_alignment_live.py",
    "scripts/verification/conformance_alignment_report.py",
    "scripts/verification/conformance_alignment_validation.py",
    "verification/README.md",
    REPORT_SCHEMA_PATH,
    "verification/schemas/catalog.schema.json",
    "verification/schemas/spec-source.schema.json",
    "verification/spec-sources.toml",
    "verification/test-catalog.toml",
}


@dataclass(frozen=True)
class LiveConformanceAlignmentState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    analysis: dict[str, Any]


def build_live_conformance_alignment_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LiveConformanceAlignmentState:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("root does not identify the repository that loaded verification modules")
    board_failures = validate_open_board_rows((root / BOARD_PATH).read_text())
    if board_failures:
        raise ValueError("; ".join(board_failures))
    policy = (root / POLICY_PATH).read_text()
    if not re.search(r"^- \[ \] `VERIF-STOP-014`(?:\s|$)", policy, re.MULTILINE):
        raise ValueError("VERIF-STOP-014 must remain open for conformance mapping")

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
    tracked_reports = tuple(_git_lines(root, "ls-files", "conformance/reports"))
    analysis = analyze_conformance_alignment(
        root,
        tests=validator.tests,
        spec_sources=validator.spec_sources,
        tracked_report_paths=tracked_reports,
    )

    paths = set(REPORT_CONTRACT_PATHS) | validator_code_input_paths(root)
    paths.update(_conformance_input_paths(root))
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
    return LiveConformanceAlignmentState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        analysis=analysis,
    )


def validate_open_board_rows(board: str) -> list[str]:
    failures: list[str] = []
    for row_id in REQUIRED_OPEN_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", board, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 7 alignment audit")
    return failures


def _conformance_input_paths(root: Path) -> set[str]:
    paths: set[str] = set()
    for path in (root / "conformance").rglob("*"):
        if not (path.is_file() or path.is_symlink()):
            continue
        relative = path.relative_to(root).as_posix()
        if relative.startswith("conformance/reports/") and relative != REPORT_KEEP_PATH:
            continue
        paths.add(relative)
    return paths


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


def _git_lines(root: Path, *args: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ValueError(f"git {' '.join(args)} failed with exit {result.returncode}")
    return sorted(line for line in result.stdout.splitlines() if line)


def _head_commit(root: Path) -> str:
    lines = _git_lines(root, "rev-parse", "HEAD")
    if len(lines) != 1 or not COMMIT_RE.fullmatch(lines[0]):
        raise ValueError("source commit must identify a clean full Git SHA")
    return lines[0]


def _display_path(root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        return path.as_posix()
