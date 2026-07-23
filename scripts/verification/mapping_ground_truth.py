"""Ground-truth qualification for execution-guided mapping candidates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


def partition_ground_truth_candidates(
    *,
    candidates: Sequence[Mapping[str, Any]],
    expected_invariants: Mapping[str, Sequence[str]],
) -> tuple[list[Mapping[str, Any]], list[str]]:
    """Separate hidden-ground-truth rows from quarantined unknown candidates."""

    known: list[Mapping[str, Any]] = []
    unknown: list[str] = []
    seen: set[str] = set()
    for candidate in sorted(
        candidates, key=lambda row: str(row.get("discovery_id", ""))
    ):
        discovery_id = candidate.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("ground-truth candidate requires discovery_id")
        if discovery_id in seen:
            raise ValueError(f"ground-truth candidate duplicates {discovery_id}")
        seen.add(discovery_id)
        if discovery_id in expected_invariants:
            known.append(candidate)
        else:
            unknown.append(discovery_id)
    return known, unknown


def evaluate_ground_truth(
    *,
    candidates: Sequence[Mapping[str, Any]],
    expected_invariants: Mapping[str, Sequence[str]],
    accepted_reviews: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Compare candidate quality and separately reviewed acceptances with truth."""

    results: list[dict[str, Any]] = []
    seen: set[str] = set()
    for candidate in sorted(
        candidates, key=lambda row: str(row.get("discovery_id", ""))
    ):
        discovery_id = candidate.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("ground-truth candidate requires discovery_id")
        if discovery_id in seen:
            raise ValueError(f"ground-truth candidate duplicates {discovery_id}")
        seen.add(discovery_id)
        if discovery_id not in expected_invariants:
            raise ValueError(f"candidate {discovery_id} is outside ground truth")
        proposed = _strings(
            candidate.get("candidate_invariant_ids"),
            f"{discovery_id} candidate invariants",
        )
        expected = _strings(
            expected_invariants[discovery_id],
            f"{discovery_id} expected invariants",
        )
        false_ids = sorted(set(proposed) - set(expected))
        missed_ids = sorted(set(expected) - set(proposed))
        results.append(
            {
                "discovery_id": discovery_id,
                "candidate_invariant_ids": proposed,
                "expected_invariant_ids": expected,
                "false_invariant_ids": false_ids,
                "missed_invariant_ids": missed_ids,
                "exact": not false_ids and not missed_ids,
            }
        )
    candidates_by_id = {
        str(candidate["discovery_id"]): candidate for candidate in candidates
    }
    accepted_results: list[dict[str, Any]] = []
    seen_reviews: set[str] = set()
    for review in sorted(
        accepted_reviews, key=lambda row: str(row.get("discovery_id", ""))
    ):
        discovery_id = review.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("ground-truth accepted review requires discovery_id")
        if discovery_id in seen_reviews:
            raise ValueError(
                f"ground-truth accepted reviews duplicate {discovery_id}"
            )
        seen_reviews.add(discovery_id)
        candidate = candidates_by_id.get(discovery_id)
        if candidate is None:
            raise ValueError(
                f"accepted review {discovery_id} lacks a candidate signal"
            )
        selected = _strings(
            review.get("selected_invariant_ids"),
            f"{discovery_id} selected invariants",
        )
        proposed = _strings(
            candidate.get("candidate_invariant_ids"),
            f"{discovery_id} candidate invariants",
        )
        if not set(selected).issubset(set(proposed)):
            raise ValueError(
                f"accepted review {discovery_id} selects a non-candidate invariant"
            )
        expected = _strings(
            expected_invariants[discovery_id],
            f"{discovery_id} expected invariants",
        )
        false_ids = sorted(set(selected) - set(expected))
        missed_ids = sorted(set(expected) - set(selected))
        accepted_results.append(
            {
                "discovery_id": discovery_id,
                "selected_invariant_ids": selected,
                "expected_invariant_ids": expected,
                "false_invariant_ids": false_ids,
                "missed_invariant_ids": missed_ids,
                "exact": not false_ids and not missed_ids,
            }
        )
    return {
        "pilot_candidates": len(results),
        "zero_false_candidates": all(not row["false_invariant_ids"] for row in results),
        "exact_candidates": [
            row["discovery_id"] for row in results if row["exact"]
        ],
        "candidate_results": results,
        "reviewed_acceptances": len(accepted_results),
        "zero_false_accepted_mappings": bool(accepted_results)
        and all(not row["false_invariant_ids"] for row in accepted_results),
        "exact_accepted_mappings": [
            row["discovery_id"] for row in accepted_results if row["exact"]
        ],
        "accepted_results": accepted_results,
        "creates_catalog_mappings": False,
    }


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must contain non-empty strings")
    if len(value) != len(set(value)):
        raise ValueError(f"{label} must not contain duplicates")
    return sorted(value)
