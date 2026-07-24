"""Deterministic structural refinement of specification-demand evidence.

The functions in this module group and rank exact execution/code-fact joins.
They do not infer behavior, create invariant authority, or promote proof.
"""

from __future__ import annotations

import hashlib
import json
from collections import defaultdict
from collections.abc import Mapping, Sequence
from typing import Any


OBSERVED = "observed"
SPEC_GAP = "spec_gap"
DEFAULT_DOMINANT_CONCENTRATION_PPM = 800_000
_CELL_KEY_FIELDS = (
    "parent_spec_gap_ref",
    "area",
    "owner",
    "source_kind",
    "path",
    "qualified_symbol_parent",
    "item_kind",
)


def structural_cell_key(
    *, parent_spec_gap_ref: str, fact: Mapping[str, Any]
) -> tuple[str, ...]:
    """Return a mechanical source-structure key for one spec-gap fact."""

    qualified_symbol = _required_string(fact, "qualified_symbol", "code fact")
    return (
        _nonempty_string(parent_spec_gap_ref, "parent spec-gap reference"),
        _required_string(fact, "area", "code fact"),
        _required_string(fact, "owner", "code fact"),
        _required_string(fact, "source_kind", "code fact"),
        _required_string(fact, "path", "code fact"),
        _qualified_symbol_parent(qualified_symbol),
        _required_string(fact, "item_kind", "code fact"),
    )


