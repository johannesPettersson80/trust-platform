"""Live repository inputs and source binding for the Phase 6 oracle audit."""

from __future__ import annotations

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
from .requirement_oracle_mapping import analyze_requirement_oracles
from .test_catalog_common import input_digest


REPORT_SCHEMA_PATH = "verification/schemas/requirement-oracle-audit-report.schema.json"
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_OPEN_ROWS = (
    "VERIF-P1A-002",
    "VERIF-P1A-003",
    "VERIF-P1A-006",
    "VERIF-P1A-007",
    "VERIF-P1B-012",
    "VERIF-P1B-014",
    "VERIF-P3-006",
    "VERIF-P4A-005",
    "VERIF-P5-000B",
    "VERIF-P6-007",
    "VERIF-P6-008",
    "VERIF-P6-009",
    "VERIF-P6-010",
    "VERIF-P14-000",
)
REPORT_CONTRACT_PATHS = {
    "docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md",
    "scripts/report_requirement_oracle_audit.py",
    "scripts/validate_requirement_oracle_audit_report.py",
    "scripts/verification/requirement_oracle_cli.py",
    "scripts/verification/requirement_oracle_contract.py",
    "scripts/verification/requirement_oracle_live.py",
    "scripts/verification/requirement_oracle_mapping.py",
    "scripts/verification/requirement_oracle_report.py",
    "scripts/verification/requirement_oracle_validation.py",
    "verification/README.md",
    REPORT_SCHEMA_PATH,
    "verification/schemas/invariant.schema.json",
    "verification/schemas/spec-gap.schema.json",
    "verification/schemas/spec-source.schema.json",
    "verification/spec-gaps.toml",
    "verification/spec-sources.toml",
}


@dataclass(frozen=True)
class LiveRequirementOracleState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    analysis: dict[str, Any]


def build_live_requirement_oracle_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LiveRequirementOracleState:
    """Load validated metadata and derive the complete report input closure."""

    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("root does not identify the repository that loaded verification modules")
    board_failures = validate_open_board_rows((root / BOARD_PATH).read_text())
    if board_failures:
        raise ValueError("; ".join(board_failures))
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
    analysis = analyze_requirement_oracles(
        invariants=validator.invariants,
        spec_sources=validator.spec_sources,
        spec_gaps=validator.spec_gaps,
    )
    paths = set(REPORT_CONTRACT_PATHS) | validator_code_input_paths(root)
    for invariant in validator.invariants.values():
        path = invariant.get("_path")
        if isinstance(path, Path):
            paths.add(path.resolve().relative_to(root).as_posix())
    for source in validator.spec_sources.values():
        path = source.get("path")
        if isinstance(path, str):
            paths.add(path)
    input_paths = tuple(sorted(paths))
    failures = validate_bound_input_paths(root, input_paths)
    if failures:
        raise ValueError("; ".join(failures))
    commit = _head_commit(root)
    if require_clean_commit:
        dirty = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
            check=False,
            capture_output=True,
        )
        if dirty.returncode != 0 or dirty.stdout:
            raise ValueError("source commit must identify a clean full Git SHA")
    report_timestamp = timestamp or datetime.now(timezone.utc).isoformat(timespec="seconds")
    return LiveRequirementOracleState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        analysis=analysis,
    )


def validate_open_board_rows(board: str) -> list[str]:
    """Keep enforcement and incomplete traceability rows open for this report."""

    failures: list[str] = []
    for row_id in REQUIRED_OPEN_ROWS:
        pattern = re.compile(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", re.MULTILINE)
        if not pattern.search(board):
            failures.append(f"{row_id} must remain open for the Phase 6 oracle audit")
    return failures


def validate_source_revision(
    root: Path,
    commit: object,
    input_paths: tuple[str, ...],
) -> list[str]:
    """Bind all report inputs to a clean, resolvable source revision."""

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
        failures.append(
            f"could not compare report inputs with source commit: exit {changed.returncode}"
        )
    return sorted(set(failures))


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
