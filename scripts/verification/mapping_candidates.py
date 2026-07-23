"""Execution-guided test-catalog candidate ranking.

Coverage and mutation observations are candidate-ranking inputs only. This
module never writes catalog metadata or promotes verification proof.
"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Mapping, Sequence
from typing import Any


OBSERVED = "observed"
SPECIFIED = "specified"
SPEC_GAP = "spec_gap"
CANDIDATE_TIERS = ("C1", "C2", "C3", "C4")
DOMINANT_CONCENTRATION_PPM = 800_000


def analyze_mapping_candidates(
    *,
    trace_records: Sequence[Mapping[str, Any]],
    code_mappings: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    mutation_kills: Sequence[Mapping[str, Any]] = (),
    live_code_facts: Mapping[str, Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    """Return deterministic candidate and specification-demand rows."""

    kills_by_test_and_invariant: dict[tuple[str, str], set[str]] = defaultdict(set)
    for kill in mutation_kills:
        if kill.get("outcome") != "killed":
            continue
        discovery_id = _required_string(kill, "discovery_id", "mutation kill")
        invariant_id = _required_string(kill, "invariant_id", "mutation kill")
        mutation_id = _required_string(kill, "mutation_id", "mutation kill")
        kills_by_test_and_invariant[(discovery_id, invariant_id)].add(mutation_id)

    candidates: list[dict[str, Any]] = []
    demands: list[dict[str, Any]] = []
    authoritative_live_facts = (
        code_mappings if live_code_facts is None else live_code_facts
    )
    unmapped_discovery_by_area: dict[str, set[str]] = defaultdict(set)
    unmapped_code_by_area: dict[str, set[str]] = defaultdict(set)
    seen_discovery_ids: set[str] = set()
    for trace in sorted(trace_records, key=lambda row: str(row.get("discovery_id", ""))):
        discovery_id = _required_string(trace, "discovery_id", "trace record")
        if discovery_id in seen_discovery_ids:
            raise ValueError(f"duplicate trace record for {discovery_id}")
        seen_discovery_ids.add(discovery_id)
        if trace.get("trace_status") != OBSERVED:
            continue
        covered_code_ids = _sorted_unique_strings(
            trace.get("covered_code_ids"), f"{discovery_id} covered_code_ids"
        )
        unknown_code_ids = sorted(
            set(covered_code_ids) - set(authoritative_live_facts)
        )
        if unknown_code_ids:
            raise ValueError(
                f"{discovery_id} trace references code IDs absent from live code "
                f"inventory: {unknown_code_ids}"
            )
        unmapped_code_ids = sorted(set(covered_code_ids) - set(code_mappings))
        for code_id in unmapped_code_ids:
            area = _required_string(
                authoritative_live_facts[code_id],
                "area",
                f"{code_id} live code fact",
            )
            unmapped_discovery_by_area[area].add(discovery_id)
            unmapped_code_by_area[area].add(code_id)

        specified_code_ids: list[str] = []
        spec_gap_code_ids: list[str] = []
        spec_gap_refs: set[str] = set()
        invariant_code_ids: dict[str, set[str]] = defaultdict(set)
        for code_id in covered_code_ids:
            mapping = code_mappings.get(code_id)
            if mapping is None:
                continue
            disposition = mapping.get("disposition")
            if disposition == SPECIFIED:
                specified_code_ids.append(code_id)
                invariant_ids = _sorted_unique_strings(
                    mapping.get("invariant_ids"), f"{code_id} invariant_ids"
                )
                if not invariant_ids:
                    raise ValueError(f"{code_id} specified mapping lacks invariant_ids")
                for invariant_id in invariant_ids:
                    if invariant_id not in invariants:
                        raise ValueError(f"{code_id} references unknown invariant {invariant_id}")
                    invariant_code_ids[invariant_id].add(code_id)
            elif disposition == SPEC_GAP:
                spec_gap_code_ids.append(code_id)
                spec_gap_refs.add(
                    _required_string(mapping, "spec_gap_ref", f"{code_id} spec-gap mapping")
                )

        candidate_invariant_ids = sorted(invariant_code_ids)
        if not candidate_invariant_ids:
            if spec_gap_code_ids or unmapped_code_ids:
                demands.append(
                    {
                        "discovery_id": discovery_id,
                        "candidate_tier": "C4",
                        "covered_code_ids": covered_code_ids,
                        "spec_gap_refs": sorted(spec_gap_refs),
                        "unmapped_code_ids": unmapped_code_ids,
                    }
                )
            continue

        mutation_kill_ids = sorted(
            {
                mutation_id
                for invariant_id in candidate_invariant_ids
                for mutation_id in kills_by_test_and_invariant[
                    (discovery_id, invariant_id)
                ]
            }
        )
        concentrations = {
            invariant_id: (
                len(invariant_code_ids[invariant_id]) * 1_000_000
                // max(1, len(specified_code_ids))
            )
            for invariant_id in candidate_invariant_ids
        }
        if len(candidate_invariant_ids) > 1:
            tier = "C3"
        elif mutation_kill_ids or (
            len(specified_code_ids) >= 2
            and concentrations[candidate_invariant_ids[0]]
            >= DOMINANT_CONCENTRATION_PPM
        ):
            tier = "C1"
        else:
            tier = "C2"
        candidates.append(
            {
                "discovery_id": discovery_id,
                "candidate_tier": tier,
                "candidate_invariant_ids": candidate_invariant_ids,
                "covered_code_ids": covered_code_ids,
                "specified_code_ids": specified_code_ids,
                "spec_gap_code_ids": spec_gap_code_ids,
                "spec_gap_refs": sorted(spec_gap_refs),
                "unmapped_code_ids": unmapped_code_ids,
                "concentration_ppm": {
                    invariant_id: concentrations[invariant_id]
                    for invariant_id in candidate_invariant_ids
                },
                "mutation_kill_ids": mutation_kill_ids,
            }
        )
    return {
        "candidates": candidates,
        "demands": demands,
        "demand_ranking": _build_demand_ranking(
            demands=demands,
            code_mappings=code_mappings,
        ),
        "unmapped_code_demand_ranking": [
            {
                "area": area,
                "affected_discovery_ids": sorted(
                    unmapped_discovery_by_area[area]
                ),
                "covered_code_ids": sorted(unmapped_code_by_area[area]),
                "tests_affected": len(unmapped_discovery_by_area[area]),
                "code_facts_missing": len(unmapped_code_by_area[area]),
            }
            for area in sorted(
                unmapped_code_by_area,
                key=lambda item: (
                    -len(unmapped_discovery_by_area[item]),
                    -len(unmapped_code_by_area[item]),
                    item,
                ),
            )
        ],
    }


def _build_demand_ranking(
    *,
    demands: Sequence[Mapping[str, Any]],
    code_mappings: Mapping[str, Mapping[str, Any]],
) -> list[dict[str, Any]]:
    discovery_by_gap: dict[str, set[str]] = defaultdict(set)
    code_by_gap: dict[str, set[str]] = defaultdict(set)
    for demand in demands:
        discovery_id = _required_string(demand, "discovery_id", "demand")
        gap_refs = _sorted_unique_strings(
            demand.get("spec_gap_refs"), f"{discovery_id} spec_gap_refs"
        )
        for gap_ref in gap_refs:
            discovery_by_gap[gap_ref].add(discovery_id)
        for code_id in _sorted_unique_strings(
            demand.get("covered_code_ids"), f"{discovery_id} covered_code_ids"
        ):
            mapping = code_mappings.get(code_id)
            if mapping is None or mapping.get("disposition") != SPEC_GAP:
                continue
            gap_ref = _required_string(
                mapping, "spec_gap_ref", f"{code_id} spec-gap mapping"
            )
            if gap_ref in gap_refs:
                code_by_gap[gap_ref].add(code_id)
    rows = [
        {
            "spec_gap_ref": gap_ref,
            "affected_discovery_ids": sorted(discovery_ids),
            "covered_code_ids": sorted(code_by_gap[gap_ref]),
            "tests_unblocked": len(discovery_ids),
            "code_facts_unblocked": len(code_by_gap[gap_ref]),
        }
        for gap_ref, discovery_ids in discovery_by_gap.items()
    ]
    return sorted(
        rows,
        key=lambda row: (
            -int(row["tests_unblocked"]),
            -int(row["code_facts_unblocked"]),
            str(row["spec_gap_ref"]),
        ),
    )


def _required_string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _sorted_unique_strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array of strings")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must be an array of non-empty strings")
    return sorted(set(value))
