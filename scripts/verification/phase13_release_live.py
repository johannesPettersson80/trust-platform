"""Live repository state for the Phase 13 release-evidence audit."""

from __future__ import annotations

import json
import platform as host_platform
import re
import subprocess
import tomllib
from collections import Counter
from dataclasses import dataclass
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Any, Mapping

try:
    from scripts.check_dependency_exceptions import validate_file
    from scripts.check_release_version_alignment import (
        package_json_version,
        package_lock_versions,
        workspace_version_from_cargo,
    )
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    from check_dependency_exceptions import validate_file  # type: ignore[no-redef]
    from check_release_version_alignment import (  # type: ignore[no-redef]
        package_json_version,
        package_lock_versions,
        workspace_version_from_cargo,
    )

from .metadata_validator.core import Validator
from .phase13_release import (
    MANIFEST_PATH,
    MANIFEST_SCHEMA_PATH,
    PROOF_ORIGINS,
    REPORT_SCHEMA_PATH,
    validate_manifest,
)
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
STATIC_INPUTS = {
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    "Cargo.toml",
    "CHANGELOG.md",
    "deny.toml",
    "docs/specs/24-release-evidence.md",
    "editors/vscode/package.json",
    "editors/vscode/package-lock.json",
    "scripts/check_dependency_exceptions.py",
    "scripts/check_release_tag_preflight.py",
    "scripts/check_release_version_alignment.py",
    "scripts/check_version_release_evidence.py",
    "scripts/generate_release_gate_report.py",
    "scripts/generate_release_provenance.py",
    "scripts/release_evidence_contract.py",
    "scripts/report_phase13_release_evidence.py",
    "scripts/validate_phase13_release_evidence_report.py",
    "scripts/verification/phase13_release.py",
    "scripts/verification/phase13_release_cli.py",
    "scripts/verification/phase13_release_live.py",
    "scripts/verification/phase13_release_validation.py",
    "verification/spec-gaps.toml",
    "verification/test-catalog.toml",
    "verification/ui-acceptance.toml",
    "verification/suites/release.toml",
    MANIFEST_PATH,
    MANIFEST_SCHEMA_PATH,
    REPORT_SCHEMA_PATH,
}


@dataclass(frozen=True)
class LivePhase13State:
    commit: str
    branch: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    manifest: dict[str, Any]
    candidate: dict[str, Any]
    public_release: dict[str, Any]
    proof_origins: tuple[dict[str, Any], ...]
    security: dict[str, Any]
    platforms: tuple[dict[str, Any], ...]
    conformance: dict[str, Any]
    hardware_labs: tuple[dict[str, Any], ...]
    ui_acceptance: dict[str, Any]
    known_gaps: tuple[dict[str, Any], ...]


