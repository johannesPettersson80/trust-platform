"""Closed contract and deterministic rendering for Phase 6A self-test fixtures."""

from __future__ import annotations

import tomllib
from pathlib import Path
from typing import Any, Iterable

from .metadata_validator.constants import ROOT


BYPASS_CONTRACT_PATH = ROOT / "verification/selftests/bypass-fixtures.toml"
ROOT_FIELDS = {
    "schema_version",
    "id",
    "status",
    "owner",
    "last_reviewed",
    "spec_source_scanner_status",
    "spec_source_scanner_blocked_by",
    "limitations",
    "cases",
}
CASE_FIELDS = {
    "id",
    "board_row",
    "fixture_kind",
    "assigned_layer",
    "executor",
    "expected_disposition",
    "expected_signal",
    "assertion_strength",
}
EXPECTED_CASES = {
    "P6A_GOOD_COMMITTED_METADATA_001": ("VERIF-P6A-001", "metadata_known_good"),
    "P6A_BAD_MISSING_REQUIRED_FIELD_001": ("VERIF-P6A-002", "metadata_missing_required_field"),
    "P6A_BAD_UNKNOWN_STATUS_001": ("VERIF-P6A-002", "metadata_unknown_status"),
    "P6A_BAD_STALE_RUNNABLE_PATH_001": ("VERIF-P6A-002", "metadata_stale_runnable_path"),
    "P6A_BAD_UNKNOWN_INVARIANT_001": ("VERIF-P6A-002", "metadata_unknown_invariant"),
    "P6A_BAD_UNKNOWN_SUITE_001": ("VERIF-P6A-002", "metadata_unknown_suite"),
    "P6A_BAD_SCHEMA_VERSION_001": ("VERIF-P6A-002", "metadata_schema_version"),
    "P6A_BAD_PUBLIC_CLAIM_WITHOUT_PROOF_OR_GAP_001": (
        "VERIF-P6A-002",
        "metadata_public_claim_without_proof_or_gap",
    ),
    "P6A_BAD_IGNORED_DURABLE_EVIDENCE_001": (
        "VERIF-P6A-002A",
        "metadata_ignored_durable_evidence",
    ),
    "P6A_BAD_UNKNOWN_EVIDENCE_001": ("VERIF-P6A-002A", "metadata_unknown_evidence"),
    "P6A_BAD_MAPPED_EMPTY_INVARIANTS_001": (
        "VERIF-P6A-002A",
        "metadata_mapped_empty_invariants",
    ),
    "P6A_BAD_STALE_TEST_NAME_001": ("VERIF-P6A-002A", "catalog_stale_test_name"),
    "P6A_BAD_VALIDATED_EMPTY_EVIDENCE_001": (
        "VERIF-P6A-002A",
        "metadata_validated_empty_evidence",
    ),
    "P6A_BAD_SAFETY_VALIDATED_GAP_OPEN_001": (
        "VERIF-P6A-002A",
        "metadata_safety_validated_gap_open",
    ),
    "P6A_BAD_SAFETY_VALIDATED_SPEC_GAP_001": (
        "VERIF-P6A-002A",
        "metadata_safety_validated_spec_gap",
    ),
    "P6A_BAD_VALIDATED_LOW_PROOF_001": (
        "VERIF-P6A-002A",
        "metadata_validated_low_proof",
    ),
    "P6A_BAD_DECISION_TABLE_MISSING_BEHAVIOR_001": (
        "VERIF-P6A-009",
        "metadata_decision_table_missing_behavior",
    ),
    "P6A_BAD_CASE_UNKNOWN_FAMILY_001": ("VERIF-P6A-009", "case_unknown_family"),
    "P6A_BAD_STALE_CASE_DIGEST_001": ("VERIF-P6A-009", "metadata_stale_case_digest"),
    "P6A_BAD_SKIPPED_CASE_ARTIFACT_001": (
        "VERIF-P6A-009",
        "proof_skipped_case_artifact",
    ),
    "P6A_BAD_HIGH_RISK_RED_PRODUCER_001": (
        "VERIF-P6A-009",
        "evidence_high_risk_red_producer",
    ),
    "P6A_BAD_HIGH_RISK_GREEN_PRODUCER_001": (
        "VERIF-P6A-009",
        "evidence_high_risk_green_producer",
    ),
    "P6A_BAD_GREEN_MISSING_RED_PAIR_001": (
        "VERIF-P6A-009",
        "evidence_green_missing_red_pair",
    ),
    "P6A_BAD_RISK_DOWNGRADE_NO_DECISION_001": (
        "VERIF-P6A-009",
        "planner_risk_downgrade_without_decision",
    ),
    "P6A_BAD_COMPILE_ERROR_AS_RED_001": ("VERIF-P6A-009", "proof_compile_error_as_red"),
    "P6A_BAD_HARNESS_PANIC_AS_RED_001": ("VERIF-P6A-009", "proof_harness_panic_as_red"),
    "P6A_BAD_ASSERT_NOTHING_RED_001": ("VERIF-P6A-010", "proof_assert_nothing_red"),
}
EXECUTOR_CONTRACTS = {
    "metadata_known_good": (
        "known_good",
        "metadata_validator",
        "accept",
        "no validation failures",
    ),
    "metadata_missing_required_field": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "test TEST_BYTECODE_CONTAINER_INVALID_MAGIC missing owner",
    ),
    "metadata_unknown_status": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "uses unknown status 'fixture_unknown'",
    ),
    "metadata_stale_runnable_path": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "runnable test path does not exist",
    ),
    "metadata_unknown_invariant": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "references unknown invariant INV_UNKNOWN_P6A",
    ),
    "metadata_unknown_suite": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "references unknown suite SUITE_UNKNOWN_P6A",
    ),
    "metadata_schema_version": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "must use schema_version = 2",
    ),
    "metadata_public_claim_without_proof_or_gap": (
        "known_bad",
        "metadata_validator.validate_public_claim_links",
        "reject",
        "has no proof-backed invariant or explicit spec gap",
    ),
    "metadata_ignored_durable_evidence": (
        "known_bad",
        "metadata_validator.validate_evidence",
        "reject",
        "evidence path is gitignored",
    ),
    "metadata_unknown_evidence": (
        "known_bad",
        "metadata_validator.validate_invariants",
        "reject",
        "references unknown evidence EVID_UNKNOWN_P6A",
    ),
    "metadata_mapped_empty_invariants": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "mapped test must name invariants",
    ),
    "catalog_stale_test_name": (
        "known_bad",
        "catalog_staleness",
        "reject",
        "name is stale",
    ),
    "metadata_validated_empty_evidence": (
        "known_bad",
        "metadata_validator.validate_invariants",
        "reject",
        "validated without evidence_refs",
    ),
    "metadata_safety_validated_gap_open": (
        "known_bad",
        "metadata_validator.validate_invariants",
        "reject",
        "high-risk validated with open coverage cell time_or_clock_fault",
    ),
    "metadata_safety_validated_spec_gap": (
        "known_bad",
        "metadata_validator.validate_invariants",
        "reject",
        "high-risk validated with open coverage cell ordering_or_lifecycle",
    ),
    "metadata_validated_low_proof": (
        "known_bad",
        "metadata_validator.validate_invariants",
        "reject",
        "validated with insufficient proof_level 'S1'",
    ),
    "metadata_decision_table_missing_behavior": (
        "known_bad",
        "metadata_validator.validate_invariants",
        "reject",
        "decision_table has applicable covered dimensions but no behavior rows",
    ),
    "case_unknown_family": (
        "known_bad",
        "case_file_validator",
        "reject",
        "uses unknown family 'fixture_unknown'",
    ),
    "metadata_stale_case_digest": (
        "known_bad",
        "metadata_validator.validate_tests",
        "reject",
        "case_file_digest mismatch",
    ),
    "proof_skipped_case_artifact": (
        "known_bad",
        "case_artifact_validator",
        "reject",
        "was skipped without waiver",
    ),
    "evidence_high_risk_red_producer": (
        "known_bad",
        "evidence_pairing",
        "reject",
        "high-risk red/green proof producer 'codex' is not allowlisted",
    ),
    "evidence_high_risk_green_producer": (
        "known_bad",
        "evidence_pairing",
        "reject",
        "high-risk red/green proof producer 'codex' is not allowlisted",
    ),
    "evidence_green_missing_red_pair": (
        "known_bad",
        "evidence_pairing",
        "reject",
        "green proof missing pairing field paired_red_evidence",
    ),
    "planner_risk_downgrade_without_decision": (
        "known_bad",
        "planner_report",
        "report",
        "risk downgrade requires decision_ref",
    ),
    "proof_compile_error_as_red": (
        "known_bad",
        "proof_producer",
        "reject",
        "compile_error",
    ),
    "proof_harness_panic_as_red": (
        "known_bad",
        "proof_producer",
        "reject",
        "harness_panic",
    ),
    "proof_assert_nothing_red": (
        "boundary",
        "proof_producer",
        "reject",
        "none",
    ),
}
REQUIRED_CASE_IDS = set(EXPECTED_CASES)
LAYERS = {
    "metadata_validator",
    "metadata_validator.validate_tests",
    "metadata_validator.validate_invariants",
    "metadata_validator.validate_evidence",
    "metadata_validator.validate_public_claim_links",
    "catalog_staleness",
    "case_file_validator",
    "case_artifact_validator",
    "evidence_pairing",
    "planner_report",
    "proof_producer",
}


