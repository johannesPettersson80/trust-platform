"""Cross-record integrity checks for verification metadata."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from .constants import RESOLUTION_STATUSES, ROOT

Fail = Callable[[Path, str], None]

OPEN_GAP_RESOLUTIONS = {"open", "decision_recorded", "spec_updated", "test_mapped"}
# Registered code-coverage backlog stays unresolved and reference-valid, but it
# must not area-block every planner request the way an actionable open gap does.
UNRESOLVED_GAP_RESOLUTIONS = OPEN_GAP_RESOLUTIONS | {"registered_backlog"}
SPEC_GAP_RESOLUTION_STATUSES = RESOLUTION_STATUSES | {"registered_backlog"}
RUNNABLE_TEST_STATUSES = {"mapped", "test_written", "implemented", "validated"}


def validate_open_spec_gap_references(
    *,
    fail: Fail,
    spec_gaps: dict[str, dict[str, Any]],
    required_specs: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    tests: dict[str, dict[str, Any]],
    risks: dict[str, dict[str, Any]],
) -> None:
    """Every open gap must remain visible to planning or traceability."""

    referenced = referenced_spec_gaps(required_specs, invariants, tests, risks)
    for gap in spec_gaps.values():
        if gap.get("status") != "spec_gap":
            continue
        if gap.get("resolution_status") not in UNRESOLVED_GAP_RESOLUTIONS:
            continue
        if gap["id"] not in referenced:
            fail(
                gap["_path"],
                f"open spec gap {gap['id']} is orphaned from required specs, invariants, coverage cells, behavior rows, tests, and risks",
            )


def referenced_spec_gaps(
    required_specs: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    tests: dict[str, dict[str, Any]],
    risks: dict[str, dict[str, Any]],
) -> set[str]:
    refs: set[str] = set()
    for record in required_specs.values():
        if record.get("spec_gap_ref"):
            refs.add(record["spec_gap_ref"])
    for record in invariants.values():
        refs.update(record.get("spec_gap_refs", []))
        for cell in record.get("coverage", {}).get("cells", []):
            if cell.get("spec_gap_ref"):
                refs.add(cell["spec_gap_ref"])
        for behavior in record.get("behavior", []):
            if behavior.get("spec_gap_ref"):
                refs.add(behavior["spec_gap_ref"])
    for record in tests.values():
        if record.get("spec_gap_ref"):
            refs.add(record["spec_gap_ref"])
    for record in risks.values():
        refs.update(record.get("related_spec_gaps", []))
    return refs


def test_counts_as_runnable(record: dict[str, Any]) -> bool:
    return record.get("status") in RUNNABLE_TEST_STATUSES


def validate_runnable_test_path(fail: Fail, path: Path, record: dict[str, Any]) -> None:
    if not test_counts_as_runnable(record):
        return
    test_path = ROOT / record.get("path", "")
    if not test_path.exists():
        fail(path, f"{record['id']} runnable test path does not exist: {record.get('path')}")
