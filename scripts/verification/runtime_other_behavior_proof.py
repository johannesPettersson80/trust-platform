"""Plan the Phase 18 runtime-other exact behavior proof transaction."""

from __future__ import annotations

import json
import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .runtime_other_behavior_routes import BEHAVIOR_TEST_NAMES


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
SEMANTIC_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-other-106-semantic-lock.json"
)


def build_assignment_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Resolve the frozen tranche without changing repository metadata."""

    root = root.resolve()
    semantic_lock = root / SEMANTIC_LOCK_PATH
    if semantic_lock.is_file():
        return _assignment_plan_from_lock(semantic_lock)
    report = _json(gap_report)
    frozen_demands = _records(
        report.get("confirmed_missing_tests"),
        "confirmed missing tests",
    )
    frozen_refs = {
        _string(row, "behavior_ref", "confirmed missing test")
        for row in frozen_demands
    }
    route_refs = set(BEHAVIOR_TEST_NAMES)
    if not route_refs <= frozen_refs:
        raise ValueError(
            "runtime-other routes are not a frozen demand subset: "
            f"{sorted(route_refs - frozen_refs)!r}"
        )
    candidates = _json(
        root
        / "target/gate-artifacts/verification/p18-runtime-other-106/"
        "candidates.json"
    )
    candidate_demands = _records(
        candidates.get("demands"),
        "runtime-other candidate demands",
    )
    candidate_refs = {
        _string(row, "behavior_ref", "runtime-other candidate")
        for row in candidate_demands
    }
    if candidate_refs != route_refs or len(candidate_demands) != len(route_refs):
        raise ValueError(
            "runtime-other route population drifted from its frozen candidates"
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
        discovery_id = review.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("denominator review lacks discovery identity")
        if discovery_id in denominator_by_discovery:
            raise ValueError(
                f"denominator duplicates discovery {discovery_id}"
            )
        denominator_by_discovery[discovery_id] = review

    assignments_by_discovery: dict[str, dict[str, Any]] = {}
    unresolved: list[str] = []
    route_names: set[str] = set()
    assignment_link_count = 0
    for behavior_ref, names in sorted(BEHAVIOR_TEST_NAMES.items()):
        invariant_id = behavior_ref.partition("#behavior:")[0]
        if len(names) != len(set(names)):
            raise ValueError(f"{behavior_ref} repeats an exact test name")
        route_names.update(names)
        assignment_link_count += len(names)
        for name in names:
            try:
                resolved = _resolve_test(
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
        if len(refs) == 0:
            raise ValueError(f"{discovery_id} has no behavior assignment")
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
    expected_remaining = len(frozen_refs - route_refs)
    if expected_remaining != 90:
        raise ValueError(
            f"runtime-other expected residual drifted to {expected_remaining}"
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


def _assignment_plan_from_lock(path: Path) -> dict[str, Any]:
    lock = _json(path)
    if (
        lock.get("schema_version") != 1
        or lock.get("id") != "RUNTIME_OTHER_106_SEMANTIC_LOCK"
    ):
        raise ValueError("runtime-other semantic lock identity drifted")
    summary = lock.get("summary")
    if not isinstance(summary, Mapping):
        raise ValueError("runtime-other semantic lock lacks summary")
    assignments = [
        dict(row)
        for row in _records(lock.get("assignments"), "locked assignments")
    ]
    if (
        summary.get("behavior_demand_count") != 106
        or summary.get("campaign_test_count") != 196
        or summary.get("new_catalog_test_count") != 34
        or summary.get("existing_catalog_test_count") != 163
        or summary.get("confirmed_missing_test_count") != 90
        or len(assignments) != 196
    ):
        raise ValueError("runtime-other semantic lock population drifted")
    new_assignments = [
        row for row in assignments if row.get("resolution") == "new_catalog"
    ]
    if len(new_assignments) != 33:
        raise ValueError("runtime-other locked promotion population drifted")
    dispositions = Counter(
        (
            "absent_from_denominator"
            if row.get("denominator_disposition") is None
            else str(row["denominator_disposition"])
        )
        for row in new_assignments
    )
    new_by_invariant = Counter(
        invariant_id
        for row in new_assignments
        for invariant_id in _string_array(
            row.get("invariant_ids"), "locked invariant IDs"
        )
    )
    return {
        "demand_count": 106,
        "route_name_count": 196,
        "assigned_test_count": 196,
        "existing_catalog_test_count": 163,
        "new_catalog_test_count": 33,
        "assignment_link_count": 230,
        "new_denominator_dispositions": dict(sorted(dispositions.items())),
        "new_tests_by_invariant": dict(sorted(new_by_invariant.items())),
        "expected_post_transaction_missing_tests": 90,
        "assignments": assignments,
        "unresolved": [],
    }


def _resolve_test(
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
        catalog_discovery_id = _string(
            catalog_test,
            "discovery_id",
            f"catalog test {name}",
        )
        if discovery_id != catalog_discovery_id:
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


def _string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string list")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return sorted(result)


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value
