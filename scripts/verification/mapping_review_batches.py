"""Deterministic semantic batches for test-catalog mapping review cards."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Mapping, Sequence
from typing import Any


def build_review_batches(
    card: Mapping[str, Any],
    *,
    max_batch_size: int,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Group displayed proposals without changing their queue-global indices."""

    if (
        not isinstance(max_batch_size, int)
        or isinstance(max_batch_size, bool)
        or max_batch_size < 1
    ):
        raise ValueError("mapping review max batch size must be a positive integer")
    proposals = card.get("proposals")
    if (
        not isinstance(proposals, Sequence)
        or isinstance(proposals, (str, bytes))
        or any(not isinstance(proposal, Mapping) for proposal in proposals)
    ):
        raise ValueError("mapping review card proposals must be an object array")

    grouped: dict[
        tuple[str, tuple[str, ...], tuple[str, ...]],
        list[Mapping[str, Any]],
    ] = defaultdict(list)
    seen_indices: set[int] = set()
    for proposal in proposals:
        proposal_index = proposal.get("proposal_index")
        if (
            not isinstance(proposal_index, int)
            or isinstance(proposal_index, bool)
            or proposal_index < 1
            or proposal_index in seen_indices
        ):
            raise ValueError("mapping review proposal indices must be positive and unique")
        seen_indices.add(proposal_index)
        tier = _required_string(proposal, "candidate_tier", proposal_index)
        invariants = _object_array(proposal.get("invariants"), "invariants", proposal_index)
        behaviors = _object_array(proposal.get("behaviors"), "behaviors", proposal_index)
        invariant_ids = tuple(
            sorted(
                _required_string(invariant, "invariant_id", proposal_index)
                for invariant in invariants
            )
        )
        behavior_refs = tuple(
            sorted(
                _required_string(behavior, "behavior_ref", proposal_index)
                for behavior in behaviors
            )
        )
        if not invariant_ids or not behavior_refs:
            raise ValueError(
                f"mapping review proposal {proposal_index} lacks semantic grouping"
            )
        grouped[(tier, invariant_ids, behavior_refs)].append(proposal)

    cards: list[dict[str, Any]] = []
    manifest_batches: list[dict[str, Any]] = []
    batch_index = 0
    for semantic_group_index, (group_key, members) in enumerate(
        sorted(grouped.items()),
        start=1,
    ):
        tier, invariant_ids, behavior_refs = group_key
        ordered = sorted(members, key=lambda item: int(item["proposal_index"]))
        for offset in range(0, len(ordered), max_batch_size):
            batch_index += 1
            chunk = ordered[offset : offset + max_batch_size]
            json_file = f"batch-{batch_index:03d}.json"
            markdown_file = f"batch-{batch_index:03d}.md"
            cards.append(
                {
                    key: value
                    for key, value in card.items()
                    if key != "proposals"
                }
                | {"proposals": [dict(proposal) for proposal in chunk]}
            )
            indices = [int(proposal["proposal_index"]) for proposal in chunk]
            manifest_batches.append(
                {
                    "batch_index": batch_index,
                    "semantic_group_index": semantic_group_index,
                    "candidate_tier": tier,
                    "invariant_ids": list(invariant_ids),
                    "behavior_refs": list(behavior_refs),
                    "proposal_indices": indices,
                    "proposal_count": len(indices),
                    "json_file": json_file,
                    "markdown_file": markdown_file,
                }
            )

    manifest = {
        "schema_version": 1,
        "generator": "test-catalog-mapping-review-batches",
        "generator_version": "1",
        "source_commit": card.get("source_commit"),
        "proposal_queue_digest": card.get("proposal_queue_digest"),
        "max_batch_size": max_batch_size,
        "proposal_count": len(proposals),
        "semantic_group_count": len(grouped),
        "batch_count": len(cards),
        "batches": manifest_batches,
    }
    return manifest, cards


def _object_array(
    value: Any,
    label: str,
    proposal_index: int,
) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, Mapping) for item in value)
    ):
        raise ValueError(
            f"mapping review proposal {proposal_index} {label} must be an object array"
        )
    return list(value)


def _required_string(
    value: Mapping[str, Any],
    field: str,
    proposal_index: int,
) -> str:
    result = value.get(field)
    if not isinstance(result, str) or not result:
        raise ValueError(
            f"mapping review proposal {proposal_index} requires non-empty {field}"
        )
    return result
