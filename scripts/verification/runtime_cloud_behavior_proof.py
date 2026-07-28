"""Plan the Phase 18 runtime-cloud exact behavior proof transaction."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .exact_behavior_assignments import build_exact_assignment_plan
from .frozen_catalog_gaps import generate_report
from .runtime_cloud_behavior_routes import BEHAVIOR_TEST_NAMES


SEMANTIC_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-cloud-23-semantic-lock.json"
)


def build_assignment_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Resolve all seven runtime-cloud authorities without changing metadata."""

    root = root.resolve()
    semantic_lock = root / SEMANTIC_LOCK_PATH
    if semantic_lock.is_file():
        return _assignment_plan_from_lock(semantic_lock)
    return build_exact_assignment_plan(
        root=root,
        gap_report=(
            gap_report
            if gap_report.is_file()
            else generate_report(root)
        ),
        scan_report=scan_report,
        routes=BEHAVIOR_TEST_NAMES,
        expected_remaining=67,
    )


def _assignment_plan_from_lock(path: Path) -> dict[str, Any]:
    lock = json.loads(path.read_text())
    if (
        not isinstance(lock, Mapping)
        or lock.get("schema_version") != 1
        or lock.get("id") != "RUNTIME_CLOUD_23_SEMANTIC_LOCK"
    ):
        raise ValueError("runtime-cloud semantic lock identity drifted")
    summary = lock.get("summary")
    assignments_value = lock.get("assignments")
    if not isinstance(summary, Mapping):
        raise ValueError("runtime-cloud semantic lock lacks summary")
    if (
        not isinstance(assignments_value, Sequence)
        or isinstance(assignments_value, (str, bytes))
        or any(not isinstance(row, Mapping) for row in assignments_value)
    ):
        raise ValueError("runtime-cloud semantic lock assignments are invalid")
    assignments = [dict(row) for row in assignments_value]
    if (
        summary.get("behavior_demand_count") != 23
        or summary.get("campaign_test_count") != 52
        or summary.get("existing_catalog_test_count") != 52
        or summary.get("confirmed_missing_test_count") != 67
        or len(assignments) != 52
    ):
        raise ValueError("runtime-cloud semantic lock population drifted")
    assignment_link_count = sum(
        len(refs)
        for row in assignments
        for refs in [row.get("behavior_refs")]
        if isinstance(refs, list)
    )
    if assignment_link_count != 53:
        raise ValueError("runtime-cloud semantic lock link count drifted")
    return {
        "demand_count": 23,
        "route_name_count": 52,
        "assigned_test_count": 52,
        "existing_catalog_test_count": 52,
        "new_catalog_test_count": 0,
        "assignment_link_count": 53,
        "new_denominator_dispositions": {},
        "new_tests_by_invariant": {},
        "expected_post_transaction_missing_tests": 67,
        "assignments": assignments,
        "unresolved": [],
    }
