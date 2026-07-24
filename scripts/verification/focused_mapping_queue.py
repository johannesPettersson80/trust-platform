"""Lossless queue merging for focused mapping candidate shards."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Mapping, Sequence
from typing import Any

from .mapping_demand_refinement import (
    build_structural_demand_ranking,
    classify_demand_affinity,
    structural_cell_id,
    structural_cell_key,
)


def merge_focused_queue(
    *,
    existing_queue: Mapping[str, Any],
    focused_candidate_document: Mapping[str, Any],
    focused_proposals: Sequence[Mapping[str, Any]],
    selected_discovery_ids: Sequence[str],
    code_mappings: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, Any], list[Mapping[str, Any]]]:
    """Replace only selected discoveries and rebuild aggregate demand views."""

    selected = set(selected_discovery_ids)
    if not selected or len(selected) != len(selected_discovery_ids):
        raise ValueError("focused queue selection must be non-empty and unique")
    existing_demands = _records(
        existing_queue.get("demands", []), "existing demand"
    )
    focused_demands = _records(
        focused_candidate_document.get("demands", []), "focused demand"
    )
    existing_proposals = _records(
        existing_queue.get("proposals", []), "existing proposal"
    )
    new_proposals = _records(focused_proposals, "focused proposal")
    pending_other_batches = sorted(
        _required_string(
            proposal, "discovery_id", "existing proposal"
        )
        for proposal in existing_proposals
        if _required_string(
            proposal, "discovery_id", "existing proposal"
        )
        not in selected
    )
    if pending_other_batches:
        raise ValueError(
            "focused queue has pending proposals from another batch; "
            "review or reject them before generating the next batch: "
            + ", ".join(pending_other_batches)
        )
    _require_exact_focused_scope(
        records=focused_demands + new_proposals,
        selected=selected,
    )

    merged_demands = _replace_selected(
        existing=existing_demands,
        replacements=focused_demands,
        selected=selected,
        label="demand",
    )
    merged_proposals = _replace_selected(
        existing=existing_proposals,
        replacements=new_proposals,
        selected=selected,
        label="proposal",
    )
    observations = _demand_observations(
        demands=merged_demands,
        code_mappings=code_mappings,
    )
    area_by_unmapped_code = _unmapped_code_areas(
        existing_queue.get("unmapped_code_demand_ranking", []),
        focused_candidate_document.get(
            "unmapped_code_demand_ranking", []
        ),
    )
    demand_ranking = _demand_ranking(observations)
    structural_ranking = build_structural_demand_ranking(observations)
    unmapped_ranking = _unmapped_ranking(
        demands=merged_demands,
        area_by_code=area_by_unmapped_code,
    )
    merged_candidate = dict(focused_candidate_document)
    merged_candidate["demands"] = merged_demands
    merged_candidate["spec_demand_observations"] = observations
    merged_candidate["demand_ranking"] = demand_ranking
    merged_candidate["structural_demand_ranking"] = structural_ranking
    merged_candidate["unmapped_code_demand_ranking"] = unmapped_ranking
    return merged_candidate, merged_proposals


def _replace_selected(
    *,
    existing: Sequence[Mapping[str, Any]],
    replacements: Sequence[Mapping[str, Any]],
    selected: set[str],
    label: str,
) -> list[Mapping[str, Any]]:
    indexed: dict[str, Mapping[str, Any]] = {}
    for record in existing:
        discovery_id = _required_string(record, "discovery_id", label)
        if discovery_id in indexed:
            raise ValueError(f"existing queue duplicates {label} {discovery_id}")
        if discovery_id not in selected:
            indexed[discovery_id] = record
    for record in replacements:
        discovery_id = _required_string(record, "discovery_id", label)
        if discovery_id not in selected:
            raise ValueError(
                f"focused {label} escapes selected scope: {discovery_id}"
            )
        if discovery_id in indexed:
            raise ValueError(f"focused queue duplicates {label} {discovery_id}")
        indexed[discovery_id] = record
    return [indexed[key] for key in sorted(indexed)]


def _require_exact_focused_scope(
    *,
    records: Sequence[Mapping[str, Any]],
    selected: set[str],
) -> None:
    seen: set[str] = set()
    for record in records:
        discovery_id = _required_string(
            record, "discovery_id", "focused queue record"
        )
        if discovery_id not in selected:
            raise ValueError(
                f"focused queue record escapes selected scope: {discovery_id}"
            )
        if discovery_id in seen:
            raise ValueError(
                f"focused discovery has both demand and proposal: {discovery_id}"
            )
        seen.add(discovery_id)


def _demand_observations(
    *,
    demands: Sequence[Mapping[str, Any]],
    code_mappings: Mapping[str, Mapping[str, Any]],
) -> list[dict[str, Any]]:
    observations = []
    for demand in demands:
        discovery_id = _required_string(
            demand, "discovery_id", "mapping demand"
        )
        code_by_key: dict[tuple[str, ...], set[str]] = defaultdict(set)
        for code_id in _strings(
            demand.get("covered_code_ids"),
            f"{discovery_id} covered code IDs",
        ):
            mapping = code_mappings.get(code_id)
            if mapping is None or mapping.get("disposition") != "spec_gap":
                continue
            gap_ref = _required_string(
                mapping, "spec_gap_ref", f"{code_id} code mapping"
            )
            code_by_key[
                structural_cell_key(
                    parent_spec_gap_ref=gap_ref,
                    fact=mapping,
                )
            ].add(code_id)
        cells = [
            {
                "cell_id": structural_cell_id(key),
                "parent_spec_gap_ref": key[0],
                "area": key[1],
                "owner": key[2],
                "source_kind": key[3],
                "path": key[4],
                "qualified_symbol_parent": key[5],
                "item_kind": key[6],
                "covered_code_ids": sorted(code_by_key[key]),
            }
            for key in sorted(code_by_key)
        ]
        unmapped = _strings(
            demand.get("unmapped_code_ids"),
            f"{discovery_id} unmapped code IDs",
        )
        affinity = classify_demand_affinity(
            cells=cells,
            unmapped_code_ids=unmapped,
        )
        observations.append(
            {
                "discovery_id": discovery_id,
                "spec_gap_code_ids": sorted(
                    code_id
                    for codes in code_by_key.values()
                    for code_id in codes
                ),
                "unmapped_code_ids": unmapped,
                "cells": cells,
                **affinity,
            }
        )
    return observations


def _demand_ranking(
    observations: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    discovery_by_gap: dict[str, set[str]] = defaultdict(set)
    code_by_gap: dict[str, set[str]] = defaultdict(set)
    for observation in observations:
        discovery_id = _required_string(
            observation, "discovery_id", "demand observation"
        )
        for cell in _records(
            observation.get("cells", []), "demand structural cell"
        ):
            gap_ref = _required_string(
                cell, "parent_spec_gap_ref", "demand structural cell"
            )
            discovery_by_gap[gap_ref].add(discovery_id)
            code_by_gap[gap_ref].update(
                _strings(
                    cell.get("covered_code_ids"),
                    f"{gap_ref} covered code IDs",
                )
            )
    rows = [
        {
            "spec_gap_ref": gap_ref,
            "affected_discovery_ids": sorted(discovery_by_gap[gap_ref]),
            "covered_code_ids": sorted(code_by_gap[gap_ref]),
            "tests_touching": len(discovery_by_gap[gap_ref]),
            "code_facts_touched": len(code_by_gap[gap_ref]),
        }
        for gap_ref in discovery_by_gap
    ]
    return sorted(
        rows,
        key=lambda row: (
            -int(row["tests_touching"]),
            -int(row["code_facts_touched"]),
            str(row["spec_gap_ref"]),
        ),
    )


def _unmapped_code_areas(*rankings: Any) -> dict[str, str]:
    result: dict[str, str] = {}
    for ranking in rankings:
        for row in _records(ranking, "unmapped-code ranking"):
            area = _required_string(row, "area", "unmapped-code ranking")
            for code_id in _strings(
                row.get("covered_code_ids"),
                f"{area} unmapped code IDs",
            ):
                previous = result.get(code_id)
                if previous is not None and previous != area:
                    raise ValueError(
                        f"unmapped code area conflicts for {code_id}"
                    )
                result[code_id] = area
    return result


def _unmapped_ranking(
    *,
    demands: Sequence[Mapping[str, Any]],
    area_by_code: Mapping[str, str],
) -> list[dict[str, Any]]:
    discovery_by_area: dict[str, set[str]] = defaultdict(set)
    code_by_area: dict[str, set[str]] = defaultdict(set)
    for demand in demands:
        discovery_id = _required_string(
            demand, "discovery_id", "mapping demand"
        )
        for code_id in _strings(
            demand.get("unmapped_code_ids"),
            f"{discovery_id} unmapped code IDs",
        ):
            area = area_by_code.get(code_id)
            if area is None:
                raise ValueError(
                    f"unmapped code lacks preserved area authority: {code_id}"
                )
            discovery_by_area[area].add(discovery_id)
            code_by_area[area].add(code_id)
    rows = [
        {
            "area": area,
            "affected_discovery_ids": sorted(discovery_by_area[area]),
            "covered_code_ids": sorted(code_by_area[area]),
            "tests_touching": len(discovery_by_area[area]),
            "code_facts_touched": len(code_by_area[area]),
        }
        for area in discovery_by_area
    ]
    return sorted(
        rows,
        key=lambda row: (
            -int(row["tests_touching"]),
            -int(row["code_facts_touched"]),
            str(row["area"]),
        ),
    )


def _records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} collection must be an array")
    if any(not isinstance(record, Mapping) for record in value):
        raise ValueError(f"{label} rows must be objects")
    return list(value)


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must contain nonempty strings")
    return sorted(set(value))


def _required_string(
    record: Mapping[str, Any], field: str, label: str
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires {field}")
    return value
