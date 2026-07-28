"""Resolve reviewed exact behavior routes to live catalog identities."""

from __future__ import annotations

import json
import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")


def build_exact_assignment_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
    routes: Mapping[str, tuple[str, ...]],
    expected_remaining: int,
) -> dict[str, Any]:
    """Resolve a reviewed route map without mutating repository metadata."""

    root = root.resolve()
    report = _json(gap_report)
    frozen_demands = _records(
        report.get("confirmed_missing_tests"),
        "confirmed missing tests",
    )
    frozen_refs = {
        _string(row, "behavior_ref", "confirmed missing test")
        for row in frozen_demands
    }
    route_refs = set(routes)
    if not route_refs <= frozen_refs:
        raise ValueError(
            "exact routes are not a frozen demand subset: "
            f"{sorted(route_refs - frozen_refs)!r}"
        )
    actual_remaining = len(frozen_refs - route_refs)
    if actual_remaining != expected_remaining:
        raise ValueError(
            "exact route residual drifted: "
            f"{actual_remaining}, expected {expected_remaining}"
        )

    catalog = _toml(root / CATALOG_PATH)
    catalog_tests = _records(catalog.get("tests"), "catalog tests")
    denominator = _toml(root / DENOMINATOR_PATH)
    denominator_reviews = _records(
        denominator.get("reviews"),
        "denominator reviews",
    )
    scan = _json(scan_report)
    scan_facts = _records(scan.get("inferred_facts"), "live scanner facts")

    catalog_by_name: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    scan_by_name: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    scan_by_identity: dict[tuple[str, str], list[Mapping[str, Any]]] = (
        defaultdict(list)
    )
    denominator_by_discovery: dict[str, Mapping[str, Any]] = {}
    for test in catalog_tests:
        name = test.get("name")
        if isinstance(name, str) and name:
            catalog_by_name[name].append(test)
    for fact in scan_facts:
        name = fact.get("name")
        path = fact.get("path")
        if isinstance(name, str) and name:
            scan_by_name[name].append(fact)
            if isinstance(path, str) and path:
                scan_by_identity[(path, name)].append(fact)
    for review in denominator_reviews:
        discovery_id = _string(
            review,
            "discovery_id",
            "denominator review",
        )
        if discovery_id in denominator_by_discovery:
            raise ValueError(
                f"denominator duplicates discovery {discovery_id}"
            )
        denominator_by_discovery[discovery_id] = review

    assignments_by_discovery: dict[str, dict[str, Any]] = {}
    unresolved: list[str] = []
    route_names: set[str] = set()
    assignment_link_count = 0
    for behavior_ref, names in sorted(routes.items()):
        invariant_id = behavior_ref.partition("#behavior:")[0]
        if len(names) != len(set(names)):
            raise ValueError(f"{behavior_ref} repeats an exact test name")
        route_names.update(names)
        assignment_link_count += len(names)
        for name in names:
            try:
                resolved = resolve_exact_test(
                    invariant_id=invariant_id,
                    name=name,
                    catalog_by_name=catalog_by_name,
                    scan_by_name=scan_by_name,
                    scan_by_identity=scan_by_identity,
                    denominator_by_discovery=denominator_by_discovery,
                )
            except ValueError as error:
                unresolved.append(f"{behavior_ref}: {error}")
                continue
            discovery_id = _string(
                resolved,
                "discovery_id",
                f"{behavior_ref}:{name}",
            )
            existing = assignments_by_discovery.get(discovery_id)
            if existing is None:
                existing = dict(resolved)
                existing["behavior_refs"] = []
                assignments_by_discovery[discovery_id] = existing
            elif any(
                existing[field] != resolved[field]
                for field in (
                    "path",
                    "name",
                    "resolution",
                    "catalog_test_id",
                )
            ):
                raise ValueError(
                    f"{discovery_id} resolved to inconsistent live identities"
                )
            existing["behavior_refs"].append(behavior_ref)

    assignments = []
    for discovery_id, assignment in sorted(assignments_by_discovery.items()):
        refs = sorted(set(assignment.pop("behavior_refs")))
        assignments.append(
            {
                **assignment,
                "behavior_refs": refs,
                "invariant_ids": sorted(
                    {ref.partition("#behavior:")[0] for ref in refs}
                ),
            }
        )
    new_assignments = [
        assignment
        for assignment in assignments
        if assignment["resolution"] == "new_catalog"
    ]
    dispositions = Counter(
        (
            "absent_from_denominator"
            if assignment["denominator_disposition"] is None
            else str(assignment["denominator_disposition"])
        )
        for assignment in new_assignments
    )
    new_by_invariant = Counter(
        invariant_id
        for assignment in new_assignments
        for invariant_id in assignment["invariant_ids"]
    )
    return {
        "demand_count": len(route_refs),
        "route_name_count": len(route_names),
        "assigned_test_count": len(assignments),
        "existing_catalog_test_count": sum(
            assignment["resolution"] == "existing_catalog"
            for assignment in assignments
        ),
        "new_catalog_test_count": len(new_assignments),
        "assignment_link_count": assignment_link_count,
        "new_denominator_dispositions": dict(sorted(dispositions.items())),
        "new_tests_by_invariant": dict(sorted(new_by_invariant.items())),
        "expected_post_transaction_missing_tests": expected_remaining,
        "assignments": assignments,
        "unresolved": unresolved,
    }