def structural_cell_id(key: Sequence[str]) -> str:
    """Return a stable generated identity for a structural cell key."""

    normalized = tuple(
        _nonempty_string(value, f"structural cell key field {index}")
        if index != 5
        else _string(value, f"structural cell key field {index}")
        for index, value in enumerate(key)
    )
    if len(normalized) != len(_CELL_KEY_FIELDS):
        raise ValueError(
            f"structural cell key requires {len(_CELL_KEY_FIELDS)} fields"
        )
    payload = json.dumps(
        normalized,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode()
    return f"DEMAND_CELL_{hashlib.sha256(payload).hexdigest().upper()}"


def classify_demand_affinity(
    *,
    cells: Sequence[Mapping[str, Any]],
    unmapped_code_ids: Sequence[str],
    dominant_concentration_ppm: int = DEFAULT_DOMINANT_CONCENTRATION_PPM,
) -> dict[str, Any]:
    """Classify locality without treating locality as semantic authority."""

    _validate_concentration_threshold(dominant_concentration_ppm)
    unmapped = _sorted_unique_strings(unmapped_code_ids, "unmapped_code_ids")
    counts: list[tuple[int, str]] = []
    seen_cell_ids: set[str] = set()
    for cell in cells:
        cell_id = _required_string(cell, "cell_id", "structural cell")
        if cell_id in seen_cell_ids:
            raise ValueError(f"duplicate structural cell {cell_id}")
        seen_cell_ids.add(cell_id)
        covered_code_ids = _sorted_unique_strings(
            cell.get("covered_code_ids"),
            f"{cell_id} covered_code_ids",
        )
        if not covered_code_ids:
            raise ValueError(f"{cell_id} structural cell has no covered code facts")
        counts.append((len(covered_code_ids), cell_id))

    if not counts:
        return {
            "affinity_tier": "R3",
            "dominant_cell_id": "",
            "dominant_concentration_ppm": 0,
        }

    ordered = sorted(counts, key=lambda item: (-item[0], item[1]))
    total = sum(count for count, _cell_id in ordered)
    top_count, top_cell_id = ordered[0]
    concentration = top_count * 1_000_000 // total
    unique_top = len(ordered) == 1 or top_count > ordered[1][0]

    if len(ordered) == 1 and not unmapped:
        tier = "R1"
        selected_cell_id = top_cell_id
    elif (
        not unmapped
        and unique_top
        and concentration >= dominant_concentration_ppm
    ):
        tier = "R2"
        selected_cell_id = top_cell_id
    else:
        tier = "R3"
        selected_cell_id = ""
    return {
        "affinity_tier": tier,
        "dominant_cell_id": selected_cell_id,
        "dominant_concentration_ppm": concentration,
    }


def build_spec_demand_observations(
    *,
    trace_records: Sequence[Mapping[str, Any]],
    code_mappings: Mapping[str, Mapping[str, Any]],
    live_code_facts: Mapping[str, Mapping[str, Any]] | None = None,
    dominant_concentration_ppm: int = DEFAULT_DOMINANT_CONCENTRATION_PPM,
) -> list[dict[str, Any]]:
    """Build demand observations independently of mapping-candidate presence."""

    _validate_concentration_threshold(dominant_concentration_ppm)
    authoritative_facts = (
        code_mappings if live_code_facts is None else live_code_facts
    )
    observations: list[dict[str, Any]] = []
    seen_discovery_ids: set[str] = set()
    for trace in sorted(trace_records, key=lambda row: str(row.get("discovery_id", ""))):
        discovery_id = _required_string(trace, "discovery_id", "trace record")
        if discovery_id in seen_discovery_ids:
            raise ValueError(f"duplicate trace record for {discovery_id}")
        seen_discovery_ids.add(discovery_id)
        if trace.get("trace_status") != OBSERVED:
            continue

        covered_code_ids = _sorted_unique_strings(
            trace.get("covered_code_ids"),
            f"{discovery_id} covered_code_ids",
        )
        unknown_code_ids = sorted(set(covered_code_ids) - set(authoritative_facts))
        if unknown_code_ids:
            raise ValueError(
                f"{discovery_id} trace references code IDs absent from live code "
                f"inventory: {unknown_code_ids}"
            )

        unmapped_code_ids = sorted(set(covered_code_ids) - set(code_mappings))
        spec_gap_code_ids: list[str] = []
        code_ids_by_cell: dict[tuple[str, ...], set[str]] = defaultdict(set)
        for code_id in covered_code_ids:
            mapping = code_mappings.get(code_id)
            if mapping is None or mapping.get("disposition") != SPEC_GAP:
                continue
            parent_spec_gap_ref = _required_string(
                mapping,
                "spec_gap_ref",
                f"{code_id} spec-gap mapping",
            )
            fact = dict(mapping)
            live_fact = authoritative_facts.get(code_id)
            if live_fact is not None:
                fact.update(live_fact)
            key = structural_cell_key(
                parent_spec_gap_ref=parent_spec_gap_ref,
                fact=fact,
            )
            spec_gap_code_ids.append(code_id)
            code_ids_by_cell[key].add(code_id)

        if not spec_gap_code_ids and not unmapped_code_ids:
            continue
        cells = [
            {
                "cell_id": structural_cell_id(key),
                **dict(zip(_CELL_KEY_FIELDS, key, strict=True)),
                "covered_code_ids": sorted(code_ids_by_cell[key]),
            }
            for key in sorted(code_ids_by_cell)
        ]
        affinity = classify_demand_affinity(
            cells=cells,
            unmapped_code_ids=unmapped_code_ids,
            dominant_concentration_ppm=dominant_concentration_ppm,
        )
        observations.append(
            {
                "discovery_id": discovery_id,
                "spec_gap_code_ids": sorted(spec_gap_code_ids),
                "unmapped_code_ids": unmapped_code_ids,
                "cells": cells,
                **affinity,
            }
        )
    return observations


def build_structural_demand_ranking(
    observations: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Rank exact structural cells with honest touch/locality counters."""

    rows_by_cell: dict[str, dict[str, Any]] = {}
    seen_discovery_ids: set[str] = set()
    for observation in sorted(
        observations,
        key=lambda row: str(row.get("discovery_id", "")),
    ):
        discovery_id = _required_string(
            observation,
            "discovery_id",
            "demand observation",
        )
        if discovery_id in seen_discovery_ids:
            raise ValueError(f"duplicate demand observation for {discovery_id}")
        seen_discovery_ids.add(discovery_id)
        tier = _required_string(
            observation,
            "affinity_tier",
            f"{discovery_id} demand observation",
        )
        if tier not in {"R1", "R2", "R3"}:
            raise ValueError(f"{discovery_id} has unknown affinity tier {tier}")
        dominant_cell_id = _string(
            observation.get("dominant_cell_id"),
            f"{discovery_id} dominant_cell_id",
        )
        cells = observation.get("cells")
        if not isinstance(cells, Sequence) or isinstance(cells, (str, bytes)):
            raise ValueError(f"{discovery_id} cells must be an array")
        seen_observation_cells: set[str] = set()
        for cell in cells:
            if not isinstance(cell, Mapping):
                raise ValueError(f"{discovery_id} cell must be an object")
            cell_id = _required_string(cell, "cell_id", f"{discovery_id} cell")
            if cell_id in seen_observation_cells:
                raise ValueError(f"{discovery_id} repeats structural cell {cell_id}")
            seen_observation_cells.add(cell_id)
            structural_fields = {
                field: _string(
                    cell.get(field),
                    f"{discovery_id} {cell_id} {field}",
                )
                for field in _CELL_KEY_FIELDS
            }
            expected_id = structural_cell_id(
                tuple(structural_fields[field] for field in _CELL_KEY_FIELDS)
            )
            if cell_id != expected_id:
                raise ValueError(
                    f"{discovery_id} structural cell ID drift: "
                    f"recorded={cell_id} expected={expected_id}"
                )
            covered_code_ids = _sorted_unique_strings(
                cell.get("covered_code_ids"),
                f"{discovery_id} {cell_id} covered_code_ids",
            )
            if not covered_code_ids:
                raise ValueError(
                    f"{discovery_id} {cell_id} has no covered code facts"
                )
            accumulated = rows_by_cell.get(cell_id)
            if accumulated is None:
                accumulated = {
                    "cell_id": cell_id,
                    **structural_fields,
                    "affected_discovery_ids": set(),
                    "covered_code_ids": set(),
                    "exclusive_discovery_ids": set(),
                    "dominant_discovery_ids": set(),
                }
                rows_by_cell[cell_id] = accumulated
            elif any(
                accumulated[field] != structural_fields[field]
                for field in _CELL_KEY_FIELDS
            ):
                raise ValueError(f"structural metadata drift for {cell_id}")
            accumulated["affected_discovery_ids"].add(discovery_id)
            accumulated["covered_code_ids"].update(covered_code_ids)
            if tier == "R1" and dominant_cell_id == cell_id:
                accumulated["exclusive_discovery_ids"].add(discovery_id)
            elif tier == "R2" and dominant_cell_id == cell_id:
                accumulated["dominant_discovery_ids"].add(discovery_id)

    rows = []
    for accumulated in rows_by_cell.values():
        affected = sorted(accumulated["affected_discovery_ids"])
        covered = sorted(accumulated["covered_code_ids"])
        exclusive = sorted(accumulated["exclusive_discovery_ids"])
        dominant = sorted(accumulated["dominant_discovery_ids"])
        rows.append(
            {
                "cell_id": accumulated["cell_id"],
                **{
                    field: accumulated[field]
                    for field in _CELL_KEY_FIELDS
                },
                "affected_discovery_ids": affected,
                "covered_code_ids": covered,
                "tests_touching": len(affected),
                "tests_exclusive": len(exclusive),
                "tests_dominant": len(dominant),
                "code_facts_touched": len(covered),
            }
        )
    return sorted(
        rows,
        key=lambda row: (
            -int(row["tests_exclusive"]),
            -int(row["tests_dominant"]),
            -int(row["tests_touching"]),
            -int(row["code_facts_touched"]),
            str(row["cell_id"]),
        ),
    )


def _qualified_symbol_parent(qualified_symbol: str) -> str:
    if "::" in qualified_symbol:
        return qualified_symbol.rsplit("::", 1)[0]
    if "." in qualified_symbol:
        return qualified_symbol.rsplit(".", 1)[0]
    return ""


def _validate_concentration_threshold(value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError("dominant concentration threshold must be an integer")
    if value < 1 or value > 1_000_000:
        raise ValueError(
            "dominant concentration threshold must be between 1 and 1000000"
        )


def _required_string(record: Mapping[str, Any], field: str, label: str) -> str:
    return _nonempty_string(record.get(field), f"{label} {field}")


def _nonempty_string(value: Any, label: str) -> str:
    result = _string(value, label)
    if not result:
        raise ValueError(f"{label} must be non-empty")
    return result


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be a string")
    return value


def _sorted_unique_strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    strings = [_nonempty_string(item, f"{label} item") for item in value]
    if len(strings) != len(set(strings)):
        raise ValueError(f"{label} contains duplicate values")
    return sorted(strings)
