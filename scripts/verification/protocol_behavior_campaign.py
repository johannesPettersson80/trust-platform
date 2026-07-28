"""Plan and execute the exact Phase 18 protocol behavior campaign."""

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
from .protocol_behavior_proof import build_assignment_plan


DEFAULT_GAP_REPORT = Path(
    "target/gate-artifacts/verification/p18-runtime-cloud-23/"
    "post-mapping-gap-report.json"
)
DEFAULT_SCAN_REPORT = Path(
    "docs/internal/testing/evidence/plc-verification-program/2026-07-28/"
    "p18-runtime-other-exact-behavior-106/live-scan.json"
)
DEFAULT_PLAN_OUT = Path(
    "target/gate-artifacts/verification/p18-protocol-29/"
    "campaign-plan.json"
)
DEFAULT_RAW_DIR = Path(
    "target/gate-artifacts/verification/p18-protocol-29/raw"
)
DEFAULT_CAMPAIGN_OUT = Path(
    "target/gate-artifacts/verification/p18-protocol-29/campaign.json"
)


def build_campaign_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Build the exact grouped protocol campaign."""

    assignment = build_assignment_plan(
        root=root,
        gap_report=gap_report,
        scan_report=scan_report,
    )
    return build_exact_campaign_plan(
        root=root,
        assignment=assignment,
        basis="p18_protocol_exact_behavior_assignment_29",
        group_prefix="PROTOCOL_29",
        new_test_identity=_new_test_identity,
    )


def _new_test_identity(selected: Mapping[str, Any]) -> dict[str, Any]:
    path = str(selected["path"])
    if path != "crates/trust-runtime/tests/opcua_integration.rs":
        raise ValueError(f"{selected['discovery_id']} lacks an exact target")
    return exact_identity(
        package="trust-runtime",
        target_kind="test",
        target_name="opcua_integration",
        full_name=str(selected["name"]),
    )


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
            f"planned {plan['test_count']} protocol tests in "
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
        f"protocol tests in {campaign['group_count']} exact groups"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