def resolve_exact_test(
    *,
    invariant_id: str,
    name: str,
    catalog_by_name: Mapping[str, Sequence[Mapping[str, Any]]],
    scan_by_name: Mapping[str, Sequence[Mapping[str, Any]]],
    scan_by_identity: Mapping[
        tuple[str, str],
        Sequence[Mapping[str, Any]],
    ],
    denominator_by_discovery: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Resolve one reviewed name to an exact live scanner identity."""

    catalog_candidates = list(catalog_by_name.get(name, ()))
    already_linked = [
        test
        for test in catalog_candidates
        if invariant_id in test.get("invariants", [])
    ]
    if len(already_linked) == 1:
        catalog_test = already_linked[0]
    elif len(already_linked) > 1:
        raise ValueError(
            f"{name!r} has multiple catalog rows linked to {invariant_id}"
        )
    elif len(catalog_candidates) == 1:
        catalog_test = catalog_candidates[0]
    elif len(catalog_candidates) > 1:
        raise ValueError(
            f"{name!r} has {len(catalog_candidates)} ambiguous catalog rows"
        )
    else:
        catalog_test = None

    if catalog_test is not None:
        path = _string(catalog_test, "path", f"catalog test {name}")
        facts = list(scan_by_identity.get((path, name), ()))
        if len(facts) != 1:
            raise ValueError(
                f"{path}:{name} expected one live scanner fact, found {len(facts)}"
            )
        fact = facts[0]
        discovery_id = _string(
            fact,
            "stable_id",
            f"scanner fact {path}:{name}",
        )
        if discovery_id != _string(
            catalog_test,
            "discovery_id",
            f"catalog test {name}",
        ):
            raise ValueError(
                f"{name!r} catalog/scanner discovery identity drifted"
            )
        review = denominator_by_discovery.get(discovery_id)
        if (
            review is None
            or review.get("disposition") != "catalog_mapped"
            or review.get("catalog_test_id") != catalog_test.get("id")
        ):
            raise ValueError(
                f"{name!r} existing catalog denominator binding drifted"
            )
        return {
            "discovery_id": discovery_id,
            "source_kind": _string(
                fact,
                "source_kind",
                f"scanner fact {name}",
            ),
            "path": path,
            "name": name,
            "command_hint": _string(
                fact,
                "command_hint",
                f"scanner fact {name}",
            ),
            "resolution": "existing_catalog",
            "catalog_test_id": _string(
                catalog_test,
                "id",
                f"catalog test {name}",
            ),
            "denominator_disposition": "catalog_mapped",
        }

    facts = list(scan_by_name.get(name, ()))
    if len(facts) != 1:
        raise ValueError(
            f"{name!r} expected one uncataloged scanner fact, found {len(facts)}"
        )
    fact = facts[0]
    discovery_id = _string(
        fact,
        "stable_id",
        f"scanner fact {name}",
    )
    review = denominator_by_discovery.get(discovery_id)
    if review is not None:
        if review.get("disposition") != "reviewed_nonmapping":
            raise ValueError(
                f"{name!r} uncataloged denominator is not reviewed_nonmapping"
            )
        if (
            review.get("path") != fact.get("path")
            or review.get("name") != name
            or review.get("discovery_source_kind") != fact.get("source_kind")
        ):
            raise ValueError(f"{name!r} denominator/scanner identity drifted")
    return {
        "discovery_id": discovery_id,
        "source_kind": _string(
            fact,
            "source_kind",
            f"scanner fact {name}",
        ),
        "path": _string(fact, "path", f"scanner fact {name}"),
        "name": name,
        "command_hint": _string(
            fact,
            "command_hint",
            f"scanner fact {name}",
        ),
        "resolution": "new_catalog",
        "catalog_test_id": None,
        "denominator_disposition": (
            None if review is None else review.get("disposition")
        ),
    }


def _json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
    return document


def _toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an object list")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} contains a non-object")
    return list(value)


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value
