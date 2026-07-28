"""Plan the Phase 18 exact MQTT, OPC UA, and Modbus proof transaction."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .exact_behavior_assignments import build_exact_assignment_plan
from .frozen_catalog_gaps import generate_report
from .protocol_behavior_routes import BEHAVIOR_TEST_NAMES


SEMANTIC_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "protocol-29-semantic-lock.json"
)


def build_assignment_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Return the protocol assignment plan."""

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
        expected_remaining=38,
    )


def _assignment_plan_from_lock(path: Path) -> dict[str, Any]:
    lock = json.loads(path.read_text())
    if (
        not isinstance(lock, Mapping)
        or lock.get("schema_version") != 1
        or lock.get("id") != "PROTOCOL_29_SEMANTIC_LOCK"
    ):
        raise ValueError("protocol semantic lock identity drifted")
    summary = lock.get("summary")
    assignments_value = lock.get("assignments")
    if not isinstance(summary, Mapping):
        raise ValueError("protocol semantic lock lacks summary")
    if (
        not isinstance(assignments_value, Sequence)
        or isinstance(assignments_value, (str, bytes))
        or any(not isinstance(row, Mapping) for row in assignments_value)
    ):
        raise ValueError("protocol semantic lock assignments are invalid")
    assignments = [dict(row) for row in assignments_value]
    if (
        summary.get("behavior_demand_count") != 29
        or summary.get("campaign_test_count") != 32
        or summary.get("existing_catalog_test_count") != 29
        or summary.get("new_catalog_test_count") != 3
        or summary.get("confirmed_missing_test_count") != 38
        or len(assignments) != 32
    ):
        raise ValueError("protocol semantic lock population drifted")
    assignment_link_count = sum(
        len(refs)
        for row in assignments
        for refs in [row.get("behavior_refs")]
        if isinstance(refs, list)
    )
    if assignment_link_count != 35:
        raise ValueError("protocol semantic lock link count drifted")
    return {
        "demand_count": 29,
        "route_name_count": 32,
        "assigned_test_count": 32,
        "existing_catalog_test_count": 29,
        "new_catalog_test_count": 3,
        "assignment_link_count": 35,
        "new_denominator_dispositions": {"reviewed_nonmapping": 3},
        "new_tests_by_invariant": {"RT_OPCUA_SERVER_SURFACE_001": 3},
        "expected_post_transaction_missing_tests": 38,
        "assignments": assignments,
        "unresolved": [],
    }
