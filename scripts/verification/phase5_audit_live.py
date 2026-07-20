"""Live repository state and provenance for the combined Phase 5 audit."""

from __future__ import annotations

import platform
import re
import subprocess
import tomllib
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .area_routing import validate_area_routing
from .gate_inventory import load_gate_inventory, validate_gate_inventory
from .metadata_validator.constants import AREAS
from .metadata_validator.suites import validate_suite_records
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest


REPORT_SCHEMA_PATH = "verification/schemas/phase5-suite-audit-report.schema.json"
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
TAXONOMY_PATH = "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
P5_000B_CLOSED_RE = re.compile(r"^- \[x\] `VERIF-P5-000B`", re.MULTILINE)
REPORT_CONTRACT_PATHS = {
    "scripts/report_phase5_suite_audit.py",
    "scripts/validate_phase5_suite_audit_report.py",
    "scripts/verification/area_routing.py",
    "scripts/verification/focused_test_suite.py",
    "scripts/verification/gate_inventory.py",
    "scripts/verification/metadata_validator/constants.py",
    "scripts/verification/metadata_validator/suites.py",
    "scripts/verification/planner.py",
    "scripts/verification/phase5_audit_cli.py",
    "scripts/verification/phase5_audit_live.py",
    "scripts/verification/phase5_audit_report.py",
    "scripts/verification/phase5_audit_validation.py",
    "scripts/verification/report_input_contract.py",
    "scripts/verification/report_gate.py",
    "scripts/verification/test_catalog_common.py",
    "scripts/verification/test_catalog_json_schema.py",
    "scripts/verification/test_catalog_models.py",
    "scripts/verification/test_catalog_surfaces.py",
    "scripts/verification/test_catalog_validation.py",
    "scripts/run_verification_focused_tests.py",
    "scripts/plan_tests.py",
    "scripts/cargo_test_fast_link.sh",
    "scripts/verification_metadata_gate.sh",
    "scripts/verification_report_gate.py",
    "crates/trust-runtime/tests/device_in_the_loop.rs",
    REPORT_SCHEMA_PATH,
    "verification/gate-inventory.toml",
    "verification/test-catalog.toml",
    "verification/matrix.toml",
    "verification/schemas/gate-inventory.schema.json",
    "verification/schemas/matrix.schema.json",
    "verification/schemas/suite.schema.json",
    TAXONOMY_PATH,
}


@dataclass(frozen=True)
class LivePhase5AuditState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    inventory_rows: tuple[dict[str, Any], ...]
    suite_rows: tuple[dict[str, Any], ...]
    area_rows: tuple[dict[str, Any], ...]
    route_rows: tuple[dict[str, Any], ...]
    boundaries: dict[str, bool]


