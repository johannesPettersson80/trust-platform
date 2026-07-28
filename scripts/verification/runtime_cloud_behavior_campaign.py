"""Plan and execute the Phase 18 runtime-cloud behavior campaign."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from .ads_behavior_proof_campaign import execute_campaign, _write_json
from .exact_behavior_campaign import build_exact_campaign_plan
from .runtime_cloud_behavior_proof import build_assignment_plan


DEFAULT_GAP_REPORT = Path(
    "target/gate-artifacts/verification/frozen-catalog-gaps/report.json"
)
DEFAULT_SCAN_REPORT = Path(
    "docs/internal/testing/evidence/plc-verification-program/2026-07-28/"
    "p18-runtime-other-exact-behavior-106/live-scan.json"
)
DEFAULT_PLAN_OUT = Path(
    "target/gate-artifacts/verification/p18-runtime-cloud-23/"
    "campaign-plan.json"
)
DEFAULT_RAW_DIR = Path(
    "target/gate-artifacts/verification/p18-runtime-cloud-23/raw"
)
DEFAULT_CAMPAIGN_OUT = Path(
    "target/gate-artifacts/verification/p18-runtime-cloud-23/campaign.json"
)
EXACT_FULL_NAME_OVERRIDES = {
    "TEST_RUNTIME_CLOUD_CONFIG_AGENT_RECONCILES_DESIRED_REPORTED_META_AND_STATUS_001": (
        "web_io_config_integration_part_08::"
        "runtime_cloud_config_agent_reconciles_desired_reported_meta_and_status"
    ),
    "TEST_RUNTIME_CLOUD_PREFLIGHT_ALLOWS_CROSS_SITE_CFG_APPLY_WITH_ALLOWLIST_001": (
        "web_io_config_integration_part_14::"
        "runtime_cloud_preflight_allows_cross_site_cfg_apply_with_allowlist"
    ),
    "TEST_RUNTIME_CLOUD_PREFLIGHT_DEV_ALLOWS_CROSS_SITE_CFG_APPLY_WITHOUT_ALLOWLIST_001": (
        "web_io_config_integration_part_14::"
        "runtime_cloud_preflight_dev_allows_cross_site_cfg_apply_without_allowlist"
    ),
}


def build_campaign_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Build exact grouped runner commands for all seven authorities."""

    assignment = build_assignment_plan(
        root=root,
        gap_report=gap_report,
        scan_report=scan_report,
    )
    return build_exact_campaign_plan(
        root=root,
        assignment=assignment,
        basis="p18_runtime_cloud_exact_behavior_assignment_23",
        group_prefix="RUNTIME_CLOUD_23",
        full_name_overrides=EXACT_FULL_NAME_OVERRIDES,
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
            f"planned {plan['test_count']} runtime-cloud tests in "
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
        f"runtime-cloud tests in {campaign['group_count']} exact groups"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
