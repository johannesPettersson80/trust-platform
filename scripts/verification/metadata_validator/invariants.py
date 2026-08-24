"""Invariant-record validation for the verification metadata control plane."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from .constants import (
    CASE_FAMILIES,
    CONTRACT_KINDS,
    COVERAGE_STATES,
    DELTA_KEYS,
    DELTA_VALUES,
    HIGH_RISKS,
    ORACLE_KINDS,
    OUTCOMES,
    PARTITION_KEYS,
    PROOF_LEVELS,
    PROOF_LEVELS_VALIDATED,
    PROVE_PRODUCER_RE,
    RISKS,
    SPEC_STATUSES,
    VERIFICATION,
)
from .oracle_refs import (
    validate_error_code_ref,
    validate_oracle_ref,
    validate_partition_contract,
)
from .integrity import UNRESOLVED_GAP_RESOLUTIONS
from .promotion_evidence import validate_invariant_promotion_evidence

Fail = Callable[[Path, str], None]
Require = Callable[[Path, dict[str, Any], list[str], str], None]
CheckCommon = Callable[[Path, dict[str, Any]], None]
CheckRefs = Callable[
    [Path, list[str], dict[str, dict[str, Any]], str, str], None
]


def validate_invariants(
    *,
    fail: Fail,
    require: Require,
    check_common: CheckCommon,
    check_refs: CheckRefs,
    invariants: dict[str, dict[str, Any]],
    spec_sources: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
    tests: dict[str, dict[str, Any]],
    ignored_tests: dict[str, dict[str, Any]],
    suites: dict[str, dict[str, Any]],
    evidence: dict[str, dict[str, Any]],
    approved_producers: set[str],
) -> None:
    """Validate invariant structure, references, and proof-state consistency."""

    required = [
        "schema_version",
        "id",
        "title",
        "area",
        "risk",
        "status",
        "owner",
        "claim",
        "contract_kind",
        "spec.status",
        "oracle.kind",
        "oracle.ref",
        "proof_level",
        "tests",
        "gates",
        "missing",
        "coverage",
    ]
    for record in invariants.values():
        path = record["_path"]
        require(path, record, required, "invariant")
        check_common(path, record)
        relative = path.relative_to(VERIFICATION / "invariants")
        if relative.parts[0] != record.get("area"):
            fail(
                path,
                f"{record['id']} area {record.get('area')!r} does not match "
                f"directory {relative.parts[0]!r}",
            )
        if path.stem != record.get("id"):
            fail(path, f"{record['id']} filename must match invariant id")
        if record.get("risk") not in RISKS:
            fail(path, f"{record['id']} has unknown risk {record.get('risk')!r}")
        if record.get("proof_level") not in PROOF_LEVELS:
            fail(
                path,
                f"{record['id']} has unknown proof_level "
                f"{record.get('proof_level')!r}",
            )
        if record.get("contract_kind") not in CONTRACT_KINDS:
            fail(
                path,
                f"{record['id']} has unknown contract_kind "
                f"{record.get('contract_kind')!r}",
            )
        spec = record.get("spec", {})
        if spec.get("status") not in SPEC_STATUSES:
            fail(
                path,
                f"{record['id']} has unknown spec.status {spec.get('status')!r}",
            )
        if not spec.get("source_refs") and not record.get("spec_gap_refs"):
            fail(path, f"{record['id']} must name spec.source_refs or spec_gap_refs")
        oracle = record.get("oracle", {})
        if oracle.get("kind") not in ORACLE_KINDS:
            fail(
                path,
                f"{record['id']} has unknown oracle.kind {oracle.get('kind')!r}",
            )
        if record.get("status") == "spec_gap":
            if oracle.get("ref") not in record.get("spec_gap_refs", []):
                fail(
                    path,
                    f"{record['id']} spec_gap oracle.ref must name one of "
                    "spec_gap_refs",
                )
            else:
                gap = spec_gaps.get(oracle.get("ref"))
                if (
                    not gap
                    or gap.get("resolution_status") not in UNRESOLVED_GAP_RESOLUTIONS
                ):
                    fail(
                        path,
                        f"{record['id']} spec_gap oracle.ref must name an "
                        "unresolved spec gap",
                    )
        elif oracle.get("ref") in spec_gaps:
            fail(path, f"{record['id']} non-spec-gap oracle.ref cannot name a spec gap")
        else:
            validate_oracle_ref(
                fail=fail,
                path=path,
                owner_id=record["id"],
                oracle_ref=oracle.get("ref"),
                spec_sources=spec_sources,
            )
        check_refs(path, record.get("tests", []), tests, "test", record["id"])
        check_refs(path, record.get("gates", []), suites, "suite", record["id"])
        check_refs(
            path,
            record.get("evidence_refs", []),
            evidence,
            "evidence",
            record["id"],
        )
        validate_invariant_promotion_evidence(
            fail=fail,
            path=path,
            invariant=record,
            evidence=evidence,
            suites=suites,
            tests=tests,
            ignored_tests=ignored_tests,
        )
        check_refs(
            path,
            record.get("spec_gap_refs", []),
            spec_gaps,
            "spec gap",
            record["id"],
        )
        for source_id in spec.get("source_refs", []):
            if source_id not in spec_sources:
                fail(path, f"{record['id']} references unknown spec source {source_id}")
        cells = record.get("coverage", {}).get("cells")
        if not isinstance(cells, list) or not cells:
            fail(path, f"{record['id']} must have coverage.cells")
        else:
            for cell in cells:
                _validate_coverage_cell(
                    fail=fail,
                    path=path,
                    record=record,
                    cell=cell,
                    spec_sources=spec_sources,
                    spec_gaps=spec_gaps,
                )
        if (
            record.get("contract_kind") == "decision_table"
            and isinstance(cells, list)
            and any(
                isinstance(cell, dict)
                and cell.get("state") in {"covered", "covered_by_fuzz"}
                for cell in cells
            )
            and not record.get("behavior")
        ):
            fail(
                path,
                f"{record['id']} decision_table has applicable covered "
                "dimensions but no behavior rows",
            )
        for behavior in record.get("behavior", []):
            _validate_behavior(
                fail=fail,
                path=path,
                record=record,
                behavior=behavior,
                spec_sources=spec_sources,
                spec_gaps=spec_gaps,
            )
        if record.get("status") == "validated":
            _validate_validated_invariant(fail, path, record)
        if record.get("status") == "test_written" and not record.get("tests"):
            fail(path, f"{record['id']} is test_written without tests")
        if record.get("status") == "implemented" and (
            not record.get("tests") or not record.get("evidence_refs")
        ):
            fail(path, f"{record['id']} is implemented without tests and evidence")
        if (
            record.get("status") in {"implemented", "validated"}
            and record.get("risk") in HIGH_RISKS
            and not _has_closing_high_risk_evidence(
                record, evidence, approved_producers
            )
        ):
            fail(
                path,
                f"{record['id']} high-risk {record.get('status')} lacks "
                "allowlisted green/lock evidence that back-links the invariant",
            )


def _validate_coverage_cell(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    cell: dict[str, Any],
    spec_sources: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
) -> None:
    dimension = cell.get("dimension")
    if dimension not in CASE_FAMILIES:
        fail(path, f"{record['id']} has unknown coverage dimension {dimension!r}")
    state = cell.get("state")
    if state not in COVERAGE_STATES:
        fail(path, f"{record['id']} has unknown coverage state {state!r}")
    if state == "spec_gap":
        gap_id = cell.get("spec_gap_ref")
        if gap_id not in spec_gaps:
            fail(
                path,
                f"{record['id']} coverage cell references unknown "
                f"spec_gap_ref {gap_id!r}",
            )
    if state in {"covered", "covered_by_fuzz"} and not record.get("tests"):
        fail(
            path,
            f"{record['id']} coverage cell {dimension} is {state} without tests",
        )
    if state == "not_applicable":
        decision_ref = cell.get("decision_ref")
        source = spec_sources.get(decision_ref)
        if (
            not source
            or source.get("authority")
            not in {"reviewed_decision", "reviewed_deviation"}
            or source.get("source_status") != "active"
            or source.get("oracle_eligible") is not True
        ):
            fail(
                path,
                f"{record['id']} not_applicable cell requires active reviewed "
                "decision/deviation decision_ref",
            )


def _validate_behavior(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    behavior: dict[str, Any],
    spec_sources: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
) -> None:
    partition = behavior.get("partition")
    if not isinstance(partition, dict) or not partition:
        fail(path, f"{record['id']} behavior must define a partition table")
    else:
        unknown_keys = set(partition) - PARTITION_KEYS
        if unknown_keys:
            fail(
                path,
                f"{record['id']} behavior partition has unknown keys "
                f"{sorted(unknown_keys)}",
            )
        validate_partition_contract(
            fail=fail,
            path=path,
            owner_id=record["id"],
            behavior=behavior,
        )
    if "oracle_ref" not in behavior and "spec_gap_ref" not in behavior:
        fail(path, f"{record['id']} behavior must name oracle_ref or spec_gap_ref")
    if "oracle_ref" in behavior and "spec_gap_ref" in behavior:
        fail(
            path,
            f"{record['id']} behavior cannot use oracle_ref and spec_gap_ref together",
        )
    if "spec_gap_ref" in behavior and behavior["spec_gap_ref"] not in spec_gaps:
        fail(
            path,
            f"{record['id']} behavior references unknown spec_gap_ref "
            f"{behavior['spec_gap_ref']}",
        )
    if "spec_gap_ref" in behavior:
        forbidden = {
            "outcome",
            "delta",
            "error_code",
            "no_partial_apply",
            "fault_surface",
        } & set(behavior)
        if forbidden:
            fail(
                path,
                f"{record['id']} spec-gap behavior cannot carry expected "
                f"outcome fields {sorted(forbidden)}",
            )
        return
    validate_oracle_ref(
        fail=fail,
        path=path,
        owner_id=record["id"],
        oracle_ref=behavior.get("oracle_ref"),
        spec_sources=spec_sources,
    )
    validate_error_code_ref(
        fail=fail,
        path=path,
        owner_id=record["id"],
        behavior=behavior,
        spec_sources=spec_sources,
    )
    if behavior.get("outcome") not in OUTCOMES:
        fail(
            path,
            f"{record['id']} has unknown behavior outcome "
            f"{behavior.get('outcome')!r}",
        )
    delta = behavior.get("delta")
    if not isinstance(delta, dict):
        fail(path, f"{record['id']} behavior must use structured delta")
        return
    extra = set(delta) - DELTA_KEYS
    missing = DELTA_KEYS - set(delta)
    if extra:
        fail(path, f"{record['id']} behavior delta has unknown keys {sorted(extra)}")
    if missing:
        fail(path, f"{record['id']} behavior delta missing keys {sorted(missing)}")
    for key, value in delta.items():
        if value not in DELTA_VALUES.get(key, set()):
            fail(
                path,
                f"{record['id']} behavior delta.{key} has invalid value {value!r}",
            )
        if value == "expected_delta" and not (
            behavior.get("expected_delta_ref")
            or behavior.get("notes")
            or behavior.get("rationale")
        ):
            fail(
                path,
                f"{record['id']} behavior uses expected_delta without "
                "oracle-cited expected_delta_ref/notes",
            )


def _validate_validated_invariant(
    fail: Fail, path: Path, record: dict[str, Any]
) -> None:
    if record.get("proof_level") not in PROOF_LEVELS_VALIDATED:
        fail(
            path,
            f"{record['id']} validated with insufficient proof_level "
            f"{record.get('proof_level')!r}",
        )
    if not record.get("tests"):
        fail(path, f"{record['id']} validated without tests")
    if not record.get("evidence_refs"):
        fail(path, f"{record['id']} validated without evidence_refs")
    if record.get("spec", {}).get("status") != "specified":
        fail(path, f"{record['id']} validated without spec.status = specified")
    if record.get("risk") in HIGH_RISKS:
        for cell in record.get("coverage", {}).get("cells", []):
            if cell.get("state") in {"gap_open", "spec_gap"}:
                fail(
                    path,
                    f"{record['id']} high-risk validated with open coverage "
                    f"cell {cell.get('dimension')}",
                )


def _has_closing_high_risk_evidence(
    invariant: dict[str, Any],
    evidence: dict[str, dict[str, Any]],
    approved_producers: set[str],
) -> bool:
    for evidence_id in invariant.get("evidence_refs", []):
        record = evidence.get(evidence_id)
        if not record:
            continue
        if invariant["id"] not in record.get("linked_invariants", []):
            continue
        if record.get("proof_kind") not in {"green", "lock_compare"}:
            continue
        producer = str(record.get("producer", ""))
        if PROVE_PRODUCER_RE.match(producer) or producer in approved_producers:
            return True
    return False