def build_live_phase13_state(
    root: Path,
    *,
    branch: str,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LivePhase13State:
    root = root.resolve()
    if not branch or branch.startswith("-"):
        raise ValueError("branch label must be non-empty and must not look like an option")
    report_timestamp = timestamp or datetime.now(timezone.utc).isoformat(timespec="seconds")
    parsed_timestamp = datetime.fromisoformat(report_timestamp)
    if parsed_timestamp.tzinfo is None:
        raise ValueError("timestamp must be ISO-8601 with a timezone")

    manifest = tomllib.loads((root / MANIFEST_PATH).read_text(encoding="utf-8"))
    manifest_schema = json.loads((root / MANIFEST_SCHEMA_PATH).read_text(encoding="utf-8"))
    manifest_failures = validate_manifest(manifest)
    manifest_failures.extend(validate_json_schema_instance(manifest, manifest_schema))
    if manifest_failures:
        raise ValueError("release-evidence manifest invalid: " + "; ".join(sorted(set(manifest_failures))))

    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "metadata validation failed: "
            + "; ".join(failure.message for failure in validator.failures[:10])
        )

    version = workspace_version_from_cargo(root / "Cargo.toml")
    package_version = package_json_version(root / "editors/vscode/package.json")
    lock_top, lock_root = package_lock_versions(root / "editors/vscode/package-lock.json")
    versions = [version, package_version, lock_top, lock_root]
    expected_tag = f"v{version}"
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    annotated_tag = _annotated_tag_present(root, expected_tag)

    snapshot = dict(manifest["latest_public_snapshot"])
    required_assets = list(manifest["required_release_assets"])
    missing_assets = sorted(set(required_assets) - set(snapshot["assets"]))
    matches_candidate = snapshot["tag"] == expected_tag
    public_release = {
        **snapshot,
        "required_assets": required_assets,
        "missing_required_assets": missing_assets,
        "matches_candidate": matches_candidate,
    }
    candidate = {
        "version": version,
        "expected_tag": expected_tag,
        "version_sources": list(manifest["version_sources"]),
        "versions_synchronized": len(set(versions)) == 1,
        "changelog_mentions_version": f"v{version}" in changelog,
        "annotated_tag_present": annotated_tag,
        "release_complete": annotated_tag and matches_candidate and not missing_assets,
    }

    evidence = [
        record
        for evidence_id, record in validator.evidence.items()
        if not evidence_id.startswith("EVID_P13_")
    ]
    proof_origins = tuple(_proof_origin_rows(evidence, snapshot))
    report_date = parsed_timestamp.date()
    exceptions = validate_file(root / manifest["security_policy_path"], today=report_date)
    ci_text = (root / manifest["ci_workflow_path"]).read_text(encoding="utf-8")
    release_text = (root / manifest["release_workflow_path"]).read_text(encoding="utf-8")
    policy = manifest["security_policy"]
    security = {
        "owned_exceptions": len(exceptions),
        "expired_exceptions": sum(row.expires is not None and row.expires < report_date for row in exceptions),
        "maximum_exception_days": policy["maximum_exception_days"],
        "cargo_policy_configured": all(
            fragment in ci_text
            for fragment in (
                "check_dependency_exceptions.py",
                "cargo deny check advisories licenses bans sources",
                "cargo audit --deny warnings",
            )
        ),
        "npm_audit_configured": "npm audit --audit-level=low" in ci_text + release_text,
        "rust_commands": list(policy["rust_commands"]),
        "node_commands": list(policy["node_commands"]),
        "gate_execution_claimed": False,
    }

    snapshot_version = snapshot["tag"].removeprefix("v")
    snapshot_assets = set(snapshot["assets"])
    platform_rows = []
    for row in manifest["platforms"]:
        expected_assets = [
            row["runtime_asset"], row["lsp_asset"],
            row["vsix_asset_template"].format(version=snapshot_version),
        ]
        platform_rows.append(
            {
                **row,
                "snapshot_tag": snapshot["tag"],
                "expected_public_assets": expected_assets,
                "public_assets_present": set(expected_assets) <= snapshot_assets,
            }
        )

    conformance_rows = [
        row for row in validator.tests.values()
        if row.get("discovery_source_kind") == "conformance_case"
    ]
    conformance = {
        "catalog_cases": len(conformance_rows),
        "linked_cases": sum(bool(row.get("invariants")) and bool(row.get("oracle_ref")) for row in conformance_rows),
        "missing_links": sorted(row["id"] for row in conformance_rows if not row.get("invariants") or not row.get("oracle_ref")),
        "public_asset_present": "conformance-status.json" in snapshot_assets and "conformance-status.md" in snapshot_assets,
        "execution_claimed": False,
    }
    hardware_labs = tuple(
        {
            "board_row": row_id,
            "status": "skipped_unproven",
            "evidence_count": sum(record.get("kind") == "lab_report" for record in evidence),
        }
        for row_id in manifest["hardware_lab_rows"]
    )
    journeys = validator.ui_acceptance.get("journeys", [])
    statuses = Counter(row.get("status") for row in journeys)
    ui_acceptance = {
        "journeys": len(journeys),
        "accepted_journeys": statuses["ux_accepted"],
        "provisional_journeys": statuses["provisional"],
        "missing_journeys": statuses["evidence_missing"],
        "stale_journeys": statuses["stale"],
    }
    open_spec_gaps = sorted(
        row["id"] for row in validator.spec_gaps.values()
        if row.get("resolution_status") != "closed"
    )
    known_gaps = tuple(
        _known_gaps(
            candidate=candidate,
            public_release=public_release,
            open_spec_gaps=open_spec_gaps,
            hardware_labs=hardware_labs,
            ui_acceptance=ui_acceptance,
            conformance=conformance,
        )
    )

    input_paths = tuple(sorted(STATIC_INPUTS))
    path_failures = validate_bound_input_paths(root, input_paths)
    if path_failures:
        raise ValueError("; ".join(path_failures))
    commit = _head_commit(root)
    if require_clean_commit:
        status = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=all"],
            cwd=root, check=False, capture_output=True,
        )
        if status.returncode or status.stdout:
            raise ValueError("source commit must identify a clean full Git SHA")
    return LivePhase13State(
        commit=commit,
        branch=branch,
        timestamp=report_timestamp,
        platform=f"{host_platform.system().lower()}-{host_platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        manifest=manifest,
        candidate=candidate,
        public_release=public_release,
        proof_origins=proof_origins,
        security=security,
        platforms=tuple(platform_rows),
        conformance=conformance,
        hardware_labs=hardware_labs,
        ui_acceptance=ui_acceptance,
        known_gaps=known_gaps,
    )


