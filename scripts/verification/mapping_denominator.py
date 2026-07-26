"""Deterministic reconciliation of the live test-catalog denominator."""

from __future__ import annotations

import copy
from collections.abc import Mapping, Sequence
from typing import Any

from .test_catalog_denominator import (
    DATE_RE,
    NONMAPPING_REASON_DEFINITIONS,
    ROOT_CONSTS,
    ROOT_FIELDS,
)
from .test_catalog_models import InferredTestFact


def reconcile_denominator_document(
    *,
    document: Mapping[str, Any],
    facts: Sequence[InferredTestFact],
    tests: Sequence[Mapping[str, Any]],
    ignored_tests: Sequence[Mapping[str, Any]] | Mapping[str, Mapping[str, Any]],
    reviewed_date: str,
) -> dict[str, Any]:
    """Generate the exhaustive structural partition without semantic inference."""

    if not DATE_RE.fullmatch(reviewed_date):
        raise ValueError("denominator reconciliation requires YYYY-MM-DD review date")
    if set(document) != ROOT_FIELDS:
        raise ValueError("denominator reconciliation root fields drift")
    for field, expected in ROOT_CONSTS.items():
        if document.get(field) != expected:
            raise ValueError(
                f"denominator reconciliation {field} must equal {expected!r}"
            )
    if document.get("reason_definitions") != NONMAPPING_REASON_DEFINITIONS:
        raise ValueError("denominator reconciliation reason definitions drift")

    facts_by_id: dict[str, InferredTestFact] = {}
    for fact in facts:
        if fact.stable_id in facts_by_id:
            raise ValueError(
                f"denominator reconciliation duplicates live fact {fact.stable_id}"
            )
        facts_by_id[fact.stable_id] = fact

    catalog_by_discovery: dict[str, str] = {}
    for record in tests:
        if record.get("subject_kind") != "generated_test":
            continue
        test_id = _required_string(record, "id", "catalog record")
        discovery_id = _required_string(record, "discovery_id", test_id)
        if discovery_id not in facts_by_id:
            raise ValueError(
                f"catalog mapping {test_id} is absent from live scanner facts"
            )
        previous = catalog_by_discovery.get(discovery_id)
        if previous is not None:
            raise ValueError(
                f"live fact {discovery_id} is catalog-mapped by both "
                f"{previous} and {test_id}"
            )
        catalog_by_discovery[discovery_id] = test_id

    ignored_records = (
        ignored_tests.values()
        if isinstance(ignored_tests, Mapping)
        else ignored_tests
    )
    ignored_by_discovery: dict[str, Mapping[str, Any]] = {}
    for record in ignored_records:
        discovery_id = _required_string(
            record, "discovery_id", "ignored-test registry row"
        )
        if discovery_id in ignored_by_discovery:
            raise ValueError(
                f"ignored-test registry duplicates {discovery_id}"
            )
        ignored_by_discovery[discovery_id] = record

    existing_reviews = document.get("reviews")
    if not isinstance(existing_reviews, Sequence) or isinstance(
        existing_reviews, (str, bytes)
    ):
        raise ValueError("denominator reconciliation requires review rows")
    existing_by_discovery: dict[str, Mapping[str, Any]] = {}
    for index, review in enumerate(existing_reviews):
        if not isinstance(review, Mapping):
            raise ValueError(f"denominator review {index} must be a table")
        discovery_id = _required_string(
            review, "discovery_id", f"denominator review {index}"
        )
        if discovery_id in existing_by_discovery:
            raise ValueError(f"denominator reviews duplicate {discovery_id}")
        existing_by_discovery[discovery_id] = review

    reconciled: list[dict[str, Any]] = []
    for discovery_id, fact in sorted(facts_by_id.items()):
        common = {
            "discovery_id": discovery_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "ignore_state": fact.ignore_state,
        }
        catalog_test_id = catalog_by_discovery.get(discovery_id)
        if catalog_test_id is not None:
            semantic = {
                **common,
                "disposition": "catalog_mapped",
                "catalog_test_id": catalog_test_id,
            }
        else:
            semantic = {
                **common,
                "disposition": "reviewed_nonmapping",
                "reason_code": _structural_nonmapping_reason(
                    fact,
                    ignored_by_discovery.get(discovery_id),
                ),
            }
        previous = existing_by_discovery.get(discovery_id)
        previous_date = (
            previous.get("last_reviewed")
            if isinstance(previous, Mapping)
            else None
        )
        if (
            isinstance(previous_date, str)
            and DATE_RE.fullmatch(previous_date)
            and _without_review_date(previous) == semantic
        ):
            row_date = previous_date
        else:
            row_date = reviewed_date
        reconciled.append({**semantic, "last_reviewed": row_date})

    updated = copy.deepcopy(dict(document))
    changed = list(existing_reviews) != reconciled
    updated["reviews"] = reconciled
    if changed:
        updated["last_reviewed"] = reviewed_date
    return updated


def rebind_reviewed_nonmapping_identity(
    *,
    document: Mapping[str, Any],
    old_discovery_id: str,
    old_name: str,
    new_fact: InferredTestFact,
    reviewed_date: str,
) -> dict[str, Any]:
    """Mechanically rebind one renamed, still-unmapped scanner identity."""

    if not DATE_RE.fullmatch(reviewed_date):
        raise ValueError("denominator rebind requires YYYY-MM-DD review date")
    reviews = document.get("reviews")
    if not isinstance(reviews, Sequence) or isinstance(reviews, (str, bytes)):
        raise ValueError("denominator rebind requires review rows")
    matches = [
        (index, row)
        for index, row in enumerate(reviews)
        if isinstance(row, Mapping)
        and row.get("discovery_id") == old_discovery_id
    ]
    if len(matches) != 1:
        raise ValueError("denominator rebind requires exactly one old identity")
    if any(
        isinstance(row, Mapping)
        and row.get("discovery_id") == new_fact.stable_id
        for row in reviews
    ):
        raise ValueError("denominator rebind new identity already exists")

    index, old = matches[0]
    if (
        old.get("disposition") != "reviewed_nonmapping"
        or old.get("name") != old_name
        or old.get("path") != new_fact.path
        or old.get("discovery_source_kind") != new_fact.source_kind
        or old.get("ignore_state") != new_fact.ignore_state
    ):
        raise ValueError(
            "denominator rebind only permits an exact unmapped name change"
        )

    rebound = dict(old)
    rebound["discovery_id"] = new_fact.stable_id
    rebound["name"] = new_fact.name
    rebound["last_reviewed"] = reviewed_date
    updated_reviews = [dict(row) for row in reviews]
    updated_reviews[index] = rebound

    updated = copy.deepcopy(dict(document))
    updated["reviews"] = sorted(
        updated_reviews, key=lambda row: str(row.get("discovery_id", ""))
    )
    updated["last_reviewed"] = reviewed_date
    return updated


def _structural_nonmapping_reason(
    fact: InferredTestFact,
    ignored: Mapping[str, Any] | None,
) -> str:
    if fact.ignore_state != "not_ignored":
        if ignored is None or ignored.get("ignore_state") != fact.ignore_state:
            raise ValueError(
                f"ignored live fact {fact.stable_id} lacks exact ignored registry binding"
            )
        return "ignored_test_registry_owned"
    if fact.source_kind == "fuzz_target":
        return "fuzz_program_owned"
    if fact.source_kind in {"gate_script", "github_workflow_job"}:
        return "gate_inventory_owned"
    return "no_reviewed_spec_or_invariant_binding"


def _without_review_date(value: Mapping[str, Any]) -> dict[str, Any]:
    return {key: item for key, item in value.items() if key != "last_reviewed"}


def _required_string(
    value: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    result = value.get(field)
    if not isinstance(result, str) or not result:
        raise ValueError(f"{label} requires non-empty {field}")
    return result
