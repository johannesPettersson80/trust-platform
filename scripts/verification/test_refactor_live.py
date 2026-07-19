"""Live, fail-closed input composition for the Phase 2A refactor assessment."""

from __future__ import annotations

import json
import tomllib
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_validation import validate_report_payload as validate_scan_payload
from .test_catalog_vscode_registration import audit_vscode_test_registration
from .test_refactor_assessment import LIMITATIONS, build_test_refactor_assessment
from .test_refactor_contract import (
    PROPOSAL_SCHEMA_PATH,
    PROPOSALS_PATH,
    REDIRECT_SCHEMA_PATH,
    REDIRECTS_PATH,
    load_named_records,
    load_test_refactor_files,
    validate_test_refactor_records,
)


CATALOG_PATH = Path("verification/test-catalog.toml")
POLICY_PATH = Path("xtask/config/full_map_policy.json")
REPORT_SCHEMA_PATH = Path("verification/schemas/test-refactor-assessment-report.schema.json")
THRESHOLD_SOURCE = "xtask/config/full_map_policy.json#kiss.existing_file_note_limit"
REPORT_ENTRYPOINT_PATHS = {
    "scripts/check_test_catalog_staleness.py",
    "scripts/report_test_refactor_assessment.py",
    "scripts/validate_test_refactor_assessment_report.py",
    "scripts/validate_test_refactor_proposals.py",
}
REPORT_LIMITATIONS = (
    *LIMITATIONS,
    "Mechanical signals never authorize a move, rename, or split; change dispositions remain unsupported in this v1 assessment.",
    "The single-identity proposal model refuses split rather than under-modeling multiple targets.",
    "Completed moves and renames require case-file-bound lock proof; catalog rows without that binding remain blocked.",
    "The mutable evidence index is globally validated but excluded from the report digest closure to avoid self-reference.",
    "Platform is historical generation provenance; at-rest validation cannot rederive a prior host platform.",
)


@dataclass(frozen=True)
class LiveTestRefactorState:
    assessment: dict[str, Any]
    scope: dict[str, Any]
    limitations: tuple[str, ...]
    input_paths: tuple[str, ...]
    commit: str
    timestamp: str
    platform: str
    catalog_count: int
    fact_count: int
    proposal_count: int
    redirect_count: int


def build_live_test_refactor_state(
    root: Path,
    *,
    timestamp: str | None = None,
) -> LiveTestRefactorState:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("--root must identify the repository that loaded verification modules")

    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "; ".join(
                f"metadata: {_display_path(root, failure.path)}: {failure.message}"
                for failure in validator.failures
            )
        )

    scan = scan_repository(root, timestamp=timestamp)
    scan_payload = scan.to_dict()
    scan_failures = validate_scan_payload(scan_payload)
    if scan_payload.get("scan_status") != "complete":
        scan_failures.append("generated catalog scan_status is not complete")
    if scan_failures:
        raise ValueError("; ".join(f"generated catalog: {item}" for item in scan_failures))

    catalog_failures, tests = load_named_records(root / CATALOG_PATH, "tests")
    contract_failures, proposals, redirects = load_test_refactor_files(root)
    evidence_failures, evidence = load_named_records(
        root / "verification/evidence-index.toml", "evidence"
    )
    failures = [*catalog_failures, *contract_failures, *evidence_failures]
    failures.extend(
        validate_catalog_staleness(root=root, tests=tests, facts=scan.inferred_facts)
    )

    suites, suite_paths = _load_suites(root)
    threshold = _load_large_file_threshold(root)
    audit = audit_vscode_test_registration(root)
    assessment = build_test_refactor_assessment(
        root=root,
        scanner_facts=[fact.to_dict() for fact in scan.inferred_facts],
        catalog_records=[tests[test_id] for test_id in sorted(tests)],
        suites=suites,
        vscode_registration_audit=asdict(audit),
        large_file_threshold=threshold,
        proposals=[proposals[proposal_id] for proposal_id in sorted(proposals)],
    )
    failures.extend(
        validate_test_refactor_records(
            root=root,
            proposals=proposals,
            redirects=redirects,
            tests=tests,
            evidence=evidence,
            facts=scan.inferred_facts,
            assessment=assessment,
        )
    )
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))

    case_files = {
        str(record["case_file"])
        for record in tests.values()
        if isinstance(record.get("case_file"), str)
    }
    input_paths = sorted(
        set(scan.provenance.input_paths)
        | validator_code_input_paths(root)
        | case_files
        | set(suite_paths)
        | REPORT_ENTRYPOINT_PATHS
        | {
            CATALOG_PATH.as_posix(),
            POLICY_PATH.as_posix(),
            PROPOSALS_PATH.as_posix(),
            REDIRECTS_PATH.as_posix(),
            PROPOSAL_SCHEMA_PATH.as_posix(),
            REDIRECT_SCHEMA_PATH.as_posix(),
            REPORT_SCHEMA_PATH.as_posix(),
            "verification/schemas/catalog.schema.json",
            "verification/schemas/suite.schema.json",
        }
    )
    input_failures = validate_bound_input_paths(root, input_paths)
    if input_failures:
        raise ValueError("; ".join(input_failures))

    return LiveTestRefactorState(
        assessment=assessment,
        scope={
            "large_file_line_threshold": threshold,
            "large_threshold_source": THRESHOLD_SOURCE,
            "mixed_purpose_basis": "reviewed_catalog_area_or_test_class_diversity_only",
            "broad_claim_basis": "multiple_catalog_invariants_without_authorized_catalog_v2_dimensions",
            "duplicate_basis": "whole_file_exact_or_normalized_plus_explicit_case_structure",
            "duration_basis": "reviewed_catalog_duration_class_only",
            "debt_is_report_failure": False,
        },
        limitations=REPORT_LIMITATIONS,
        input_paths=tuple(input_paths),
        commit=scan.provenance.commit,
        timestamp=timestamp or scan.provenance.timestamp,
        platform=scan.provenance.platform,
        catalog_count=len(tests),
        fact_count=len(scan.inferred_facts),
        proposal_count=len(proposals),
        redirect_count=len(redirects),
    )


def _load_suites(root: Path) -> tuple[list[dict[str, Any]], list[str]]:
    suites: list[dict[str, Any]] = []
    paths: list[str] = []
    for path in sorted((root / "verification/suites").glob("*.toml")):
        try:
            record = tomllib.loads(path.read_text())
        except (OSError, tomllib.TOMLDecodeError) as exc:
            raise ValueError(f"suite cannot be read: {path}: {exc}") from exc
        suites.append(record)
        paths.append(path.relative_to(root).as_posix())
    if not suites:
        raise ValueError("verification/suites contains no suite records")
    return suites, paths


def _load_large_file_threshold(root: Path) -> int:
    try:
        policy = json.loads((root / POLICY_PATH).read_text())
        value = policy["kiss"]["existing_file_note_limit"]
    except (OSError, json.JSONDecodeError, KeyError, TypeError) as exc:
        raise ValueError(f"large-file threshold cannot be read from {POLICY_PATH}: {exc}") from exc
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ValueError("kiss.existing_file_note_limit must be a positive integer")
    return value


def _display_path(root: Path, path: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
