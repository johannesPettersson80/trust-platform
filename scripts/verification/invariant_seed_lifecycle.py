"""Reviewed lifecycle rules for canonical invariants imported by Phase 4."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS
from .metadata_validator.promotion_evidence import (
    PROMOTED_PROOF_LEVELS,
    validate_invariant_promotion_evidence,
)


LIFECYCLE_VERSION = 1
BASELINE = "baseline"
EXECUTION_READY = "execution_ready"
LIFECYCLE_STATES = {BASELINE, EXECUTION_READY}
EXECUTION_READY_SEED_IDS = {
    "IEC_TIMER_001",
    "RT_SAFE_DEADLINE_001",
    "RT_SAFE_FORCE_001",
    "RT_SAFE_IO_001",
    "RT_SAFE_NAN_001",
    "RT_SAFE_PANIC_001",
    "RT_SAFE_RETAIN_001",
    "RT_SAFE_RESTART_001",
    "RT_SAFE_STOP_001",
    "RT_RELOAD_001",
    "VM_SEAM_ENC_001",
}
BASELINE_STATUSES = {"gap_open", "spec_gap"}
EXECUTION_STATUSES = {
    "gap_open",
    "spec_gap",
    "test_written",
    "implemented",
    "validated",
}
NON_CLAIM_AUTHORITIES = {
    "normative_external",
    "normative_product",
    "reviewed_decision",
    "reviewed_deviation",
}


def validate_seed_lifecycle(
    *,
    seed_id: str,
    canonical_id: str,
    origin: str,
    lifecycle_version: object,
    lifecycle_state: object,
    invariant: Mapping[str, Any],
    invariant_path: str,
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    suites: Mapping[str, Mapping[str, Any]],
    ignored_tests: Mapping[str, Mapping[str, Any]] | None = None,
) -> None:
    """Keep the import audit immutable while allowing one reviewed execution pilot."""

    prefix = seed_id
    if type(lifecycle_version) is not int or lifecycle_version != LIFECYCLE_VERSION:
        raise ValueError(f"{prefix}: lifecycle_version must be {LIFECYCLE_VERSION}")
    if not isinstance(lifecycle_state, str) or lifecycle_state not in LIFECYCLE_STATES:
        raise ValueError(f"{prefix}: unknown lifecycle_state {lifecycle_state!r}")
    if seed_id in EXECUTION_READY_SEED_IDS:
        if lifecycle_state != EXECUTION_READY:
            raise ValueError(f"{prefix}: reviewed execution seed must use execution_ready")
    elif lifecycle_state != BASELINE:
        raise ValueError(f"{prefix}: seed is not reviewed for execution_ready")

    spec = invariant.get("spec")
    oracle = invariant.get("oracle")
    coverage = invariant.get("coverage")
    if not isinstance(spec, Mapping) or not isinstance(oracle, Mapping) or not isinstance(
        coverage, Mapping
    ):
        raise ValueError(f"{prefix}: invariant requires spec, oracle, and coverage objects")
    source_refs = _string_list(spec.get("source_refs"), f"{prefix}: spec.source_refs")
    unknown_sources = sorted(set(source_refs) - set(spec_sources))
    if unknown_sources:
        raise ValueError(
            f"{prefix}: spec.source_refs are unknown: {', '.join(unknown_sources)}"
        )
    cells = coverage.get("cells")
    if not isinstance(cells, list) or not cells:
        raise ValueError(f"{prefix}: coverage.cells must be non-empty")
    oracle_ref = oracle.get("ref")
    if not isinstance(oracle_ref, str) or not oracle_ref:
        raise ValueError(f"{prefix}: oracle.ref must be a non-empty string")

    if lifecycle_state == BASELINE:
        _validate_baseline(
            prefix=prefix,
            canonical_id=canonical_id,
            origin=origin,
            invariant=invariant,
            source_refs=source_refs,
            oracle=oracle,
            cells=cells,
            spec_sources=spec_sources,
            spec_gaps=spec_gaps,
            tests=tests,
            evidence=evidence,
        )
        return

    _validate_execution_ready(
        prefix=prefix,
        canonical_id=canonical_id,
        invariant=invariant,
        invariant_path=invariant_path,
        source_refs=source_refs,
        oracle=oracle,
        cells=cells,
        spec_sources=spec_sources,
        spec_gaps=spec_gaps,
        tests=tests,
        evidence=evidence,
        suites=suites,
        ignored_tests=ignored_tests or {},
    )


def _validate_baseline(
    *,
    prefix: str,
    canonical_id: str,
    origin: str,
    invariant: Mapping[str, Any],
    source_refs: list[str],
    oracle: Mapping[str, Any],
    cells: list[Any],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
) -> None:
    status = invariant.get("status")
    if not isinstance(status, str) or status not in BASELINE_STATUSES:
        raise ValueError(f"{prefix}: baseline status must be gap_open or spec_gap")
    if invariant.get("proof_level") != "S0":
        raise ValueError(f"{prefix}: baseline proof_level must remain S0")
    test_ids = _string_list(invariant.get("tests"), f"{prefix}: tests")
    evidence_ids = _string_list(
        invariant.get("evidence_refs"), f"{prefix}: evidence_refs"
    )
    if origin == "phase4" and test_ids:
        raise ValueError(f"{prefix}: baseline phase4 seed must retain empty tests")
    if origin == "phase4" and evidence_ids:
        raise ValueError(
            f"{prefix}: baseline phase4 seed must retain empty evidence_refs"
        )
    spec_gap_refs = _string_list(
        invariant.get("spec_gap_refs", []), f"{prefix}: spec_gap_refs"
    )
    oracle_ref = oracle["ref"]
    if status == "gap_open":
        if not source_refs or oracle_ref not in source_refs:
            raise ValueError(f"{prefix}: every gap_open seed requires its written oracle source")
        if spec_gap_refs:
            raise ValueError(f"{prefix}: gap_open invariant cannot claim spec gaps")
        _validate_oracle_source(prefix, oracle_ref, spec_sources)
        if invariant["spec"].get("status") != "specified":
            raise ValueError(f"{prefix}: gap_open invariant requires spec.status specified")
        for cell in cells:
            if (
                not isinstance(cell, Mapping)
                or cell.get("state") != "gap_open"
                or "spec_gap_ref" in cell
            ):
                raise ValueError(
                    f"{prefix}: gap_open coverage cells must remain gap_open without a spec gap"
                )
    else:
        _validate_open_gap_posture(
            prefix=prefix,
            canonical_id=canonical_id,
            oracle_ref=oracle_ref,
            spec_gap_refs=spec_gap_refs,
            cells=cells,
            spec_gaps=spec_gaps,
            allowed_resolutions={"open"},
        )
    for evidence_id in evidence_ids:
        item = evidence.get(evidence_id)
        if item is None or item.get("proof_kind") != "none":
            raise ValueError(
                f"{prefix}: baseline evidence must use proof_kind none: {evidence_id}"
            )
    for test_id in test_ids:
        item = tests.get(test_id)
        if item is None or item.get("spec_gap_ref") not in spec_gap_refs:
            raise ValueError(
                f"{prefix}: baseline test must remain bound to an open invariant spec gap: {test_id}"
            )


def _validate_execution_ready(
    *,
    prefix: str,
    canonical_id: str,
    invariant: Mapping[str, Any],
    invariant_path: str,
    source_refs: list[str],
    oracle: Mapping[str, Any],
    cells: list[Any],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    suites: Mapping[str, Mapping[str, Any]],
    ignored_tests: Mapping[str, Mapping[str, Any]],
) -> None:
    status = invariant.get("status")
    if not isinstance(status, str) or status not in EXECUTION_STATUSES:
        raise ValueError(
            f"{prefix}: execution_ready status must be gap_open, spec_gap, "
            "test_written, implemented, or validated"
        )
    proof_level = invariant.get("proof_level")
    if not isinstance(proof_level, str) or (
        proof_level != "S0" and proof_level not in PROMOTED_PROOF_LEVELS
    ):
        raise ValueError(
            f"{prefix}: execution_ready proof_level must be S0, G1, G2, or R1"
        )

    test_ids = _string_list(invariant.get("tests"), f"{prefix}: tests")
    evidence_ids = _string_list(
        invariant.get("evidence_refs"), f"{prefix}: evidence_refs"
    )
    spec_gap_refs = _string_list(
        invariant.get("spec_gap_refs", []), f"{prefix}: spec_gap_refs"
    )
    if status == "validated":
        if proof_level not in PROMOTED_PROOF_LEVELS:
            raise ValueError(
                f"{prefix}: validated requires promoted proof_level G1, G2, or R1"
            )
        if invariant.get("spec", {}).get("status") != "specified":
            raise ValueError(f"{prefix}: validated requires spec.status specified")
        if spec_gap_refs:
            raise ValueError(f"{prefix}: validated requires no current spec gaps")
        if not test_ids or not evidence_ids:
            raise ValueError(
                f"{prefix}: validated requires linked tests and evidence"
            )
        missing = _string_list(invariant.get("missing", []), f"{prefix}: missing")
        if missing:
            raise ValueError(f"{prefix}: validated requires no missing obligations")
        if any(
            not isinstance(cell, Mapping)
            or cell.get("state") not in {"covered", "covered_by_fuzz", "not_applicable"}
            for cell in cells
        ):
            raise ValueError(
                f"{prefix}: validated requires every coverage cell to be closed"
            )
    oracle_ref = oracle["ref"]
    _validate_current_gaps(
        prefix=prefix,
        canonical_id=canonical_id,
        status=str(status),
        oracle_ref=oracle_ref,
        spec_gap_refs=spec_gap_refs,
        cells=cells,
        spec_gaps=spec_gaps,
    )
    if status == "gap_open":
        if spec_gap_refs:
            raise ValueError(f"{prefix}: gap_open invariant cannot claim spec gaps")
        if oracle_ref not in source_refs:
            raise ValueError(f"{prefix}: gap_open seed requires its written oracle source")
        _validate_oracle_source(prefix, oracle_ref, spec_sources)
    elif status != "spec_gap":
        if oracle_ref not in source_refs:
            raise ValueError(
                f"{prefix}: execution state {status} requires a reviewed source oracle"
            )
        _validate_oracle_source(prefix, oracle_ref, spec_sources)

    for test_id in test_ids:
        item = tests.get(test_id)
        if item is None:
            raise ValueError(f"{prefix}: execution_ready references unknown test {test_id}")
        if canonical_id not in _string_list(
            item.get("invariants", []), f"{prefix}: test {test_id}.invariants"
        ):
            raise ValueError(
                f"{prefix}: execution_ready test does not link canonical invariant: {test_id}"
            )
        test_gap = item.get("spec_gap_ref")
        if test_gap is not None and test_gap not in spec_gap_refs:
            raise ValueError(
                f"{prefix}: execution_ready test names a gap not currently listed by "
                f"the invariant: {test_id}"
            )
    for evidence_id in evidence_ids:
        item = evidence.get(evidence_id)
        if item is None:
            raise ValueError(
                f"{prefix}: execution_ready references unknown evidence {evidence_id}"
            )
        if canonical_id not in _string_list(
            item.get("linked_invariants", []),
            f"{prefix}: evidence {evidence_id}.linked_invariants",
        ):
            raise ValueError(
                f"{prefix}: execution_ready evidence does not link canonical invariant: "
                f"{evidence_id}"
            )

    promotion_failures: list[str] = []
    validate_invariant_promotion_evidence(
        fail=lambda _path, message: promotion_failures.append(message),
        path=Path(invariant_path),
        invariant=invariant,
        evidence=evidence,
        suites=suites,
        tests=tests,
        ignored_tests=ignored_tests,
    )
    if promotion_failures:
        raise ValueError(f"{prefix}: " + "; ".join(sorted(set(promotion_failures))))


def _validate_current_gaps(
    *,
    prefix: str,
    canonical_id: str,
    status: str,
    oracle_ref: object,
    spec_gap_refs: list[str],
    cells: list[Any],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> None:
    for gap_id in spec_gap_refs:
        gap = spec_gaps.get(gap_id)
        if gap is None or gap.get("resolution_status") not in OPEN_GAP_RESOLUTIONS:
            raise ValueError(f"{prefix}: current spec gap is not open/actionable: {gap_id}")
        if canonical_id not in gap.get("affected_invariants", []):
            raise ValueError(
                f"{prefix}: current spec gap must name the canonical invariant: {gap_id}"
            )
    if status == "spec_gap" and oracle_ref not in spec_gap_refs:
        raise ValueError(f"{prefix}: spec_gap oracle must reference a listed current gap")
    if status != "spec_gap" and oracle_ref in spec_gaps:
        raise ValueError(f"{prefix}: non-spec-gap execution state cannot use a gap oracle")
    for cell in cells:
        if not isinstance(cell, Mapping):
            raise ValueError(f"{prefix}: coverage cell must be an object")
        gap_id = cell.get("spec_gap_ref")
        if cell.get("state") == "spec_gap":
            if gap_id not in spec_gap_refs:
                raise ValueError(
                    f"{prefix}: spec_gap coverage cell must name a listed current gap"
                )
        elif gap_id is not None:
            raise ValueError(
                f"{prefix}: only a spec_gap coverage cell may name spec_gap_ref"
            )


def _validate_open_gap_posture(
    *,
    prefix: str,
    canonical_id: str,
    oracle_ref: object,
    spec_gap_refs: list[str],
    cells: list[Any],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    allowed_resolutions: set[str],
) -> None:
    if not spec_gap_refs or oracle_ref not in spec_gap_refs:
        raise ValueError(f"{prefix}: spec_gap oracle must reference a listed focused gap")
    focused_gap_ids = {str(oracle_ref)}
    for gap_id in spec_gap_refs:
        gap = spec_gaps.get(gap_id)
        if gap is None or gap.get("resolution_status") not in allowed_resolutions:
            raise ValueError(f"{prefix}: spec gap must remain open: {gap_id}")
    for cell in cells:
        gap_id = cell.get("spec_gap_ref") if isinstance(cell, Mapping) else None
        if (
            not isinstance(cell, Mapping)
            or cell.get("state") != "spec_gap"
            or gap_id not in spec_gap_refs
            or gap_id not in spec_gaps
            or spec_gaps[gap_id].get("resolution_status") not in allowed_resolutions
        ):
            raise ValueError(
                f"{prefix}: coverage spec_gap_ref must use a listed open focused gap"
            )
        focused_gap_ids.add(str(gap_id))
    for gap_id in focused_gap_ids:
        if canonical_id not in spec_gaps[gap_id].get("affected_invariants", []):
            raise ValueError(
                f"{prefix}: focused spec gap must name the canonical invariant: {gap_id}"
            )


def _validate_oracle_source(
    prefix: str,
    oracle_ref: object,
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> None:
    source = spec_sources.get(str(oracle_ref))
    if (
        source is None
        or source.get("source_status") != "active"
        or source.get("authority") not in NON_CLAIM_AUTHORITIES
    ):
        raise ValueError(
            f"{prefix}: oracle must use an active normative or reviewed source"
        )
    if source.get("oracle_eligible") is not True:
        raise ValueError(f"{prefix}: oracle source is provenance-only")


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or not all(
        isinstance(item, str) and item for item in value
    ):
        raise ValueError(f"{label} must be a string array")
    if len(value) != len(set(value)):
        raise ValueError(f"{label} must be duplicate-free")
    return value
