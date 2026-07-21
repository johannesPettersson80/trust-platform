"""Pure Phase 6 requirement-to-invariant and oracle-debt analysis."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .metadata_validator.constants import HIGH_RISKS
from .metadata_validator.integrity import UNRESOLVED_GAP_RESOLUTIONS
from .metadata_validator.oracle_refs import ORACLE_AUTHORITIES


FUTURE_ENFORCEMENT_RISKS = frozenset(HIGH_RISKS)
MAPPING_GROUP_AREAS = {
    "VERIF-P6-001": ("compiler_iec",),
    "VERIF-P6-002": ("runtime_safety",),
    "VERIF-P6-003": ("protocols",),
    "VERIF-P6-004": ("editor_safety",),
    "VERIF-P6-005": ("control_security", "supply_chain_platform"),
}


def analyze_requirement_oracles(
    *,
    invariants: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Return explicit mappings and debt without inferring behavior from prose."""

    rows = [
        _invariant_row(
            invariant_id,
            record,
            spec_sources=spec_sources,
            spec_gaps=spec_gaps,
        )
        for invariant_id, record in sorted(invariants.items())
    ]
    groups = _mapping_groups(rows)
    missing = [row for row in rows if row["oracle_state"] == "spec_gap_blocked"]
    eligible = [row for row in rows if row["oracle_state"] == "eligible_oracle"]
    mapped_ids = {
        invariant_id
        for group in groups
        for invariant_id in group["invariant_ids"]
    }
    return {
        "mapping_groups": groups,
        "invariants": rows,
        "missing_oracles": missing,
        "summary": {
            "invariants_total": len(rows),
            "mapped_phase6_invariants": len(mapped_ids),
            "other_area_invariants": len(rows) - len(mapped_ids),
            "eligible_oracles": len(eligible),
            "missing_oracles": len(missing),
            "future_enforcement_candidates": sum(
                row["future_enforcement_candidate"] for row in rows
            ),
        },
    }


