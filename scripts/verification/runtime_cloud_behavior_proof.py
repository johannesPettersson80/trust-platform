"""Plan the Phase 18 runtime-cloud exact behavior proof transaction."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .exact_behavior_assignments import build_exact_assignment_plan
from .frozen_catalog_gaps import generate_report
from .runtime_cloud_behavior_routes import BEHAVIOR_TEST_NAMES

def build_assignment_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Resolve all seven runtime-cloud authorities without changing metadata."""

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