def load_bypass_contract(path: Path = BYPASS_CONTRACT_PATH) -> dict[str, Any]:
    return tomllib.loads(path.read_text())


def validate_bypass_contract(contract: Any) -> list[str]:
    failures: list[str] = []
    if not isinstance(contract, dict):
        return ["bypass contract root must be a table"]
    _check_fields(contract, ROOT_FIELDS, "contract", failures)
    if contract.get("schema_version") != 1:
        failures.append("bypass contract must use schema_version = 1")
    if contract.get("id") != "PLC_VERIFICATION_TOOLING_BYPASSES_V1":
        failures.append("bypass contract id drift")
    if contract.get("status") != "mapped":
        failures.append("bypass contract status must be mapped")
    if contract.get("spec_source_scanner_status") != "blocked":
        failures.append("spec-source scanner self-tests must remain blocked")
    if contract.get("spec_source_scanner_blocked_by") != [
        "VERIF-P1A-002",
        "VERIF-P1A-003",
        "VERIF-P1A-006",
    ]:
        failures.append("spec-source scanner blocker list drift")
    limitations = contract.get("limitations")
    if not isinstance(limitations, list) or not limitations or not all(
        isinstance(item, str) and item for item in limitations
    ):
        failures.append("limitations must be a non-empty string array")

    cases = contract.get("cases")
    if not isinstance(cases, list):
        return failures + ["bypass contract must define [[cases]]"]
    seen: set[str] = set()
    for index, row in enumerate(cases):
        label = f"cases[{index}]"
        if not isinstance(row, dict):
            failures.append(f"{label} must be a table")
            continue
        _check_fields(row, CASE_FIELDS, label, failures)
        case_id = row.get("id")
        if case_id in seen:
            failures.append(f"duplicate fixture id {case_id}")
        if isinstance(case_id, str):
            seen.add(case_id)
        expected = EXPECTED_CASES.get(case_id)
        if expected is None:
            failures.append(f"unknown fixture id {case_id!r}")
        elif (row.get("board_row"), row.get("executor")) != expected:
            failures.append(f"{case_id} board-row/executor binding drift")
        executor_contract = EXECUTOR_CONTRACTS.get(row.get("executor"))
        actual_contract = (
            row.get("fixture_kind"),
            row.get("assigned_layer"),
            row.get("expected_disposition"),
            row.get("expected_signal"),
        )
        if executor_contract is None or actual_contract != executor_contract:
            failures.append(f"{case_id} fixture catcher contract drift")
        if row.get("fixture_kind") not in {"known_good", "known_bad", "boundary"}:
            failures.append(f"{case_id} has unknown fixture_kind")
        if row.get("assigned_layer") not in LAYERS:
            failures.append(f"{case_id} has unknown assigned_layer")
        if row.get("expected_disposition") not in {"accept", "reject", "report"}:
            failures.append(f"{case_id} has unknown expected_disposition")
        if not isinstance(row.get("expected_signal"), str) or not row["expected_signal"]:
            failures.append(f"{case_id} must name expected_signal")
        if row.get("assertion_strength") != "not_assessed":
            failures.append(f"{case_id} must not claim assertion strength")
    if seen != REQUIRED_CASE_IDS:
        failures.append(
            "fixture set drift: "
            f"missing={sorted(REQUIRED_CASE_IDS - seen)}, extra={sorted(seen - REQUIRED_CASE_IDS)}"
        )
    return sorted(set(failures))


