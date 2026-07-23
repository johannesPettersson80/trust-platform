"""Semantic acceptance and rejection contracts for mapping proposals."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from typing import Any

from .metadata_validator.constants import HIGH_RISKS


DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMON_DECISION_FIELDS = frozenset(
    {
        "schema_version",
        "id",
        "proposal_id",
        "proposal_digest",
        "discovery_id",
        "outcome",
        "reviewer_ids",
        "reviewed_source_digest",
        "assertion_reviews",
        "selected_invariant_ids",
        "selected_behavior_refs",
        "command",
        "command_authority",
    }
)
ACCEPTED_DECISION_FIELDS = COMMON_DECISION_FIELDS | {"confirmed_catalog"}
REJECTED_DECISION_FIELDS = COMMON_DECISION_FIELDS | {"rejection_reason"}
CONFIRMED_CATALOG_FIELDS = frozenset(
    {
        "test_class",
        "area",
        "owner",
        "expected_result",
        "expected_failure_mode",
        "suite_tiers",
        "requires_hardware",
        "requires_network",
        "duration_class",
    }
)
COMMON_REVIEW_INPUT_FIELDS = frozenset(
    {
        "proposal_id",
        "outcome",
        "reviewer_ids",
        "assertion_reviews",
        "selected_invariant_ids",
        "selected_behavior_refs",
        "command",
        "command_authority",
    }
)
ASSERTION_REVIEW_FIELDS = frozenset(
    {"assertion_id", "disposition", "behavior_refs", "rationale"}
)
ASSERTION_DISPOSITIONS = frozenset(
    {"oracle_bound", "setup_or_precondition", "non_oracle"}
)


def build_mapping_decision(
    *,
    proposal: Mapping[str, Any],
    review_input: Mapping[str, Any],
) -> dict[str, Any]:
    outcome = review_input.get("outcome")
    expected_fields = set(COMMON_REVIEW_INPUT_FIELDS)
    if outcome == "accepted":
        expected_fields.add("confirmed_catalog")
    elif outcome == "rejected":
        expected_fields.add("rejection_reason")
    else:
        raise ValueError("review input outcome must be accepted or rejected")
    if set(review_input) != expected_fields:
        raise ValueError("review input fields drift from closed contract")
    proposal_id = proposal.get("id")
    if review_input.get("proposal_id") != proposal_id:
        raise ValueError("review input proposal ID does not match selected proposal")
    semantic: dict[str, Any] = {
        "schema_version": 1,
        "proposal_id": proposal_id,
        "proposal_digest": proposal.get("proposal_digest"),
        "discovery_id": proposal.get("discovery_id"),
        "outcome": outcome,
        "reviewer_ids": sorted(
            _required_review_strings(review_input, "reviewer_ids")
        ),
        "reviewed_source_digest": proposal.get("test_source_digest"),
        "assertion_reviews": _normalized_assertion_reviews(
            review_input.get("assertion_reviews")
        ),
        "selected_invariant_ids": sorted(
            _required_review_strings(
                review_input, "selected_invariant_ids", allow_empty=True
            )
        ),
        "selected_behavior_refs": sorted(
            _required_review_strings(
                review_input, "selected_behavior_refs", allow_empty=True
            )
        ),
        "command": review_input.get("command"),
        "command_authority": review_input.get("command_authority"),
    }
    if outcome == "accepted":
        confirmed = review_input.get("confirmed_catalog")
        if not isinstance(confirmed, Mapping) or set(confirmed) != CONFIRMED_CATALOG_FIELDS:
            raise ValueError("review input confirmed catalog fields drift")
        semantic["confirmed_catalog"] = {
            field: (
                sorted(confirmed[field])
                if field == "suite_tiers"
                and isinstance(confirmed[field], Sequence)
                and not isinstance(confirmed[field], (str, bytes))
                else confirmed[field]
            )
            for field in CONFIRMED_CATALOG_FIELDS
        }
    else:
        semantic["rejection_reason"] = review_input.get("rejection_reason")
    return {"id": stable_decision_id(semantic), **semantic}


def stable_decision_id(record: Mapping[str, Any]) -> str:
    """Generate the decision identity from all semantic fields, excluding ID."""

    semantic = {key: value for key, value in record.items() if key != "id"}
    encoded = json.dumps(
        semantic, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    digest = hashlib.sha256(encoded).hexdigest().upper()
    return "MAP_DEC_" + digest[:24]


def validate_mapping_decisions(
    *,
    decisions: Sequence[Mapping[str, Any]],
    proposals: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    candidate_generator_id: str,
) -> list[str]:
    """Validate exact per-test decisions before any catalog mutation."""

    failures: list[str] = []
    seen_ids: set[str] = set()
    seen_proposals: set[str] = set()
    for index, decision in enumerate(decisions):
        label = f"decision[{index}]"
        expected_fields = (
            ACCEPTED_DECISION_FIELDS
            if decision.get("outcome") == "accepted"
            else REJECTED_DECISION_FIELDS
            if decision.get("outcome") == "rejected"
            else COMMON_DECISION_FIELDS
        )
        if set(decision) != expected_fields:
            failures.append(f"{label} fields drift from closed review contract")
        decision_id = decision.get("id")
        if not isinstance(decision_id, str) or decision_id != stable_decision_id(decision):
            failures.append(f"{label} does not use its generated decision ID")
        elif decision_id in seen_ids:
            failures.append(f"{label} duplicates decision ID {decision_id}")
        else:
            seen_ids.add(decision_id)
            label = decision_id
        if decision.get("schema_version") != 1:
            failures.append(f"{label} must use schema_version 1")
        proposal_id = decision.get("proposal_id")
        if not isinstance(proposal_id, str) or proposal_id not in proposals:
            failures.append(f"{label} references unknown proposal")
            continue
        if proposal_id in seen_proposals:
            failures.append(f"{label} duplicates a decision for proposal {proposal_id}")
        seen_proposals.add(proposal_id)
        proposal = proposals[proposal_id]
        if decision.get("proposal_digest") != proposal.get("proposal_digest"):
            failures.append(f"{label} proposal digest does not match quarantine")
        if decision.get("discovery_id") != proposal.get("discovery_id"):
            failures.append(f"{label} discovery identity does not match proposal")
        if decision.get("reviewed_source_digest") != proposal.get("test_source_digest"):
            failures.append(f"{label} reviewed source digest does not match proposal")
        reviewers = _strings(decision.get("reviewer_ids"), f"{label} reviewers", failures)
        if reviewers != sorted(set(reviewers)):
            failures.append(f"{label} reviewer IDs must be sorted and distinct")
        if candidate_generator_id in reviewers:
            failures.append(f"{label} candidate generator cannot review its proposal")
        assertion_reviews = _validate_assertion_reviews(
            decision.get("assertion_reviews"),
            proposal=proposal,
            label=label,
            failures=failures,
            require_complete=decision.get("outcome") == "accepted",
        )

        outcome = decision.get("outcome")
        if outcome == "accepted":
            _validate_acceptance(
                decision=decision,
                proposal=proposal,
                invariants=invariants,
                reviewers=reviewers,
                assertion_reviews=assertion_reviews,
                label=label,
                failures=failures,
            )
        elif outcome == "rejected":
            _validate_rejection(decision, reviewers, label, failures)
        else:
            failures.append(f"{label} outcome must be accepted or rejected")
    return sorted(set(failures))


def suppressed_proposal_digests(
    decisions: Sequence[Mapping[str, Any]],
) -> set[str]:
    """Return only exact rejected proposal digests for candidate suppression."""

    return {
        str(decision["proposal_digest"])
        for decision in decisions
        if decision.get("outcome") == "rejected"
        and isinstance(decision.get("proposal_digest"), str)
        and DIGEST_RE.fullmatch(str(decision["proposal_digest"]))
    }


def _validate_acceptance(
    *,
    decision: Mapping[str, Any],
    proposal: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
    reviewers: list[str],
    assertion_reviews: list[Mapping[str, Any]],
    label: str,
    failures: list[str],
) -> None:
    selected = _strings(
        decision.get("selected_invariant_ids"),
        f"{label} selected invariants",
        failures,
    )
    candidate_ids = set(_plain_strings(proposal.get("candidate_invariant_ids")))
    if not selected or not set(selected).issubset(candidate_ids):
        failures.append(f"{label} selected invariants are not an exact candidate subset")
    behavior_refs = _strings(
        decision.get("selected_behavior_refs"),
        f"{label} selected behavior references",
        failures,
    )
    proposal_behavior_refs = set(_plain_strings(proposal.get("behavior_refs")))
    if not behavior_refs or not set(behavior_refs).issubset(proposal_behavior_refs):
        failures.append(f"{label} selected behavior references do not match proposal")
    for behavior_ref in behavior_refs:
        invariant_id = behavior_ref.partition("#behavior:")[0]
        if invariant_id not in selected:
            failures.append(
                f"{label} selected behavior reference lacks selected invariant"
            )
    bound = [
        review
        for review in assertion_reviews
        if review.get("disposition") == "oracle_bound"
    ]
    if not bound:
        failures.append(f"{label} acceptance requires an oracle-bound assertion")
    for review in assertion_reviews:
        assertion_id = review.get("assertion_id")
        refs = _plain_strings(review.get("behavior_refs"))
        disposition = review.get("disposition")
        if disposition == "oracle_bound":
            if not refs or not set(refs).issubset(set(behavior_refs)):
                failures.append(
                    f"{label} assertion {assertion_id} must bind selected behavior refs"
                )
        elif refs:
            failures.append(
                f"{label} non-oracle assertion {assertion_id} forbids behavior refs"
            )
    unresolved = _plain_strings(proposal.get("unresolved_assertion_helpers"))
    if unresolved:
        failures.append(
            f"{label} acceptance is blocked by unresolved assertion helpers"
        )
    risks = {
        invariants[invariant_id].get("risk")
        for invariant_id in selected
        if invariant_id in invariants
    }
    if any(risk in HIGH_RISKS for risk in risks) and len(set(reviewers)) < 2:
        failures.append(f"{label} high-risk acceptance requires two distinct reviewers")
    if not reviewers:
        failures.append(f"{label} acceptance requires a named reviewer")
    if decision.get("command_authority") != "verified_exact":
        failures.append(f"{label} acceptance requires verified_exact command authority")
    if not isinstance(decision.get("command"), str) or not decision.get("command"):
        failures.append(f"{label} acceptance requires an exact executable command")
    elif decision.get("command") != proposal.get("command"):
        failures.append(f"{label} command does not match traced proposal")
    confirmed = decision.get("confirmed_catalog")
    if not isinstance(confirmed, Mapping) or set(confirmed) != CONFIRMED_CATALOG_FIELDS:
        failures.append(f"{label} accepted catalog fields drift from contract")
    else:
        for field in (
            "test_class",
            "area",
            "owner",
            "expected_result",
            "expected_failure_mode",
            "duration_class",
        ):
            if not isinstance(confirmed.get(field), str) or not confirmed.get(field):
                failures.append(f"{label} confirmed catalog requires non-empty {field}")
        tiers = _strings(
            confirmed.get("suite_tiers"), f"{label} suite tiers", failures
        )
        if not tiers:
            failures.append(f"{label} confirmed catalog requires suite tier")
        for field in ("requires_hardware", "requires_network"):
            if not isinstance(confirmed.get(field), bool):
                failures.append(f"{label} confirmed catalog requires boolean {field}")
        if confirmed.get("duration_class") != proposal.get("duration_class"):
            failures.append(
                f"{label} duration class does not match traced proposal"
            )


def _validate_rejection(
    decision: Mapping[str, Any],
    reviewers: list[str],
    label: str,
    failures: list[str],
) -> None:
    if len(set(reviewers)) < 1:
        failures.append(f"{label} rejection requires a named reviewer")
    if _plain_strings(decision.get("selected_invariant_ids")):
        failures.append(f"{label} rejection forbids selected invariants")
    if _plain_strings(decision.get("selected_behavior_refs")):
        failures.append(f"{label} rejection forbids selected behavior references")
    if decision.get("command") != "" or decision.get("command_authority") != "not_applicable":
        failures.append(f"{label} rejection command must be not_applicable")
    reason = decision.get("rejection_reason")
    if not isinstance(reason, str) or not reason:
        failures.append(f"{label} rejection requires a reason")


def _strings(value: Any, label: str, failures: list[str]) -> list[str]:
    result = _plain_strings(value)
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        failures.append(f"{label} must be an array")
    elif len(result) != len(value):
        failures.append(f"{label} must contain only non-empty strings")
    return result


def _plain_strings(value: Any) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return []
    return [item for item in value if isinstance(item, str) and item]


def _required_review_strings(
    record: Mapping[str, Any],
    field: str,
    *,
    allow_empty: bool = False,
) -> list[str]:
    value = record.get(field)
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"review input {field} must be an array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(
            f"review input {field} must contain distinct non-empty strings"
        )
    if not allow_empty and not result:
        raise ValueError(f"review input {field} must not be empty")
    return result


def _normalized_assertion_reviews(value: Any) -> list[dict[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, Mapping) for item in value)
    ):
        raise ValueError("review input assertion_reviews must be an object array")
    normalized = []
    for index, review in enumerate(value):
        if set(review) != ASSERTION_REVIEW_FIELDS:
            raise ValueError(
                f"review input assertion_reviews[{index}] fields drift"
            )
        assertion_id = review.get("assertion_id")
        disposition = review.get("disposition")
        rationale = review.get("rationale")
        if not isinstance(assertion_id, str) or not assertion_id:
            raise ValueError(
                f"review input assertion_reviews[{index}] requires assertion_id"
            )
        if disposition not in ASSERTION_DISPOSITIONS:
            raise ValueError(
                f"review input assertion_reviews[{index}] has invalid disposition"
            )
        if not isinstance(rationale, str) or not rationale:
            raise ValueError(
                f"review input assertion_reviews[{index}] requires rationale"
            )
        behavior_refs = _required_review_strings(review, "behavior_refs", allow_empty=True)
        normalized.append(
            {
                "assertion_id": assertion_id,
                "disposition": disposition,
                "behavior_refs": sorted(behavior_refs),
                "rationale": rationale,
            }
        )
    normalized.sort(key=lambda row: str(row["assertion_id"]))
    if len({str(row["assertion_id"]) for row in normalized}) != len(normalized):
        raise ValueError("review input assertion_reviews duplicate assertion IDs")
    return normalized


def _validate_assertion_reviews(
    value: Any,
    *,
    proposal: Mapping[str, Any],
    label: str,
    failures: list[str],
    require_complete: bool,
) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, Mapping) for item in value)
    ):
        failures.append(f"{label} assertion reviews must be an object array")
        return []
    reviews = list(value)
    ids: list[str] = []
    for index, review in enumerate(reviews):
        if set(review) != ASSERTION_REVIEW_FIELDS:
            failures.append(f"{label} assertion review[{index}] fields drift")
            continue
        assertion_id = review.get("assertion_id")
        if not isinstance(assertion_id, str) or not assertion_id:
            failures.append(f"{label} assertion review[{index}] lacks assertion ID")
            continue
        ids.append(assertion_id)
        if review.get("disposition") not in ASSERTION_DISPOSITIONS:
            failures.append(
                f"{label} assertion {assertion_id} has invalid disposition"
            )
        refs = _strings(
            review.get("behavior_refs"),
            f"{label} assertion {assertion_id} behavior refs",
            failures,
        )
        if refs != sorted(set(refs)):
            failures.append(
                f"{label} assertion {assertion_id} behavior refs must be sorted and distinct"
            )
        if not isinstance(review.get("rationale"), str) or not review.get(
            "rationale"
        ):
            failures.append(
                f"{label} assertion {assertion_id} requires review rationale"
            )
    if ids != sorted(set(ids)):
        failures.append(f"{label} assertion reviews must be sorted and distinct")
    proposal_ids = {
        str(assertion.get("assertion_id"))
        for assertion in proposal.get("assertions", [])
        if isinstance(assertion, Mapping)
        and isinstance(assertion.get("assertion_id"), str)
    }
    if not set(ids).issubset(proposal_ids):
        failures.append(f"{label} assertion review names an unknown assertion")
    if require_complete and set(ids) != proposal_ids:
        failures.append(
            f"{label} acceptance must review every generated assertion exactly once"
        )
    return reviews
