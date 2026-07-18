"""Live repository inputs for the Phase 12 workflow and UI audit."""

from __future__ import annotations

import platform
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .metadata_validator.core import Validator
from .phase12_audit import build_rows
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest


SCHEMA_PATH = "verification/schemas/phase12-workflow-ui-audit-report.schema.json"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
STATIC_INPUTS = {
    "scripts/report_phase12_workflow_ui_audit.py",
    "scripts/validate_phase12_workflow_ui_audit_report.py",
    "scripts/verification/phase12_audit.py",
    "scripts/verification/phase12_audit_cli.py",
    "scripts/verification/phase12_audit_live.py",
    "scripts/verification/phase12_audit_validation.py",
    "scripts/verification/public_workflow_inventory.py",
    "scripts/verification/ui_acceptance.py",
    "verification/public-workflow-inventory.toml",
    "verification/ui-acceptance.toml",
    "verification/spec-sources.toml",
    "verification/test-catalog.toml",
    SCHEMA_PATH,
}


@dataclass(frozen=True)
class LivePhase12State:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    workflow_rows: tuple[dict[str, Any], ...]
    journey_rows: tuple[dict[str, Any], ...]


def build_live_phase12_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LivePhase12State:
    root = root.resolve()
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "metadata validation failed: "
            + "; ".join(failure.message for failure in validator.failures[:10])
        )
    reviews = validator.public_workflow_inventory.get("reviews", [])
    journeys = validator.ui_acceptance.get("journeys", [])
    workflow_rows, journey_rows = build_rows(reviews, journeys)
    if len(workflow_rows) != 47 or len(journey_rows) != 30:
        raise ValueError(
            f"Phase 12 denominator drift: {len(workflow_rows)} workflows, {len(journey_rows)} journeys"
        )
    input_paths = _input_paths(root, validator)
    failures = validate_bound_input_paths(root, input_paths)
    if failures:
        raise ValueError("; ".join(failures))
    commit = _head_commit(root)
    if require_clean_commit:
        dirty = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=all"],
            cwd=root,
            check=False,
            capture_output=True,
        )
        if dirty.returncode != 0 or dirty.stdout:
            raise ValueError("source commit must identify a clean full Git SHA")
    return LivePhase12State(
        commit=commit,
        timestamp=timestamp or datetime.now(timezone.utc).isoformat(timespec="seconds"),
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        workflow_rows=tuple(workflow_rows),
        journey_rows=tuple(journey_rows),
    )


def validate_source_revision(
    root: Path, commit: object, input_paths: tuple[str, ...]
) -> list[str]:
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        return [*failures, "commit must identify a clean full Git SHA"]
    if subprocess.run(
        ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
        cwd=root,
        check=False,
        capture_output=True,
    ).returncode:
        return [*failures, f"commit does not resolve in repository: {commit}"]
    tree = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", "-z", commit],
        cwd=root,
        check=False,
        capture_output=True,
    )
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append("source commit lacks report inputs: " + ", ".join(missing))
    if subprocess.run(
        ["git", "diff", "--quiet", commit, "--", *input_paths],
        cwd=root,
        check=False,
        capture_output=True,
    ).returncode:
        failures.append("report inputs differ from the claimed source commit")
    return sorted(set(failures))


def _input_paths(root: Path, validator: Validator) -> tuple[str, ...]:
    paths = set(STATIC_INPUTS) | validator_code_input_paths(root)
    paths.update(
        path.relative_to(root).as_posix()
        for path in (root / "docs/public").rglob("*.md")
        if path.is_file() and not path.is_symlink()
    )
    paths.update(
        record["_path"].relative_to(root).as_posix()
        for record in validator.invariants.values()
        if record["id"]
        in {
            invariant_id
            for journey in validator.ui_acceptance.get("journeys", [])
            for invariant_id in journey.get("invariant_ids", [])
        }
    )
    for journey in validator.ui_acceptance.get("journeys", []):
        paths.update(journey.get("runner_paths", []))
        for evidence in journey.get("evidence", []):
            paths.add(evidence["screenshot_path"])
            paths.add(evidence["result_path"])
    paths.add(str(validator.ui_acceptance["batch_runner_path"]))
    return tuple(sorted(paths))


def _head_commit(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, check=False, capture_output=True, text=True
    )
    commit = result.stdout.strip()
    if result.returncode or not COMMIT_RE.fullmatch(commit):
        raise ValueError("source commit must identify a clean full Git SHA")
    return commit
