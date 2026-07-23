"""Pure batch planning for reviewed mapping ledger replacements."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from .mapping_catalog_updates import (
    append_catalog_record,
    append_invariant_test,
    build_accepted_catalog_record,
    map_denominator_review,
    render_denominator_document,
)
from .mapping_review import build_mapping_decision, validate_mapping_decisions
from .mapping_toml import render_decisions_toml, render_proposals_toml


@dataclass(frozen=True)
class MappingReviewBatch:
    replacements: Mapping[str, bytes]
    accepted_test_ids: tuple[str, ...]
    rejected_proposal_ids: tuple[str, ...]


def build_review_batch(
    *,
    proposal_document: Mapping[str, Any],
    decision_document: Mapping[str, Any],
    review_inputs: Sequence[Mapping[str, Any]],
    catalog_source: str,
    denominator_document: Mapping[str, Any],
    invariant_sources: Mapping[str, tuple[str, str]],
    invariants: Mapping[str, Mapping[str, Any]],
    reviewed_date: str,
    source_commit: str,
) -> MappingReviewBatch:
    proposals = _index_records(
        proposal_document.get("proposals"), "proposal"
    )
    snapshots = _index_records(
        decision_document.get("proposal_snapshots"), "proposal snapshot"
    )
    existing_decisions = _index_records(
        decision_document.get("decisions"), "decision"
    )
    if not review_inputs:
        raise ValueError("mapping review batch must not be empty")
    review_by_proposal: dict[str, Mapping[str, Any]] = {}
    for review in review_inputs:
        proposal_id = review.get("proposal_id")
        if not isinstance(proposal_id, str) or not proposal_id:
            raise ValueError("mapping review input lacks proposal ID")
        if proposal_id in review_by_proposal:
            raise ValueError(f"mapping review batch duplicates {proposal_id}")
        if proposal_id not in proposals:
            raise ValueError(f"mapping review names unknown pending {proposal_id}")
        review_by_proposal[proposal_id] = review

    catalog_after = catalog_source
    denominator_after = dict(denominator_document)
    invariant_after = {
        invariant_id: (path, source)
        for invariant_id, (path, source) in invariant_sources.items()
    }
    new_decisions: list[dict[str, Any]] = []
    accepted_test_ids: list[str] = []
    rejected_proposal_ids: list[str] = []
    for proposal_id in sorted(review_by_proposal):
        proposal = proposals[proposal_id]
        if proposal_id in snapshots:
            raise ValueError(f"mapping proposal {proposal_id} was already reviewed")
        decision = build_mapping_decision(
            proposal=proposal,
            review_input=review_by_proposal[proposal_id],
        )
        if decision["id"] in existing_decisions:
            raise ValueError(f"mapping decision {decision['id']} already exists")
        snapshots[proposal_id] = proposal
        existing_decisions[str(decision["id"])] = decision
        new_decisions.append(decision)
        if decision["outcome"] == "rejected":
            rejected_proposal_ids.append(proposal_id)
            continue
        catalog_record = build_accepted_catalog_record(
            proposal=proposal,
            decision=decision,
            invariants=invariants,
            reviewed_date=reviewed_date,
        )
        catalog_after = append_catalog_record(catalog_after, catalog_record)
        denominator_after = map_denominator_review(
            document=denominator_after,
            proposal=proposal,
            catalog_test_id=str(catalog_record["id"]),
            reviewed_date=reviewed_date,
        )
        for invariant_id in decision["selected_invariant_ids"]:
            source_entry = invariant_after.get(str(invariant_id))
            if source_entry is None:
                raise ValueError(
                    f"accepted mapping lacks source for invariant {invariant_id}"
                )
            path, source = source_entry
            invariant_after[str(invariant_id)] = (
                path,
                append_invariant_test(source, str(catalog_record["id"])),
            )
        accepted_test_ids.append(str(catalog_record["id"]))

    decision_failures = validate_mapping_decisions(
        decisions=list(existing_decisions.values()),
        proposals=snapshots,
        invariants=invariants,
        candidate_generator_id=str(
            proposal_document.get("generator", "test-catalog-proposals")
        ),
    )
    if decision_failures:
        raise ValueError("; ".join(decision_failures))

    reviewed_ids = set(review_by_proposal)
    proposal_after = {
        **dict(proposal_document),
        "proposals": [
            proposal
            for proposal_id, proposal in sorted(proposals.items())
            if proposal_id not in reviewed_ids
        ],
    }
    decisions_after = {
        **dict(decision_document),
        "source_commit": source_commit,
        "proposal_snapshots": [
            snapshot for _, snapshot in sorted(snapshots.items())
        ],
        "decisions": [
            decision for _, decision in sorted(existing_decisions.items())
        ],
    }
    replacements: dict[str, bytes] = {
        "verification/test-catalog.toml": catalog_after.encode(),
        "verification/test-catalog-denominator.toml": (
            render_denominator_document(denominator_after).encode()
        ),
        "verification/test-catalog-proposals.toml": (
            render_proposals_toml(proposal_after).encode()
        ),
        "verification/test-catalog-mapping-decisions.toml": (
            render_decisions_toml(decisions_after).encode()
        ),
    }
    for _, (path, source) in sorted(invariant_after.items()):
        if source != invariant_sources[_invariant_id_for_path(invariant_sources, path)][1]:
            replacements[path] = source.encode()
    return MappingReviewBatch(
        replacements=dict(sorted(replacements.items())),
        accepted_test_ids=tuple(sorted(accepted_test_ids)),
        rejected_proposal_ids=tuple(sorted(rejected_proposal_ids)),
    )


def _index_records(value: Any, label: str) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"mapping {label} records must be an array")
    records: dict[str, Mapping[str, Any]] = {}
    for index, record in enumerate(value):
        if not isinstance(record, Mapping):
            raise ValueError(f"mapping {label}[{index}] must be an object")
        record_id = record.get("id")
        if not isinstance(record_id, str) or not record_id:
            raise ValueError(f"mapping {label}[{index}] lacks ID")
        if record_id in records:
            raise ValueError(f"mapping {label} records duplicate {record_id}")
        records[record_id] = record
    return records


def _invariant_id_for_path(
    invariant_sources: Mapping[str, tuple[str, str]],
    path: str,
) -> str:
    matches = [
        invariant_id
        for invariant_id, (candidate_path, _) in invariant_sources.items()
        if candidate_path == path
    ]
    if len(matches) != 1:
        raise ValueError(f"invariant source path {path} is not uniquely owned")
    return matches[0]