def build_live_phase5_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LivePhase5AuditState:
    root = root.resolve()
    failures = validate_gate_inventory(root)
    inventory = load_gate_inventory(root)
    suites = _load_suites(root)
    validate_suite_records(
        fail=lambda path, message: failures.append(f"{path}: {message}"),
        suites=suites,
        inventory=inventory,
    )
    matrix = tomllib.loads((root / "verification/matrix.toml").read_text())
    taxonomy = (root / TAXONOMY_PATH).read_text()
    failures.extend(validate_area_routing(matrix, taxonomy, canonical_areas=set(AREAS)))
    board = (root / BOARD_PATH).read_text()
    if not P5_000B_CLOSED_RE.search(board):
        failures.append("VERIF-P5-000B must remain completed after Phase 11")
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))

    inventory_rows = tuple(_inventory_row(record) for _, record in sorted(inventory.items()))
    suite_rows = tuple(_suite_row(record) for _, record in sorted(suites.items()))
    area_rows = tuple(_area_row(record) for record in matrix.get("areas", []))
    route_rows = tuple(
        _route_row(index, record)
        for index, record in enumerate(matrix.get("code_areas", []), start=1)
    )
    _require_denominators(inventory_rows, suite_rows, area_rows, route_rows)
    verification_job = next(
        row for row in inventory_rows if row["id"] == "GATE_JOB_VERIFICATION_REPORT"
    )
    boundaries = {
        "verification_gate_enforcing": verification_job["disposition"] == "assigned"
        and verification_job["enforcement"] == "required",
        "report_emits_proof": False,
        "report_closes_spec_gaps": False,
        "suite_includes_interpreted": False,
        "p5_000b_remains_open": False,
    }
    input_paths = tuple(
        sorted(
            {
                *REPORT_CONTRACT_PATHS,
                *(f"verification/suites/{path.name}" for path in (root / "verification/suites").glob("*.toml")),
                *(str(record["path"]) for record in inventory.values()),
            }
        )
    )
    path_failures = validate_bound_input_paths(root, input_paths)
    if path_failures:
        raise ValueError("; ".join(path_failures))
    commit = _head_commit(root)
    if require_clean_commit:
        dirty = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
            check=False,
            capture_output=True,
        )
        if dirty.returncode != 0 or dirty.stdout:
            raise ValueError("source commit must identify a clean full Git SHA")
    return LivePhase5AuditState(
        commit=commit,
        timestamp=timestamp or datetime.now(timezone.utc).isoformat(timespec="seconds"),
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        inventory_rows=inventory_rows,
        suite_rows=suite_rows,
        area_rows=area_rows,
        route_rows=route_rows,
        boundaries=boundaries,
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


def _load_suites(root: Path) -> dict[str, dict[str, Any]]:
    suites: dict[str, dict[str, Any]] = {}
    for path in sorted((root / "verification/suites").glob("*.toml")):
        record = tomllib.loads(path.read_text())
        record["_path"] = path
        suite_id = record.get("id")
        if not isinstance(suite_id, str) or suite_id in suites:
            raise ValueError(f"invalid or duplicate suite id in {path}")
        suites[suite_id] = record
    return suites


def _inventory_row(record: dict[str, Any]) -> dict[str, Any]:
    fields = (
        "schema_version", "id", "source_kind", "path", "name", "command", "variant",
        "command_role",
        "disposition", "suite_ids", "owner", "duration_class", "environment",
        "artifact_kind", "artifact_paths", "artifact_retention", "enforcement",
        "required_env", "rationale",
    )
    return {"discovery_id": record.get("discovery_id"), **{field: record[field] for field in fields}}


def _suite_row(record: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": record["id"],
        "title": record["title"],
        "status": record["status"],
        "owner": record["owner"],
        "duration_class": record["duration_class"],
        "environment": record["environment"],
        "direct_commands": list(record["commands"]),
        "direct_command_bindings": list(record["command_bindings"]),
        "direct_inventory_refs": list(record["inventory_ids"]),
        "evidence_destination": record["evidence_destination"],
        "includes": list(record["includes"]),
        "excludes": list(record["excludes"]),
    }


def _area_row(record: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": record["id"],
        "status": record["status"],
        "owner": record["owner"],
        "risk_default": record["risk_default"],
        "path_globs": list(record["path_globs"]),
        "required_test_classes": list(record["required_test_classes"]),
        "required_case_families": list(record["required_case_families"]),
        "direct_suite_tiers": list(record["suite_tiers"]),
    }


def _route_row(order: int, record: dict[str, Any]) -> dict[str, Any]:
    return {
        "order": order,
        "id": record["id"],
        "match_kind": record["match_kind"],
        "area_ids": list(record["area_ids"]),
        "path_globs": list(record["path_globs"]),
        "intents": list(record["intents"]),
        "required_test_classes": list(record["required_test_classes"]),
        "direct_suite_tiers": list(record["suite_tiers"]),
        "conditional_suite_tiers": list(record["conditional_suite_tiers"]),
        "notes": record["notes"],
    }


def _require_denominators(inventory, suites, areas, routes) -> None:
    expected = (("inventory records", len(inventory), 63), ("suite records", len(suites), 6),
                ("canonical areas", len(areas), 11), ("taxonomy routes", len(routes), 29))
    failures = [f"Phase 5 requires {want} {label}, found {actual}" for label, actual, want in expected if actual != want]
    if sum(row["discovery_id"] is not None for row in inventory) != 60:
        failures.append("Phase 5 requires exactly 60 live scanner-bound inventory records")
    if failures:
        raise ValueError("; ".join(failures))


def _head_commit(root: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"], check=False, capture_output=True, text=True
    )
    commit = result.stdout.strip()
    if result.returncode != 0 or not COMMIT_RE.fullmatch(commit):
        raise ValueError("source commit must identify a clean full Git SHA")
    return commit
