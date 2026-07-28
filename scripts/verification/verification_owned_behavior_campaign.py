"""Plan and execute the exact Phase 18 verification-owned campaign."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .ads_behavior_proof_campaign import execute_campaign, _write_json
from .exact_behavior_campaign import (
    build_exact_campaign_plan,
    exact_identity,
)
from .verification_owned_behavior_proof import build_assignment_plan


ARTIFACT_ROOT = Path(
    "target/gate-artifacts/verification/p18-verification-owned-11"
)
DEFAULT_GAP_REPORT = ARTIFACT_ROOT / "pre-mapping-gap-report.json"
DEFAULT_SCAN_REPORT = ARTIFACT_ROOT / "live-scan.json"
DEFAULT_PLAN_OUT = ARTIFACT_ROOT / "campaign-plan.json"
DEFAULT_RAW_DIR = ARTIFACT_ROOT / "raw"
DEFAULT_CAMPAIGN_OUT = ARTIFACT_ROOT / "campaign.json"


def build_campaign_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Build the two exact verification-owned nextest groups."""

    assignment = build_assignment_plan(
        root=root,
        gap_report=gap_report,
        scan_report=scan_report,
    )
    return build_exact_campaign_plan(
        root=root,
        assignment=assignment,
        basis="p18_verification_owned_exact_behavior_assignment_11",
        group_prefix="VERIFICATION_OWNED_11",
        new_test_identity=_new_test_identity,
    )


def _new_test_identity(selected: Mapping[str, Any]) -> dict[str, Any]:
    path = str(selected["path"])
    name = str(selected["name"])
    if path == "crates/verification-cases/src/lib.rs":
        return exact_identity(
            package="verification-cases",
            target_kind="lib",
            target_name=None,
            full_name=f"tests::{name}",
        )
    if path == "crates/trust-hir/tests/iec_table_coverage.rs":
        return exact_identity(
            package="trust-hir",
            target_kind="test",
            target_name="iec_table_coverage",
            full_name=name,
        )
    raise ValueError(f"{selected['discovery_id']} lacks an exact target")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--gap-report", type=Path, default=DEFAULT_GAP_REPORT)
    parser.add_argument("--scan-report", type=Path, default=DEFAULT_SCAN_REPORT)
    parser.add_argument("--plan-out", type=Path, default=DEFAULT_PLAN_OUT)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--raw-dir", type=Path, default=DEFAULT_RAW_DIR)
    parser.add_argument(
        "--campaign-out",
        type=Path,
        default=DEFAULT_CAMPAIGN_OUT,
    )
    parser.add_argument("--source-commit")
    args = parser.parse_args(argv)
    root = args.root.resolve()

    def rooted(path: Path) -> Path:
        return path if path.is_absolute() else root / path

    plan = build_campaign_plan(
        root=root,
        gap_report=rooted(args.gap_report),
        scan_report=rooted(args.scan_report),
    )
    _write_json(rooted(args.plan_out), plan)
    if not args.execute:
        print(
            f"planned {plan['test_count']} verification-owned tests in "
            f"{plan['group_count']} exact groups"
        )
        return 0
    if not args.source_commit:
        parser.error("--source-commit is required with --execute")
    campaign = execute_campaign(
        root=root,
        plan=plan,
        raw_dir=rooted(args.raw_dir),
        source_commit=args.source_commit,
    )
    _write_json(rooted(args.campaign_out), campaign)
    if not campaign["all_passed"]:
        for failure in campaign["failures"]:
            print(failure, file=sys.stderr)
        return 1
    print(
        f"passed {campaign['passed_count']}/{campaign['test_count']} "
        f"verification-owned tests in {campaign['group_count']} groups"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