def validate_source_revision(root: Path, commit: object, input_paths: tuple[str, ...]) -> list[str]:
    root = root.resolve()
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        return sorted(set([*failures, "commit must identify a clean full Git SHA"]))
    if subprocess.run(["git", "cat-file", "-e", f"{commit}^{{commit}}"], cwd=root, check=False, capture_output=True).returncode:
        return sorted(set([*failures, f"commit does not resolve in repository: {commit}"]))
    tree = subprocess.run(["git", "ls-tree", "-r", "--name-only", "-z", commit], cwd=root, check=False, capture_output=True)
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append("source commit lacks report inputs: " + ", ".join(missing))
    if subprocess.run(["git", "diff", "--quiet", commit, "--", *input_paths], cwd=root, check=False, capture_output=True).returncode:
        failures.append("report inputs differ from the claimed source commit")
    return sorted(set(failures))


def _proof_origin_rows(evidence: list[Mapping[str, Any]], snapshot: Mapping[str, Any]) -> list[dict[str, Any]]:
    counts = Counter()
    for row in evidence:
        platform = str(row.get("platform", "")).lower()
        if "local-" in platform or platform.startswith("local"):
            counts["local"] += 1
        if "trust-builder" in platform:
            counts["remote_builder"] += 1
        if row.get("kind") == "ci_artifact":
            counts["ci"] += 1
        if row.get("kind") == "lab_report":
            counts["hardware_lab"] += 1
        if row.get("kind") == "release_object":
            counts["public_github"] += 1
    limitations = {
        "local": "Local committed evidence is not remote, CI, hardware, or public proof.",
        "remote_builder": "Builder evidence is not CI or public release proof.",
        "ci": "Only typed ci_artifact records count; configured jobs do not.",
        "hardware_lab": "Only typed lab_report records count; skipped rows do not.",
        "public_github": "The checked public snapshot is metadata until a release_object evidence row exists.",
    }
    return [
        {
            "origin": origin,
            "evidence_count": counts[origin],
            "status": "recorded" if counts[origin] else ("snapshot_only" if origin == "public_github" and snapshot else "missing"),
            "limitation": limitations[origin],
        }
        for origin in PROOF_ORIGINS
    ]


def _known_gaps(*, candidate, public_release, open_spec_gaps, hardware_labs, ui_acceptance, conformance) -> list[dict[str, Any]]:
    return [
        {"id": "SPEC_GAPS", "status": "closed" if not open_spec_gaps else "open", "detail": f"{len(open_spec_gaps)} specification gaps remain open"},
        {"id": "CANDIDATE_PUBLICATION", "status": "closed" if candidate["release_complete"] else "open", "detail": f"{candidate['expected_tag']} lacks complete tag/workflow/Latest evidence"},
        {"id": "PUBLIC_RELEASE_ASSETS", "status": "closed" if not public_release["missing_required_assets"] else "open", "detail": f"Latest snapshot is missing {len(public_release['missing_required_assets'])} required result assets"},
        {"id": "HARDWARE_LAB", "status": "open", "detail": f"{len(hardware_labs)} hardware rows remain skipped and unproven"},
        {"id": "UI_ACCEPTANCE", "status": "closed" if ui_acceptance["accepted_journeys"] == ui_acceptance["journeys"] else "open", "detail": f"{ui_acceptance['accepted_journeys']} of {ui_acceptance['journeys']} journeys are accepted"},
        {"id": "CONFORMANCE_PUBLICATION", "status": "closed" if conformance["public_asset_present"] else "open", "detail": "Latest public release does not carry both conformance status assets"},
    ]


def _annotated_tag_present(root: Path, tag: str) -> bool:
    kind = subprocess.run(["git", "cat-file", "-t", f"refs/tags/{tag}"], cwd=root, check=False, capture_output=True, text=True)
    return kind.returncode == 0 and kind.stdout.strip() == "tag"


def _head_commit(root: Path) -> str:
    result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, check=False, capture_output=True, text=True)
    commit = result.stdout.strip()
    if result.returncode or not COMMIT_RE.fullmatch(commit):
        raise ValueError("source commit must identify a clean full Git SHA")
    return commit
