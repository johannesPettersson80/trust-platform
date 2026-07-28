"""Plan the Phase 18 runtime-other exact behavior proof transaction."""

from __future__ import annotations

import json
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .exact_behavior_assignments import build_exact_assignment_plan
from .runtime_other_behavior_routes import BEHAVIOR_TEST_NAMES


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
    route_refs = set(BEHAVIOR_TEST_NAMES)
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
    return build_exact_assignment_plan(
        root=root,
        gap_report=gap_report,
        scan_report=scan_report,
        routes=BEHAVIOR_TEST_NAMES,
        expected_remaining=90,
    )


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


def _json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
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
