"""Registered public-claim proof-or-gap traceability checks."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any, Callable


Fail = Callable[[Path, str], None]
OPEN_GAP_RESOLUTIONS = {"open", "decision_recorded", "spec_updated", "test_mapped"}
CLOSING_PROOF_KINDS = {"green", "lock_compare"}


def validate_public_claim_records(
    *,
    fail: Fail,
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    required_specs: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
) -> None:
    """Require each registered claim to retain a proof path or visible gap."""

    referenced_sources = {
        source_id
        for gap in spec_gaps.values()
        for source_id in gap.get("candidate_spec_sources", [])
    }
    referenced_sources.update(
        source_id
        for invariant in invariants.values()
        for source_id in invariant.get("spec", {}).get("source_refs", [])
    )
    referenced_sources.update(
        source_ref
        for required in required_specs.values()
        if isinstance(source_ref := required.get("source_ref"), str)
    )

    open_gap_ids = {
        gap_id
        for gap_id, gap in spec_gaps.items()
        if gap.get("resolution_status") in OPEN_GAP_RESOLUTIONS
    }
    for source_id, source in spec_sources.items():
        if source.get("authority") != "public_claim":
            continue
        if source_id not in referenced_sources:
            fail(
                source["_path"],
                f"public claim {source_id} has no invariant, required-spec, "
                "or spec-gap reference",
            )
        linked_invariants = {
            invariant_id: invariant
            for invariant_id, invariant in invariants.items()
            if source_id in invariant.get("spec", {}).get("source_refs", [])
        }
        gap_backed = any(
            source_id in gap.get("candidate_spec_sources", [])
            and gap.get("resolution_status") in OPEN_GAP_RESOLUTIONS
            for gap in spec_gaps.values()
        ) or any(
            _has_explicit_gap(invariant, open_gap_ids)
            for invariant in linked_invariants.values()
        )
        proof_backed = any(
            _has_closing_proof(invariant_id, invariant, evidence)
            for invariant_id, invariant in linked_invariants.items()
        )
        if not gap_backed and not proof_backed:
            fail(
                source["_path"],
                f"public claim {source_id} has no proof-backed invariant or "
                "explicit spec gap",
            )


def _has_closing_proof(
    invariant_id: str,
    invariant: Mapping[str, Any],
    evidence: Mapping[str, Mapping[str, Any]],
) -> bool:
    if invariant.get("status") != "validated":
        return False
    return any(
        evidence_id in evidence
        and evidence[evidence_id].get("proof_kind") in CLOSING_PROOF_KINDS
        and invariant_id in evidence[evidence_id].get("linked_invariants", [])
        for evidence_id in invariant.get("evidence_refs", [])
    )


def _has_explicit_gap(
    invariant: Mapping[str, Any], open_gap_ids: set[str]
) -> bool:
    if invariant.get("status") in {"gap_open", "spec_gap"}:
        return True
    spec_gap_refs = invariant.get("spec_gap_refs", [])
    if isinstance(spec_gap_refs, list) and any(
        gap_id in open_gap_ids for gap_id in spec_gap_refs
    ):
        return True
    coverage = invariant.get("coverage", {})
    cells = coverage.get("cells", []) if isinstance(coverage, Mapping) else []
    if not isinstance(cells, list):
        cells = []
    if any(
        isinstance(cell, Mapping)
        and (
            cell.get("state") in {"gap_open", "spec_gap"}
            or cell.get("spec_gap_ref") in open_gap_ids
        )
        for cell in cells
    ):
        return True
    behavior = invariant.get("behavior", [])
    if not isinstance(behavior, list):
        behavior = []
    return any(
        isinstance(row, Mapping) and row.get("spec_gap_ref") in open_gap_ids
        for row in behavior
    )
