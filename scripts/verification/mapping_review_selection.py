"""Numbered semantic selections compiled into generated mapping review input."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any


SELECTION_ROOT_FIELDS = {
    "schema_version",
    "proposal_queue_digest",
    "reviews",
}
ACCEPTED_SELECTION_FIELDS = {
    "proposal_index",
    "outcome",
    "reviewer_ids",
    "selected_invariant_indices",
    "oracle_bindings",
    "setup_assertion_indices",
    "non_oracle_assertion_indices",
    "confirm_generated_catalog",
    "requires_network",
}
REJECTED_SELECTION_FIELDS = {
    "proposal_index",
    "outcome",
    "reviewer_ids",
    "rejection_reason",
}


def build_review_card(
    *,
    proposal_document: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Render stable one-based choices while retaining generated identities."""

    proposals = _object_array(proposal_document.get("proposals"), "proposals")
    cards: list[dict[str, Any]] = []
    for proposal_index, proposal in enumerate(proposals, start=1):
        invariant_ids = _strings(
            proposal.get("candidate_invariant_ids"),
            f"proposal {proposal_index} candidate invariants",
        )
        invariant_cards: list[dict[str, Any]] = []
        invariant_index_by_id: dict[str, int] = {}
        for invariant_index, invariant_id in enumerate(invariant_ids, start=1):
            invariant = invariants.get(invariant_id)
            if invariant is None:
                raise ValueError(
                    f"proposal {proposal_index} invariant {invariant_id} is absent"
                )
            invariant_index_by_id[invariant_id] = invariant_index
            oracle = invariant.get("oracle")
            invariant_cards.append(
                {
                    "invariant_index": invariant_index,
                    "invariant_id": invariant_id,
                    "title": _required_string(
                        invariant, "title", f"invariant {invariant_id}"
                    ),
                    "area": _required_string(
                        invariant, "area", f"invariant {invariant_id}"
                    ),
                    "owner": _required_string(
                        invariant, "owner", f"invariant {invariant_id}"
                    ),
                    "risk": _required_string(
                        invariant, "risk", f"invariant {invariant_id}"
                    ),
                    "oracle_ref": _required_string(
                        oracle if isinstance(oracle, Mapping) else {},
                        "ref",
                        f"invariant {invariant_id} oracle",
                    ),
                    "suite_tiers": sorted(
                        _strings(
                            invariant.get("gates"),
                            f"invariant {invariant_id} gates",
                        )
                    ),
                }
            )
        behavior_cards: list[dict[str, Any]] = []
        behavior_refs = _strings(
            proposal.get("behavior_refs"),
            f"proposal {proposal_index} behavior refs",
        )
        for behavior_index, behavior_ref in enumerate(behavior_refs, start=1):
            invariant_id, row_index = _parse_behavior_ref(behavior_ref)
            invariant = invariants.get(invariant_id)
            behaviors = (
                invariant.get("behavior")
                if isinstance(invariant, Mapping)
                else None
            )
            if (
                invariant_id not in invariant_index_by_id
                or not isinstance(behaviors, Sequence)
                or isinstance(behaviors, (str, bytes))
                or row_index >= len(behaviors)
                or not isinstance(behaviors[row_index], Mapping)
            ):
                raise ValueError(
                    f"proposal {proposal_index} behavior does not resolve: "
                    f"{behavior_ref}"
                )
            behavior = behaviors[row_index]
            behavior_cards.append(
                {
                    "behavior_index": behavior_index,
                    "behavior_ref": behavior_ref,
                    "invariant_index": invariant_index_by_id[invariant_id],
                    "case_family": _display_case_family(
                        behavior, behavior_ref
                    ),
                    "outcome": _required_string(
                        behavior, "outcome", behavior_ref
                    ),
                    "rationale": _required_string(
                        behavior, "rationale", behavior_ref
                    ),
                }
            )
        assertion_cards = [
            {
                "assertion_index": assertion_index,
                "assertion_id": _required_string(
                    assertion,
                    "assertion_id",
                    f"proposal {proposal_index} assertion",
                ),
                "kind": _required_string(
                    assertion, "kind", f"proposal {proposal_index} assertion"
                ),
                "function": _required_string(
                    assertion,
                    "function",
                    f"proposal {proposal_index} assertion",
                ),
                "line": _positive_int(
                    assertion.get("line"),
                    f"proposal {proposal_index} assertion line",
                ),
                "expression": _required_string(
                    assertion,
                    "expression",
                    f"proposal {proposal_index} assertion",
                ),
            }
            for assertion_index, assertion in enumerate(
                _object_array(
                    proposal.get("assertions"),
                    f"proposal {proposal_index} assertions",
                ),
                start=1,
            )
        ]
        suggestions = proposal.get("suggestions")
        if not isinstance(suggestions, Mapping):
            raise ValueError(f"proposal {proposal_index} lacks suggestions")
        cards.append(
            {
                "proposal_index": proposal_index,
                "proposal_id": _required_string(
                    proposal, "id", f"proposal {proposal_index}"
                ),
                "discovery_id": _required_string(
                    proposal, "discovery_id", f"proposal {proposal_index}"
                ),
                "candidate_tier": _required_string(
                    proposal, "candidate_tier", f"proposal {proposal_index}"
                ),
                "path": _required_string(
                    proposal, "path", f"proposal {proposal_index}"
                ),
                "name": _required_string(
                    proposal, "name", f"proposal {proposal_index}"
                ),
                "command": _required_string(
                    proposal, "command", f"proposal {proposal_index}"
                ),
                "duration_ms": _nonnegative_int(
                    proposal.get("duration_ms"),
                    f"proposal {proposal_index} duration",
                ),
                "duration_class": _required_string(
                    proposal, "duration_class", f"proposal {proposal_index}"
                ),
                "suggested_test_class": _required_string(
                    suggestions,
                    "test_class",
                    f"proposal {proposal_index} suggestions",
                ),
                "invariants": invariant_cards,
                "behaviors": behavior_cards,
                "assertions": assertion_cards,
            }
        )
    return {
        "schema_version": 1,
        "generator": "test-catalog-mapping-review-card",
        "generator_version": "2",
        "source_commit": _required_string(
            proposal_document, "source_commit", "proposal queue"
        ),
        "proposal_queue_digest": canonical_digest(proposal_document),
        "proposals": cards,
    }


def build_review_input_from_selection(
    *,
    proposal_document: Mapping[str, Any],
    selection_document: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Resolve human numeric choices into generated IDs and catalog metadata."""

    if set(selection_document) != SELECTION_ROOT_FIELDS:
        raise ValueError("mapping review selection root fields drift")
    if selection_document.get("schema_version") != 1:
        raise ValueError("mapping review selection must use schema_version 1")
    expected_digest = canonical_digest(proposal_document)
    if selection_document.get("proposal_queue_digest") != expected_digest:
        raise ValueError("mapping review selection proposal queue digest is stale")
    proposals = _object_array(proposal_document.get("proposals"), "proposals")
    selections = _object_array(
        selection_document.get("reviews"), "review selections", nonempty=True
    )
    selected_proposals: set[int] = set()
    reviews: list[dict[str, Any]] = []
    for selection in selections:
        proposal_index = _positive_int(
            selection.get("proposal_index"), "proposal index"
        )
        if proposal_index in selected_proposals:
            raise ValueError(
                f"mapping review selection duplicates proposal index {proposal_index}"
            )
        selected_proposals.add(proposal_index)
        proposal = _indexed(proposals, proposal_index, "proposal")
        outcome = selection.get("outcome")
        if outcome == "accepted":
            reviews.append(
                _accepted_review(
                    proposal=proposal,
                    selection=selection,
                    invariants=invariants,
                    proposal_index=proposal_index,
                )
            )
        elif outcome == "rejected":
            reviews.append(
                _rejected_review(
                    proposal=proposal,
                    selection=selection,
                    proposal_index=proposal_index,
                )
            )
        else:
            raise ValueError(
                f"proposal {proposal_index} outcome must be accepted or rejected"
            )
    reviews.sort(key=lambda row: str(row["proposal_id"]))
    return {"schema_version": 1, "reviews": reviews}


def canonical_digest(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _accepted_review(
    *,
    proposal: Mapping[str, Any],
    selection: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
    proposal_index: int,
) -> dict[str, Any]:
    if set(selection) != ACCEPTED_SELECTION_FIELDS:
        raise ValueError(
            f"accepted proposal {proposal_index} selection fields drift"
        )
    if selection.get("confirm_generated_catalog") is not True:
        raise ValueError(
            f"accepted proposal {proposal_index} requires catalog confirmation"
        )
    reviewers = sorted(
        set(
            _strings(
                selection.get("reviewer_ids"),
                f"proposal {proposal_index} reviewers",
                nonempty=True,
            )
        )
    )
    invariant_ids = _strings(
        proposal.get("candidate_invariant_ids"),
        f"proposal {proposal_index} candidate invariants",
        nonempty=True,
    )
    selected_invariants = [
        _indexed(invariant_ids, index, "invariant")
        for index in _indices(
            selection.get("selected_invariant_indices"),
            len(invariant_ids),
            f"proposal {proposal_index} invariant",
            nonempty=True,
        )
    ]
    behavior_refs = _strings(
        proposal.get("behavior_refs"),
        f"proposal {proposal_index} behavior refs",
        nonempty=True,
    )
    assertions = _object_array(
        proposal.get("assertions"),
        f"proposal {proposal_index} assertions",
    )
    oracle_bindings = _oracle_bindings(
        selection.get("oracle_bindings"),
        assertion_count=len(assertions),
        behavior_refs=behavior_refs,
        selected_invariants=set(selected_invariants),
        proposal_index=proposal_index,
    )
    oracle = sorted(oracle_bindings)
    selected_behavior_refs = sorted(
        {
            behavior_ref
            for refs in oracle_bindings.values()
            for behavior_ref in refs
        }
    )
    bound_invariants = {
        behavior_ref.partition("#behavior:")[0]
        for behavior_ref in selected_behavior_refs
    }
    if bound_invariants != set(selected_invariants):
        raise ValueError(
            f"proposal {proposal_index} requires an oracle-bound assertion "
            "for every selected invariant"
        )
    setup = _indices(
        selection.get("setup_assertion_indices"),
        len(assertions),
        f"proposal {proposal_index} setup assertion",
    )
    non_oracle = _indices(
        selection.get("non_oracle_assertion_indices"),
        len(assertions),
        f"proposal {proposal_index} non-oracle assertion",
    )
    if sorted([*oracle, *setup, *non_oracle]) != list(
        range(1, len(assertions) + 1)
    ):
        raise ValueError(
            f"proposal {proposal_index} must classify every assertion exactly once"
        )
    assertion_reviews = []
    for assertion_index, assertion in enumerate(assertions, start=1):
        if assertion_index in oracle:
            disposition = "oracle_bound"
            refs = oracle_bindings[assertion_index]
            rationale = (
                "Reviewer bound this generated assertion to the selected "
                "oracle-backed behavior."
            )
        elif assertion_index in setup:
            disposition = "setup_or_precondition"
            refs = []
            rationale = (
                "Reviewer classified this generated assertion as setup or "
                "precondition."
            )
        else:
            disposition = "non_oracle"
            refs = []
            rationale = (
                "Reviewer classified this generated assertion as non-oracle."
            )
        assertion_reviews.append(
            {
                "assertion_id": _required_string(
                    assertion,
                    "assertion_id",
                    f"proposal {proposal_index} assertion",
                ),
                "disposition": disposition,
                "behavior_refs": sorted(refs),
                "rationale": rationale,
            }
        )
    assertion_reviews.sort(key=lambda row: row["assertion_id"])

    selected_records = []
    for invariant_id in selected_invariants:
        invariant = invariants.get(invariant_id)
        if invariant is None:
            raise ValueError(
                f"proposal {proposal_index} selected invariant is absent: "
                f"{invariant_id}"
            )
        selected_records.append(invariant)
    area = _shared_field(selected_records, "area", proposal_index)
    owner = _shared_field(selected_records, "owner", proposal_index)
    _shared_oracle_ref(selected_records, proposal_index)
    suite_tiers = sorted(
        {
            tier
            for invariant in selected_records
            for tier in _strings(
                invariant.get("gates"),
                f"proposal {proposal_index} invariant gates",
            )
        }
    )
    if not suite_tiers:
        raise ValueError(
            f"proposal {proposal_index} selected invariants lack suite routing"
        )
    behavior_rationales = [
        _behavior_rationale(invariants, behavior_ref)
        for behavior_ref in selected_behavior_refs
    ]
    suggestions = proposal.get("suggestions")
    if not isinstance(suggestions, Mapping) or suggestions.get("authoritative") is not False:
        raise ValueError(
            f"proposal {proposal_index} suggestions are not quarantined"
        )
    expected_result = " ".join(behavior_rationales)
    expected_failure_mode = " ".join(
        f"The observed result violates the reviewed behavior: {rationale}"
        for rationale in behavior_rationales
    )
    requires_network = selection.get("requires_network")
    if not isinstance(requires_network, bool):
        raise ValueError(
            f"proposal {proposal_index} requires_network confirmation must be boolean"
        )
    return {
        "proposal_id": _required_string(
            proposal, "id", f"proposal {proposal_index}"
        ),
        "outcome": "accepted",
        "reviewer_ids": reviewers,
        "assertion_reviews": assertion_reviews,
        "selected_invariant_ids": sorted(selected_invariants),
        "selected_behavior_refs": sorted(selected_behavior_refs),
        "command": _required_string(
            proposal, "command", f"proposal {proposal_index}"
        ),
        "command_authority": "verified_exact",
        "confirmed_catalog": {
            "test_class": _required_string(
                suggestions,
                "test_class",
                f"proposal {proposal_index} suggestions",
            ),
            "area": area,
            "owner": owner,
            "expected_result": expected_result,
            "expected_failure_mode": expected_failure_mode,
            "suite_tiers": suite_tiers,
            "requires_hardware": "hardware_lab" in suite_tiers,
            "requires_network": requires_network,
            "duration_class": _required_string(
                proposal, "duration_class", f"proposal {proposal_index}"
            ),
        },
    }


def _oracle_bindings(
    value: Any,
    *,
    assertion_count: int,
    behavior_refs: Sequence[str],
    selected_invariants: set[str],
    proposal_index: int,
) -> dict[int, list[str]]:
    bindings = _object_array(
        value,
        f"proposal {proposal_index} oracle bindings",
        nonempty=True,
    )
    resolved: dict[int, list[str]] = {}
    for binding in bindings:
        if set(binding) != {"assertion_index", "behavior_indices"}:
            raise ValueError(
                f"proposal {proposal_index} oracle binding fields drift"
            )
        assertion_index = _positive_int(
            binding.get("assertion_index"),
            f"proposal {proposal_index} oracle assertion index",
        )
        if assertion_index > assertion_count:
            raise ValueError(
                f"proposal {proposal_index} oracle assertion index "
                f"{assertion_index} is outside displayed range"
            )
        if assertion_index in resolved:
            raise ValueError(
                f"proposal {proposal_index} duplicates oracle assertion "
                f"{assertion_index}"
            )
        refs = [
            _indexed(behavior_refs, index, "behavior")
            for index in _indices(
                binding.get("behavior_indices"),
                len(behavior_refs),
                f"proposal {proposal_index} behavior",
                nonempty=True,
            )
        ]
        if any(
            behavior_ref.partition("#behavior:")[0] not in selected_invariants
            for behavior_ref in refs
        ):
            raise ValueError(
                f"proposal {proposal_index} selected behavior lacks selected "
                "invariant"
            )
        resolved[assertion_index] = sorted(refs)
    return resolved


def _rejected_review(
    *,
    proposal: Mapping[str, Any],
    selection: Mapping[str, Any],
    proposal_index: int,
) -> dict[str, Any]:
    if set(selection) != REJECTED_SELECTION_FIELDS:
        raise ValueError(
            f"rejected proposal {proposal_index} selection fields drift"
        )
    return {
        "proposal_id": _required_string(
            proposal, "id", f"proposal {proposal_index}"
        ),
        "outcome": "rejected",
        "reviewer_ids": sorted(
            set(
                _strings(
                    selection.get("reviewer_ids"),
                    f"proposal {proposal_index} reviewers",
                    nonempty=True,
                )
            )
        ),
        "assertion_reviews": [],
        "selected_invariant_ids": [],
        "selected_behavior_refs": [],
        "command": "",
        "command_authority": "not_applicable",
        "rejection_reason": _required_string(
            selection,
            "rejection_reason",
            f"proposal {proposal_index}",
        ),
    }


def _behavior_rationale(
    invariants: Mapping[str, Mapping[str, Any]],
    behavior_ref: str,
) -> str:
    invariant_id, row_index = _parse_behavior_ref(behavior_ref)
    invariant = invariants.get(invariant_id)
    behaviors = (
        invariant.get("behavior") if isinstance(invariant, Mapping) else None
    )
    if (
        not isinstance(behaviors, Sequence)
        or isinstance(behaviors, (str, bytes))
        or row_index >= len(behaviors)
        or not isinstance(behaviors[row_index], Mapping)
    ):
        raise ValueError(f"selected behavior does not resolve: {behavior_ref}")
    return _required_string(behaviors[row_index], "rationale", behavior_ref)


def _display_case_family(
    behavior: Mapping[str, Any],
    behavior_ref: str,
) -> str:
    family = behavior.get("case_family")
    if family is None:
        partition = behavior.get("partition")
        if (
            not isinstance(partition, Mapping)
            or "equals" in partition
        ):
            raise ValueError(
                f"{behavior_ref} lacks case_family outside a non-equals "
                "partition"
            )
        return "not_applicable"
    if not isinstance(family, str) or not family:
        raise ValueError(f"{behavior_ref} requires non-empty case_family")
    return family


def _shared_field(
    records: Sequence[Mapping[str, Any]],
    field: str,
    proposal_index: int,
) -> str:
    values = {
        _required_string(
            record, field, f"proposal {proposal_index} selected invariant"
        )
        for record in records
    }
    if len(values) != 1:
        raise ValueError(
            f"proposal {proposal_index} selected invariants do not share {field}"
        )
    return next(iter(values))


def _shared_oracle_ref(
    records: Sequence[Mapping[str, Any]],
    proposal_index: int,
) -> str:
    refs: set[str] = set()
    for record in records:
        oracle = record.get("oracle")
        refs.add(
            _required_string(
                oracle if isinstance(oracle, Mapping) else {},
                "ref",
                f"proposal {proposal_index} selected invariant oracle",
            )
        )
    if len(refs) != 1:
        raise ValueError(
            f"proposal {proposal_index} selected invariants do not share "
            "one oracle ref"
        )
    return next(iter(refs))


def _parse_behavior_ref(value: str) -> tuple[str, int]:
    invariant_id, separator, raw_index = value.partition("#behavior:")
    if (
        not separator
        or not invariant_id
        or not raw_index.isdigit()
        or int(raw_index) < 0
    ):
        raise ValueError(f"invalid behavior reference {value!r}")
    return invariant_id, int(raw_index)


def _indices(
    value: Any,
    size: int,
    label: str,
    *,
    nonempty: bool = False,
) -> list[int]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} indices must be an array")
    indices = list(value)
    if (
        any(
            not isinstance(index, int)
            or isinstance(index, bool)
            or index < 1
            or index > size
            for index in indices
        )
        or indices != sorted(set(indices))
    ):
        raise ValueError(
            f"{label} indices must be sorted, distinct, and within 1..{size}"
        )
    if nonempty and not indices:
        raise ValueError(f"{label} indices must not be empty")
    return indices


def _indexed(values: Sequence[Any], one_based: int, label: str) -> Any:
    if one_based < 1 or one_based > len(values):
        raise ValueError(
            f"{label} index {one_based} is outside displayed range 1..{len(values)}"
        )
    return values[one_based - 1]


def _object_array(
    value: Any,
    label: str,
    *,
    nonempty: bool = False,
) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, Mapping) for item in value)
    ):
        raise ValueError(f"{label} must be an object array")
    result = list(value)
    if nonempty and not result:
        raise ValueError(f"{label} must not be empty")
    return result


def _strings(
    value: Any,
    label: str,
    *,
    nonempty: bool = False,
) -> list[str]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise ValueError(f"{label} must be a distinct string array")
    result = list(value)
    if len(result) != len(set(result)):
        raise ValueError(f"{label} must be a distinct string array")
    if nonempty and not result:
        raise ValueError(f"{label} must not be empty")
    return result


def _required_string(
    value: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    result = value.get(field)
    if not isinstance(result, str) or not result:
        raise ValueError(f"{label} requires non-empty {field}")
    return result


def _positive_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        raise ValueError(f"{label} must be a positive integer")
    return value


def _nonnegative_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{label} must be a nonnegative integer")
    return value
