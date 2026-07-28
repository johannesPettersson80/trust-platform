"""Reviewed assertion and behavior policy for the runtime-other transaction."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .runtime_other_behavior_mapping_support import records, string, strings


REVIEWERS = ["codex-assertion-review", "codex-main"]


def new_review_input(
    *,
    proposal: Mapping[str, Any],
    assignment: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    invariant_ids = strings(assignment.get("invariant_ids"), "invariant IDs")
    behavior_refs = strings(assignment.get("behavior_refs"), "behavior refs")
    expected = " ".join(
        behavior_rationale(invariants, behavior_ref)
        for behavior_ref in behavior_refs
    )
    primary = invariants[invariant_ids[0]]
    return {
        "proposal_id": proposal["id"],
        "outcome": "accepted",
        "reviewer_ids": REVIEWERS,
        "assertion_reviews": assertion_reviews(
            proposal=proposal,
            behavior_refs=behavior_refs,
        ),
        "selected_invariant_ids": invariant_ids,
        "selected_behavior_refs": behavior_refs,
        "command": proposal["command"],
        "command_authority": "verified_exact",
        "confirmed_catalog": {
            "test_class": "behavior_lock",
            "area": primary["area"],
            "owner": primary["owner"],
            "expected_result": expected,
            "expected_failure_mode": (
                "The reviewed contract accepts missing, stale, contradictory, "
                "or falsely successful product behavior."
            ),
            "suite_tiers": ["pr"],
            "requires_hardware": False,
            "requires_network": False,
            "duration_class": proposal["duration_class"],
        },
    }


def refreshed_review_input(
    *,
    proposal: Mapping[str, Any],
    test: Mapping[str, Any],
    invariant_ids: list[str],
    behavior_refs: list[str],
) -> dict[str, Any]:
    return {
        "proposal_id": proposal["id"],
        "outcome": "accepted",
        "reviewer_ids": REVIEWERS,
        "assertion_reviews": assertion_reviews(
            proposal=proposal,
            behavior_refs=behavior_refs,
        ),
        "selected_invariant_ids": invariant_ids,
        "selected_behavior_refs": behavior_refs,
        "command": proposal["command"],
        "command_authority": "verified_exact",
        "confirmed_catalog": confirmed_from_test(
            test=test,
            duration=proposal["duration_class"],
        ),
    }


def decision_refinement_input(
    *,
    proposal: Mapping[str, Any],
    old_decision: Mapping[str, Any],
    test: Mapping[str, Any],
    invariant_ids: list[str],
    behavior_refs: list[str],
) -> dict[str, Any]:
    reviews = []
    for review in records(
        old_decision.get("assertion_reviews"),
        "assertion reviews",
    ):
        disposition = string(review, "disposition", "assertion review")
        reviews.append(
            {
                "assertion_id": string(
                    review, "assertion_id", "assertion review"
                ),
                "disposition": disposition,
                "behavior_refs": (
                    behavior_refs if disposition == "oracle_bound" else []
                ),
                "rationale": string(
                    review, "rationale", "assertion review"
                ),
            }
        )
    return {
        "proposal_id": proposal["id"],
        "outcome": "accepted",
        "reviewer_ids": REVIEWERS,
        "assertion_reviews": reviews,
        "selected_invariant_ids": invariant_ids,
        "selected_behavior_refs": behavior_refs,
        "command": proposal["command"],
        "command_authority": "verified_exact",
        "confirmed_catalog": confirmed_from_test(
            test=test,
            duration=string(proposal, "duration_class", "proposal"),
        ),
    }


def assertion_reviews(
    *,
    proposal: Mapping[str, Any],
    behavior_refs: list[str],
) -> list[dict[str, Any]]:
    reviews = []
    name = string(proposal, "name", "proposal")
    for assertion in records(proposal.get("assertions"), "assertions"):
        kind = string(assertion, "kind", "assertion")
        oracle = kind.startswith("macro:assert") or kind == "method:expect_err"
        refs = (
            play_behavior_refs(
                name=name,
                line=int(assertion["line"]),
                fallback=behavior_refs,
            )
            if oracle
            else []
        )
        reviews.append(
            {
                "assertion_id": string(
                    assertion, "assertion_id", "assertion"
                ),
                "disposition": (
                    "oracle_bound" if oracle else "setup_or_precondition"
                ),
                "behavior_refs": refs,
                "rationale": (
                    "The direct reviewed assertion locks the selected "
                    "product-contract behavior."
                    if oracle
                    else (
                        "The reviewed call establishes fixture setup or a "
                        "precondition and is not counted as product-oracle proof."
                    )
                ),
            }
        )
    if not any(row["disposition"] == "oracle_bound" for row in reviews):
        raise ValueError(f"{name} has no direct reviewed product oracle")
    return reviews


def play_behavior_refs(
    *,
    name: str,
    line: int,
    fallback: list[str],
) -> list[str]:
    if name == "play_explicit_project_shape_classification_is_fail_closed":
        mapping = {
            391: ["RT_PLAY_LAUNCH_001#behavior:1"],
            392: ["RT_PLAY_LAUNCH_001#behavior:1"],
            393: ["RT_PLAY_LAUNCH_001#behavior:2"],
            394: ["RT_PLAY_LAUNCH_001#behavior:3"],
        }
        if line not in mapping:
            raise ValueError(f"unexpected Play shape oracle line {line}")
        return mapping[line]
    if name == (
        "play_project_resolution_distinguishes_absence_from_detection_failure"
    ):
        mapping = {
            412: ["RT_PLAY_LAUNCH_001#behavior:4"],
            416: ["RT_PLAY_LAUNCH_001#behavior:4"],
            421: ["RT_PLAY_LAUNCH_001#behavior:5"],
            432: ["RT_PLAY_LAUNCH_001#behavior:6"],
            438: ["RT_PLAY_LAUNCH_001#behavior:6"],
        }
        if line not in mapping:
            raise ValueError(f"unexpected Play resolution oracle line {line}")
        return mapping[line]
    return list(fallback)


def confirmed_from_test(
    *,
    test: Mapping[str, Any],
    duration: str,
) -> dict[str, Any]:
    return {
        "test_class": string(test, "test_class", "catalog test"),
        "area": string(test, "area", "catalog test"),
        "owner": string(test, "owner", "catalog test"),
        "expected_result": string(test, "expected_result", "catalog test"),
        "expected_failure_mode": string(
            test, "expected_failure_mode", "catalog test"
        ),
        "suite_tiers": strings(test.get("suite_tiers"), "suite tiers"),
        "requires_hardware": bool(test.get("requires_hardware")),
        "requires_network": bool(test.get("requires_network")),
        "duration_class": duration,
    }


def selected_invariant_ids(
    *,
    test: Mapping[str, Any],
    assignment: Mapping[str, Any] | None,
) -> list[str]:
    values = set(strings(test.get("invariants"), "catalog invariants"))
    if assignment is not None:
        values.update(strings(assignment.get("invariant_ids"), "assigned invariants"))
    return sorted(values)


def selected_behavior_refs(
    *,
    test: Mapping[str, Any],
    assignment: Mapping[str, Any] | None,
) -> list[str]:
    values = set(
        strings(test.get("mapping_behavior_refs"), "catalog behavior refs")
    )
    if assignment is not None:
        values.update(strings(assignment.get("behavior_refs"), "assigned refs"))
    return sorted(values)


def behavior_rationale(
    invariants: Mapping[str, Mapping[str, Any]],
    behavior_ref: str,
) -> str:
    invariant_id, separator, raw_index = behavior_ref.partition("#behavior:")
    if not separator or not raw_index.isdigit():
        raise ValueError(f"invalid behavior reference {behavior_ref}")
    behaviors = records(invariants[invariant_id].get("behavior"), "behaviors")
    return string(
        behaviors[int(raw_index)],
        "rationale",
        behavior_ref,
    )