def _invariant_row(
    invariant_id: str,
    record: Mapping[str, Any],
    *,
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    if record.get("id") != invariant_id:
        raise ValueError(f"invariant index key {invariant_id!r} does not match record id")
    area = _required_string(record, "area", invariant_id)
    risk = _required_string(record, "risk", invariant_id)
    invariant_status = _required_string(record, "status", invariant_id)
    proof_level = _required_string(record, "proof_level", invariant_id)
    spec = record.get("spec")
    oracle = record.get("oracle")
    if not isinstance(spec, Mapping) or not isinstance(oracle, Mapping):
        raise ValueError(f"{invariant_id} requires spec and oracle tables")

    spec_status = _required_string(spec, "status", invariant_id)
    oracle_kind = _required_string(oracle, "kind", invariant_id)
    oracle_ref = _required_string(oracle, "ref", invariant_id)
    spec_source_refs = _string_list(spec.get("source_refs", []), invariant_id, "spec.source_refs")
    spec_gap_refs = _string_list(record.get("spec_gap_refs", []), invariant_id, "spec_gap_refs")
    tests = _string_list(record.get("tests", []), invariant_id, "tests")
    gates = _string_list(record.get("gates", []), invariant_id, "gates")
    evidence_refs = _string_list(record.get("evidence_refs", []), invariant_id, "evidence_refs")

    eligible_context: list[str] = []
    public_claims: list[str] = []
    for source_ref in spec_source_refs:
        source = spec_sources.get(source_ref)
        if source is None:
            raise ValueError(f"{invariant_id} references unknown spec source {source_ref}")
        if source.get("authority") == "public_claim":
            public_claims.append(source_ref)
        elif (
            source.get("source_status") == "active"
            and source.get("oracle_eligible") is True
            and source.get("authority") in ORACLE_AUTHORITIES
        ):
            eligible_context.append(source_ref)

    oracle_source_authority: str | None = None
    if oracle_ref in spec_gaps:
        gap = spec_gaps[oracle_ref]
        if invariant_status != "spec_gap":
            raise ValueError(f"{invariant_id} gap oracle requires status = spec_gap")
        if oracle_ref not in spec_gap_refs:
            raise ValueError(f"{invariant_id} gap oracle is not attached through spec_gap_refs")
        if (
            gap.get("status") != "spec_gap"
            or gap.get("resolution_status") not in UNRESOLVED_GAP_RESOLUTIONS
        ):
            raise ValueError(f"{invariant_id} gap oracle must name an unresolved spec gap")
        oracle_state = "spec_gap_blocked"
    else:
        if invariant_status == "spec_gap":
            raise ValueError(f"{invariant_id} spec_gap status requires a gap oracle")
        source_id = oracle_ref.split("#", 1)[0]
        source = spec_sources.get(source_id)
        if source is None:
            raise ValueError(f"{invariant_id} oracle references unknown source {source_id}")
        authority = source.get("authority")
        if authority == "public_claim":
            raise ValueError(f"public_claim source {source_id} cannot be an oracle for {invariant_id}")
        if source.get("source_status") != "active":
            raise ValueError(f"{invariant_id} oracle source {source_id} is not active")
        if source.get("oracle_eligible") is not True:
            raise ValueError(f"{invariant_id} oracle source {source_id} is provenance-only")
        if authority not in ORACLE_AUTHORITIES:
            raise ValueError(f"{invariant_id} oracle source {source_id} has ineligible authority")
        oracle_source_authority = str(authority)
        oracle_state = "eligible_oracle"

    board_row = board_row_for_area(area)
    future_candidate = (
        oracle_state == "spec_gap_blocked" and risk in FUTURE_ENFORCEMENT_RISKS
    )
    return {
        "invariant_id": invariant_id,
        "area": area,
        "risk": risk,
        "invariant_status": invariant_status,
        "proof_level": proof_level,
        "spec_status": spec_status,
        "mapping_board_row": board_row,
        "oracle_kind": oracle_kind,
        "oracle_ref": oracle_ref,
        "oracle_state": oracle_state,
        "oracle_source_authority": oracle_source_authority,
        "spec_source_refs": spec_source_refs,
        "eligible_context_source_refs": eligible_context,
        "public_claim_source_refs": public_claims,
        "spec_gap_refs": spec_gap_refs,
        "tests": tests,
        "gates": gates,
        "evidence_refs": evidence_refs,
        "future_enforcement_candidate": future_candidate,
    }


def _mapping_groups(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: list[dict[str, Any]] = []
    for board_row, area_ids in MAPPING_GROUP_AREAS.items():
        selected = [row for row in rows if row["area"] in area_ids]
        eligible = sum(row["oracle_state"] == "eligible_oracle" for row in selected)
        blocked = sum(row["oracle_state"] == "spec_gap_blocked" for row in selected)
        groups.append(
            {
                "board_row": board_row,
                "area_ids": list(area_ids),
                "invariant_count": len(selected),
                "invariant_ids": [row["invariant_id"] for row in selected],
                "eligible_oracle_count": eligible,
                "spec_gap_blocked_count": blocked,
            }
        )
    return groups


def board_row_for_area(area: str) -> str | None:
    for board_row, area_ids in MAPPING_GROUP_AREAS.items():
        if area in area_ids:
            return board_row
    return None


def _required_string(record: Mapping[str, Any], field: str, owner_id: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{owner_id} {field} must be a non-empty string")
    return value


def _string_list(value: Any, owner_id: str, field: str) -> list[str]:
    if not isinstance(value, list) or not all(
        isinstance(item, str) and item for item in value
    ):
        raise ValueError(f"{owner_id} {field} must be a string array")
    if len(value) != len(set(value)):
        raise ValueError(f"{owner_id} {field} must not contain duplicates")
    return list(value)