def render_fixture_report(contract: dict[str, Any], results: Iterable[Any]) -> str:
    ordered = sorted(results, key=lambda item: item.case_id)
    matched = sum(bool(item.matched) for item in ordered)
    lines = [
        "# Verification Tooling Self-Test Fixture Report",
        "",
        f"Contract: `{contract['id']}`",
        f"Fixtures matched: `{matched}/{len(ordered)}`",
        f"Spec-source scanner self-tests: `{contract['spec_source_scanner_status']}`",
        "Metadata proves assertion strength: `false`",
        "",
        "| Fixture | Board row | Assigned layer | Expected | Actual | Signal matched | Full wiring |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    by_id = {row["id"]: row for row in contract["cases"]}
    for result in ordered:
        row = by_id[result.case_id]
        full_wiring = (
            str(result.full_wiring_matched).lower()
            if result.assigned_layer.startswith("metadata_validator")
            else "n/a"
        )
        lines.append(
            f"| `{result.case_id}` | `{row['board_row']}` | `{result.assigned_layer}` | "
            f"`{result.expected_disposition}` | `{result.actual_disposition}` | "
            f"`{str(result.matched).lower()}` | `{full_wiring}` |"
        )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in contract["limitations"])
    return "\n".join(lines) + "\n"


def _check_fields(
    record: dict[str, Any], expected: set[str], label: str, failures: list[str]
) -> None:
    actual = set(record)
    if actual != expected:
        failures.append(
            f"{label} fields drift: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )
