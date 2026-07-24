"""Exact existing-authority checks for projected code-spec transitions."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from typing import Any


ORACLE_AUTHORITIES = frozenset(
    {
        "normative_external",
        "normative_product",
        "reviewed_decision",
        "reviewed_deviation",
    }
)


def oracle_eligible(source: Mapping[str, Any] | None) -> bool:
    return bool(
        source
        and source.get("source_status") == "active"
        and source.get("oracle_eligible") is True
        and source.get("authority") in ORACLE_AUTHORITIES
    )


def validate_selected_invariants(
    *,
    code_id: str,
    invariant_ids: list[str],
    invariants: Mapping[str, Mapping[str, Any]],
    area: str,
    spec_ref: str,
    source_id: str,
) -> dict[str, Mapping[str, Any]]:
    selected = {}
    for invariant_id in invariant_ids:
        invariant = invariants.get(invariant_id)
        if invariant is None:
            raise ValueError(f"{code_id} unknown invariant {invariant_id}")
        if invariant.get("area") != area:
            raise ValueError(f"{code_id} invariant area does not match code area")
        spec = invariant.get("spec")
        source_refs = (
            spec.get("source_refs") if isinstance(spec, Mapping) else None
        )
        if (
            not isinstance(spec, Mapping)
            or spec.get("status") != "specified"
            or not isinstance(source_refs, list)
            or source_id not in source_refs
        ):
            raise ValueError(
                f"{code_id} invariant {invariant_id} lacks specified source "
                "authority"
            )
        oracle = invariant.get("oracle")
        if not isinstance(oracle, Mapping) or oracle.get("ref") != spec_ref:
            raise ValueError(
                f"{code_id} invariant {invariant_id} oracle does not match "
                "spec_ref"
            )
        invariant_tests = invariant.get("tests")
        if (
            not isinstance(invariant_tests, list)
            or any(
                not isinstance(test_id, str) or not test_id
                for test_id in invariant_tests
            )
            or len(set(invariant_tests)) != len(invariant_tests)
        ):
            raise ValueError(
                f"{code_id} invariant {invariant_id} tests are malformed"
            )
        selected[invariant_id] = invariant
    return selected


def catalog_tests_by_discovery(
    tests: Iterable[Mapping[str, Any]],
) -> dict[str, str]:
    indexed = {}
    for test in tests:
        discovery_id = test.get("discovery_id")
        if discovery_id is None:
            continue
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("test catalog contains malformed discovery_id")
        test_id = test.get("id")
        if not isinstance(test_id, str) or not test_id:
            raise ValueError("test catalog row requires non-empty id")
        previous = indexed.get(discovery_id)
        if previous is not None:
            raise ValueError(
                f"test catalog discovery collision {discovery_id}: "
                f"{previous}, {test_id}"
            )
        indexed[discovery_id] = test_id
    return indexed


def validate_discoveries_and_tests(
    *,
    code_id: str,
    discovery_ids: list[str],
    test_ids: list[str],
    denominator_reviews: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    tests_by_discovery: Mapping[str, str],
    invariant_ids: list[str],
    invariants: Mapping[str, Mapping[str, Any]],
    area: str,
    spec_ref: str,
) -> tuple[list[str], list[str]]:
    missing_tests: list[str] = []
    missing_backlinks: list[str] = []
    for discovery_id, test_id in zip(discovery_ids, test_ids, strict=True):
        review = denominator_reviews.get(discovery_id)
        if review is None:
            raise ValueError(f"{code_id} unknown discovery_id {discovery_id}")
        disposition = review.get("disposition")
        if disposition == "reviewed_nonmapping":
            if (
                review.get("ignore_state") != "not_ignored"
                or review.get("reason_code")
                != "no_reviewed_spec_or_invariant_binding"
            ):
                raise ValueError(
                    f"{code_id} discovery {discovery_id} is disallowed for "
                    "mapping"
                )
        elif disposition == "catalog_mapped":
            if review.get("catalog_test_id") != test_id:
                raise ValueError(
                    f"{code_id} catalog_mapped discovery {discovery_id} "
                    f"does not bind derived test {test_id}"
                )
        else:
            raise ValueError(
                f"{code_id} discovery {discovery_id} is not "
                "reviewed_nonmapping or catalog_mapped"
            )
        test = tests.get(test_id)
        if test is None:
            if disposition == "catalog_mapped":
                raise ValueError(
                    f"{code_id} catalog_mapped discovery {discovery_id} "
                    f"lacks exact catalog authority for {test_id}"
                )
            collision = tests_by_discovery.get(discovery_id)
            if collision is not None:
                raise ValueError(
                    f"{code_id} discovery collision {discovery_id} with "
                    f"catalog test {collision}"
                )
            missing_tests.append(test_id)
        else:
            linked = test.get("invariants")
            if (
                test.get("subject_kind") != "generated_test"
                or test.get("discovery_id") != discovery_id
                or tests_by_discovery.get(discovery_id) != test_id
                or test.get("status") != "mapped"
                or test.get("area") != area
                or test.get("oracle_ref") != spec_ref
                or not isinstance(linked, list)
                or any(
                    not isinstance(invariant_id, str) or not invariant_id
                    for invariant_id in linked
                )
                or len(set(linked)) != len(linked)
                or not set(invariant_ids).issubset(linked)
            ):
                raise ValueError(
                    f"{code_id} derived test {test_id} lacks exact catalog "
                    "authority"
                )
        for invariant_id in invariant_ids:
            invariant_tests = invariants[invariant_id].get("tests")
            if not isinstance(invariant_tests, list):
                raise ValueError(
                    f"{code_id} invariant {invariant_id} tests are malformed"
                )
            if test_id not in invariant_tests:
                missing_backlinks.append(f"{invariant_id}:{test_id}")
    return sorted(missing_tests), sorted(missing_backlinks)
