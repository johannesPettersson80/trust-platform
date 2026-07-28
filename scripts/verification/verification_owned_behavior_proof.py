"""Plan the exact Phase 18 verification-owned behavior transaction."""

from __future__ import annotations

import json
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .exact_behavior_assignments import (
    build_exact_assignment_plan,
    resolve_exact_test,
)
from .frozen_catalog_gaps import generate_report
from .verification_owned_behavior_routes import (
    BEHAVIOR_TEST_NAMES,
    PROVENANCE_REFRESH_TEST_NAMES,
)


SEMANTIC_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "verification-owned-11-semantic-lock.json"
)


def build_assignment_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Return the verification-owned assignment plan."""

    root = root.resolve()
    semantic_lock = root / SEMANTIC_LOCK_PATH
    if semantic_lock.is_file():
        return _assignment_plan_from_lock(semantic_lock)
    plan = build_exact_assignment_plan(
        root=root,
        gap_report=(
            gap_report if gap_report.is_file() else generate_report(root)
        ),
        scan_report=scan_report,
        routes=BEHAVIOR_TEST_NAMES,
        expected_remaining=27,
    )
    return _append_provenance_refresh(
        root=root,
        scan_report=scan_report,
        plan=plan,
    )


def _append_provenance_refresh(
    *,
    root: Path,
    scan_report: Path,
    plan: Mapping[str, Any],
) -> dict[str, Any]:
    catalog = tomllib.loads(
        (root / "verification/test-catalog.toml").read_text()
    )
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    scan = json.loads(scan_report.read_text())
    catalog_by_name: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    scan_by_name: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    scan_by_identity: dict[
        tuple[str, str],
        list[Mapping[str, Any]],
    ] = defaultdict(list)
    denominator_by_discovery: dict[str, Mapping[str, Any]] = {}
    for test in catalog.get("tests", []):
        if isinstance(test, Mapping) and isinstance(test.get("name"), str):
            catalog_by_name[str(test["name"])].append(test)
    for fact in scan.get("inferred_facts", []):
        if not isinstance(fact, Mapping):
            continue
        name = fact.get("name")
        path = fact.get("path")
        if isinstance(name, str):
            scan_by_name[name].append(fact)
            if isinstance(path, str):
                scan_by_identity[(path, name)].append(fact)
    for review in denominator.get("reviews", []):
        if isinstance(review, Mapping) and isinstance(
            review.get("discovery_id"),
            str,
        ):
            denominator_by_discovery[str(review["discovery_id"])] = review

    assignments = [
        dict(row)
        for row in plan.get("assignments", [])
        if isinstance(row, Mapping)
    ]
    seen = {str(row["discovery_id"]) for row in assignments}
    for behavior_ref, names in sorted(
        PROVENANCE_REFRESH_TEST_NAMES.items()
    ):
        invariant_id = behavior_ref.partition("#behavior:")[0]
        for name in names:
            resolved = resolve_exact_test(
                invariant_id=invariant_id,
                name=name,
                catalog_by_name=catalog_by_name,
                scan_by_name=scan_by_name,
                scan_by_identity=scan_by_identity,
                denominator_by_discovery=denominator_by_discovery,
            )
            discovery_id = str(resolved["discovery_id"])
            if (
                resolved.get("resolution") != "existing_catalog"
                or discovery_id in seen
            ):
                raise ValueError(
                    f"{name} is not one unique provenance refresh"
                )
            seen.add(discovery_id)
            assignments.append(
                {
                    **resolved,
                    "behavior_refs": [behavior_ref],
                    "invariant_ids": [invariant_id],
                }
            )
    assignments.sort(key=lambda row: str(row["discovery_id"]))
    return {
        **plan,
        "route_name_count": 15,
        "assigned_test_count": 15,
        "existing_catalog_test_count": 3,
        "assignment_link_count": 17,
        "assignments": assignments,
    }


def _assignment_plan_from_lock(path: Path) -> dict[str, Any]:
    lock = json.loads(path.read_text())
    if (
        not isinstance(lock, Mapping)
        or lock.get("schema_version") != 1
        or lock.get("id") != "VERIFICATION_OWNED_11_SEMANTIC_LOCK"
    ):
        raise ValueError("verification-owned semantic lock identity drifted")
    summary = lock.get("summary")
    assignments_value = lock.get("assignments")
    if not isinstance(summary, Mapping):
        raise ValueError("verification-owned semantic lock lacks summary")
    if (
        not isinstance(assignments_value, Sequence)
        or isinstance(assignments_value, (str, bytes))
        or any(not isinstance(row, Mapping) for row in assignments_value)
    ):
        raise ValueError("verification-owned lock assignments are invalid")
    assignments = [dict(row) for row in assignments_value]
    if (
        summary.get("behavior_demand_count") != 11
        or summary.get("campaign_test_count") != 15
        or summary.get("existing_catalog_test_count") != 3
        or summary.get("new_catalog_test_count") != 12
        or summary.get("confirmed_missing_test_count") != 27
        or len(assignments) != 15
    ):
        raise ValueError("verification-owned lock population drifted")
    assignment_link_count = sum(
        len(refs)
        for row in assignments
        for refs in [row.get("behavior_refs")]
        if isinstance(refs, list)
    )
    if assignment_link_count != 17:
        raise ValueError("verification-owned lock link count drifted")
    return {
        "demand_count": 11,
        "route_name_count": 15,
        "assigned_test_count": 15,
        "existing_catalog_test_count": 3,
        "new_catalog_test_count": 12,
        "assignment_link_count": 17,
        "new_denominator_dispositions": {
            "absent_from_denominator": 2,
            "reviewed_nonmapping": 10,
        },
        "new_tests_by_invariant": {
            "VERIFICATION_CASE_ARTIFACT_HELPER_001": 9,
            "VERIFICATION_IEC_TABLE_INVENTORY_001": 3,
        },
        "expected_post_transaction_missing_tests": 27,
        "assignments": assignments,
        "unresolved": [],
    }
