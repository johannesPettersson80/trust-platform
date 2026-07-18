"""Live repository inputs and provenance for the Phase 9 fuzz-program audit."""

from __future__ import annotations

import platform
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .fuzz_program_analysis import analyze_fuzz_program
from .fuzz_program_contract import (
    FUZZ_PROGRAM_PATH,
    FUZZ_PROGRAM_SCHEMA_PATH,
    load_fuzz_program,
    validate_fuzz_program_contract,
)
from .fuzz_program_discovery import scan_cargo_fuzz_targets, scan_fuzz_like_tests
from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest


REPORT_SCHEMA_PATH = "verification/schemas/fuzz-program-audit-report.schema.json"
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
POLICY_PATH = "docs/internal/testing/checklists/plc-verification-program/policy.md"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
TIMESTAMP_RE = re.compile(
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})$"
)
REQUIRED_OPEN_ROWS = (
    "VERIF-P1A-003",
    "VERIF-P1A-006",
    "VERIF-P3-006",
    "VERIF-P4A-005",
    "VERIF-P5-000B",
    "VERIF-P6-007",
    "VERIF-P6-008",
    "VERIF-P6-009",
    "VERIF-P6-010",
    "VERIF-P8-005",
    "VERIF-P8-006",
    "VERIF-P14-000",
)
REQUIRED_OPEN_POLICY_ROWS = ("VERIF-STOP-014",)
REPORT_CONTRACT_PATHS = {
    ".github/workflows/ci.yml",
    ".github/workflows/salsa-hardening.yml",
    POLICY_PATH,
    "crates/trust-ads-server/fuzz/.gitignore",
    "docs/internal/testing/checklists/plc-verification-program/README.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-model.md",
    "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
    "fuzz/.gitignore",
    "scripts/report_fuzz_program_audit.py",
    "scripts/runtime_comms_fuzz_gate.sh",
    "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh",
    "scripts/salsa_fuzz_gate.sh",
    "scripts/validate_fuzz_program_audit_report.py",
    "verification/README.md",
    "verification/fuzz-crash-regressions.toml",
    "docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-fuzz-campaign.json",
    FUZZ_PROGRAM_PATH,
    FUZZ_PROGRAM_SCHEMA_PATH,
    REPORT_SCHEMA_PATH,
    "verification/gate-inventory.toml",
    "verification/schemas/gate-inventory.schema.json",
    "verification/suites/nightly.toml",
    "verification/suites/pr.toml",
}


@dataclass(frozen=True)
class LiveFuzzProgramState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    corpus_policy: dict[str, Any]
    crash_regression_handoff: dict[str, Any]
    analysis: dict[str, Any]


def build_live_fuzz_program_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = True,
) -> LiveFuzzProgramState:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("full metadata validation requires the repository root")
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "metadata validation failed: "
            + "; ".join(f"{item.path}: {item.message}" for item in validator.failures)
        )
    program = load_fuzz_program(root)
    failures = validate_fuzz_program_contract(root, program)
    if failures:
        raise ValueError("; ".join(failures))
    cargo_scan = scan_cargo_fuzz_targets(root)
    smoke_scan = scan_fuzz_like_tests(root)
    analysis, failures = analyze_fuzz_program(program, cargo_scan, smoke_scan)
    if failures:
        raise ValueError("; ".join(failures))
    board = (root / BOARD_PATH).read_text()
    policy = (root / POLICY_PATH).read_text()
    failures = validate_open_board_rows(board)
    failures.extend(validate_open_policy_rows(policy))
    if failures:
        raise ValueError("; ".join(failures))
    input_paths = tuple(
        sorted(
            set(REPORT_CONTRACT_PATHS)
            | validator_code_input_paths(root)
            | cargo_scan.input_paths
            | smoke_scan.input_paths
        )
    )
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
    validate_timestamp(report_timestamp)
    return LiveFuzzProgramState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        corpus_policy=dict(program["corpus_policy"]),
        crash_regression_handoff=dict(program["crash_regression_handoff"]),
        analysis=analysis,
    )


def validate_open_board_rows(board: str) -> list[str]:
    failures = []
    for row_id in REQUIRED_OPEN_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", board, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 9 fuzz audit")
    return failures


def validate_open_policy_rows(policy: str) -> list[str]:
    failures = []
    for row_id in REQUIRED_OPEN_POLICY_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", policy, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 9 fuzz audit")
    return failures


def validate_timestamp(value: object) -> None:
    if not isinstance(value, str) or not TIMESTAMP_RE.fullmatch(value):
        raise ValueError("timestamp must be ISO-8601 with a timezone")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError("timestamp must be ISO-8601 with a timezone") from exc
    if parsed.tzinfo is None:
        raise ValueError("timestamp must be ISO-8601 with a timezone")


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
