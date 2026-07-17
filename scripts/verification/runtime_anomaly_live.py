"""Live repository inputs and provenance for the Phase 8 anomaly audit."""

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
from .runtime_anomaly_contract import (
    TAXONOMY_PATH,
    TAXONOMY_SCHEMA_PATH,
    load_runtime_anomaly_taxonomy,
    validate_runtime_anomaly_contract,
)
from .runtime_anomaly_mapping import analyze_runtime_anomaly_mapping
from .test_catalog_common import input_digest
from .test_catalog_rust import scan_rust_tests


REPORT_SCHEMA_PATH = "verification/schemas/runtime-anomaly-audit-report.schema.json"
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
POLICY_PATH = "docs/internal/testing/checklists/plc-verification-program/policy.md"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_OPEN_ROWS = (
    "VERIF-P1A-003",
    "VERIF-P1A-006",
    "VERIF-P1B-012",
    "VERIF-P1B-014",
    "VERIF-P3-006",
    "VERIF-P4A-005",
    "VERIF-P5-000B",
    "VERIF-P6-007",
    "VERIF-P6-008",
    "VERIF-P6-009",
    "VERIF-P6-010",
    "VERIF-P8-002",
    "VERIF-P8-005",
    "VERIF-P8-006",
    "VERIF-P14-000",
)
REQUIRED_OPEN_POLICY_ROWS = ("VERIF-STOP-012", "VERIF-STOP-014")
REPORT_CONTRACT_PATHS = {
    POLICY_PATH,
    "docs/internal/testing/checklists/plc-verification-program/README.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-model.md",
    "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
    "docs/internal/testing/checklists/plc-verification-program/verification-areas.md",
    "docs/specs/11-runtime-engine.md",
    "scripts/report_runtime_anomaly_audit.py",
    "scripts/validate_runtime_anomaly_audit_report.py",
    "scripts/verification/runtime_anomaly_cli.py",
    "scripts/verification/runtime_anomaly_contract.py",
    "scripts/verification/runtime_anomaly_live.py",
    "scripts/verification/runtime_anomaly_mapping.py",
    "scripts/verification/runtime_anomaly_report.py",
    "scripts/verification/runtime_anomaly_report_contract.py",
    "scripts/verification/runtime_anomaly_validation.py",
    "verification/README.md",
    TAXONOMY_PATH,
    TAXONOMY_SCHEMA_PATH,
    REPORT_SCHEMA_PATH,
    "verification/ignored-tests.toml",
    "verification/schemas/ignored-test.schema.json",
    "verification/schemas/spec-gap.schema.json",
    "verification/schemas/spec-source.schema.json",
    "verification/schemas/suite.schema.json",
    "verification/spec-gaps.toml",
    "verification/spec-sources.toml",
}


@dataclass(frozen=True)
class LiveRuntimeAnomalyState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    spec_gap_reviews: dict[str, Any]
    analysis: dict[str, Any]


def build_live_runtime_anomaly_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LiveRuntimeAnomalyState:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("root does not identify the repository that loaded verification modules")
    board_failures = validate_open_board_rows((root / BOARD_PATH).read_text())
    if board_failures:
        raise ValueError("; ".join(board_failures))
    policy_failures = validate_open_policy_rows((root / POLICY_PATH).read_text())
    if policy_failures:
        raise ValueError("; ".join(policy_failures))

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
    taxonomy = load_runtime_anomaly_taxonomy(root)
    contract_failures = validate_runtime_anomaly_contract(
        root,
        taxonomy,
        spec_sources=validator.spec_sources,
        spec_gaps=validator.spec_gaps,
    )
    if contract_failures:
        raise ValueError("; ".join(contract_failures))
    scan = scan_rust_tests(root)
    if scan.diagnostics:
        raise ValueError(
            "runtime-anomaly Rust scan produced diagnostics: "
            + "; ".join(
                f"{item.path}:{item.line} {item.kind}: {item.message}"
                for item in scan.diagnostics
            )
        )
    analysis = analyze_runtime_anomaly_mapping(
        taxonomy=taxonomy,
        facts=scan.facts,
        ignored_tests=validator.ignored_tests,
        scanner_denominator=len(scan.facts),
    )

    paths = set(REPORT_CONTRACT_PATHS) | validator_code_input_paths(root)
    restart_review = taxonomy["spec_gap_reviews"]["restart_timebase"]
    if restart_review.get("outcome") == "resolved_source":
        paths.add(restart_review["source_path"])
    paths.update(scan.input_paths)
    for suite in validator.suites.values():
        path = suite.get("_path")
        if isinstance(path, Path):
            paths.add(path.resolve().relative_to(root).as_posix())
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
    return LiveRuntimeAnomalyState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        spec_gap_reviews={
            key: dict(value) for key, value in taxonomy["spec_gap_reviews"].items()
        },
        analysis=analysis,
    )


def validate_open_board_rows(board: str) -> list[str]:
    failures: list[str] = []
    for row_id in REQUIRED_OPEN_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", board, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 8 anomaly audit")
    return failures


def validate_open_policy_rows(policy: str) -> list[str]:
    failures: list[str] = []
    for row_id in REQUIRED_OPEN_POLICY_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", policy, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 8 anomaly audit")
    return failures


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
