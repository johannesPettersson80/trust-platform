"""Live repository state and provenance for the Phase 4 invariant-seed audit."""

from __future__ import annotations

import platform
import re
import subprocess
import tomllib
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from .invariant_seed_contract import (
    AREAS_PATH,
    MANIFEST_PATH,
    MANIFEST_SCHEMA_PATH,
    SeedAudit,
    load_seed_audit,
    required_review_source_paths,
)
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest


REPORT_SCHEMA_PATH = "verification/schemas/invariant-seed-audit-report.schema.json"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REPORT_CONTRACT_PATHS = {
    "scripts/report_invariant_seed_audit.py",
    "scripts/validate_invariant_seed_audit_report.py",
    "scripts/verification/invariant_seed_cli.py",
    "scripts/verification/invariant_seed_contract.py",
    "scripts/verification/invariant_seed_live.py",
    "scripts/verification/invariant_seed_report.py",
    "scripts/verification/invariant_seed_validation.py",
    "scripts/verification/report_input_contract.py",
    "scripts/verification/test_catalog_common.py",
    "scripts/verification/test_catalog_json_schema.py",
    "scripts/verification/test_catalog_validation.py",
    REPORT_SCHEMA_PATH,
    MANIFEST_SCHEMA_PATH,
}
METADATA_PATHS = {
    AREAS_PATH,
    MANIFEST_PATH,
    "verification/risk-register.toml",
    "verification/spec-gaps.toml",
    "verification/spec-sources.toml",
    "verification/test-catalog.toml",
}


@dataclass(frozen=True)
class LiveSeedAuditState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    audit: SeedAudit


def build_live_seed_audit_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LiveSeedAuditState:
    root = root.resolve()
    audit = load_seed_audit(root)
    sources = _index(root / "verification/spec-sources.toml", "spec_sources")
    risks = _index(root / "verification/risk-register.toml", "risks")
    source_paths = required_review_source_paths(audit, sources, risks)
    invariant_paths = {row.invariant_path for row in audit.rows}
    input_paths = tuple(
        sorted({*REPORT_CONTRACT_PATHS, *METADATA_PATHS, *source_paths, *invariant_paths})
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
    return LiveSeedAuditState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        audit=audit,
    )


def validate_source_revision(root: Path, commit: object, input_paths: tuple[str, ...]) -> list[str]:
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
        failures.append("source commit lacks report inputs: " + ", ".join(missing))
    changed = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", commit, "--", *input_paths],
        check=False,
        capture_output=True,
    )
    if changed.returncode != 0:
        failures.append("report inputs differ from the claimed source commit")
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


def _index(path: Path, key: str) -> dict[str, dict[str, object]]:
    data = tomllib.loads(path.read_text())
    return {row["id"]: row for row in data.get(key, []) if isinstance(row, dict)}
