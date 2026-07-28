"""Plan and execute one exact Phase 18 runtime-other behavior campaign."""

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
from .runtime_other_behavior_proof import build_assignment_plan


DEFAULT_GAP_REPORT = Path(
    "target/gate-artifacts/verification/p18-ads-71/"
    "final-frozen-gaps.json"
)
DEFAULT_SCAN_REPORT = Path(
    "target/gate-artifacts/verification/p18-runtime-other-106/"
    "live-scan.json"
)
DEFAULT_PLAN_OUT = Path(
    "target/gate-artifacts/verification/p18-runtime-other-106/"
    "campaign-plan.json"
)
DEFAULT_RAW_DIR = Path(
    "target/gate-artifacts/verification/p18-runtime-other-106/raw"
)
DEFAULT_CAMPAIGN_OUT = Path(
    "target/gate-artifacts/verification/p18-runtime-other-106/"
    "campaign.json"
)
EXACT_FULL_NAME_OVERRIDES: Mapping[str, str] = {
    "DISC_0CBB1206EF27F98C58F6": (
        "run::tests::"
        "play_explicit_project_shape_classification_is_fail_closed"
    ),
    "DISC_5DFF0A06A903D91AC7DD": (
        "run::tests::"
        "play_project_resolution_distinguishes_absence_from_detection_failure"
    ),
    "DISC_F521BAD899EFD819471D": (
        "io::gpio::tests::"
        "gpio_safe_state_handoff_writes_and_confirms_backend_output"
    ),
    "DISC_FD2311D3995B059E552D": (
        "oscat_replaced_upstream_name_remains_absent"
    ),
    "TEST_AUTO_514A3C7CD34EA66EF422": (
        "hmi::tests::"
        "scaffold_output_is_deterministic_for_same_input"
    ),
    "TEST_EDITOR_RENAME_CROSS_FILE_CAPTURE_001": (
        "ide_features_part_02::"
        "test_rename_refuses_cross_file_reference_capture"
    ),
    "TEST_EDITOR_RENAME_SHADOW_CAPTURE_001": (
        "ide_features_part_02::"
        "test_rename_refuses_shadow_capture_with_named_reason"
    ),
    "TEST_AUTO_5B43AF2B1181A8401F05": (
        "openot_telemetry_authoring_sampling_policy_round_trips_definition"
    ),
    "TEST_AUTO_C105DE057DE08E3E5527": (
        "io_snapshot_emitted_after_cycle_output_commit"
    ),
    "TEST_AUTO_E1984AB7BA18E2EA3F4A": (
        "io::registry::tests::alias_resolves_to_canonical_driver_name"
    ),
}


def build_campaign_plan(
    *,
    root: Path,
    gap_report: Path,
    scan_report: Path,
) -> dict[str, Any]:
    """Build exact grouped runner commands for the frozen assignment."""

    assignment = build_assignment_plan(
        root=root,
        gap_report=gap_report,
        scan_report=scan_report,
    )
    return build_exact_campaign_plan(
        root=root,
        assignment=assignment,
        basis="p18_runtime_other_exact_behavior_assignment_106",
        group_prefix="RUNTIME_OTHER_106",
        full_name_overrides=EXACT_FULL_NAME_OVERRIDES,
        new_test_identity=_new_test_identity,
    )


def _new_test_identity(selected: Mapping[str, Any]) -> dict[str, Any]:
    discovery_id = str(selected["discovery_id"])
    path = str(selected["path"])
    name = str(selected["name"])
    if path == "crates/trust-runtime/tests/openot_telemetry.rs":
        target = ("trust-runtime", "test", "openot_telemetry")
    elif path == "crates/trust-runtime/tests/openot_capstone.rs":
        target = ("trust-runtime", "test", "openot_capstone")
    elif path == "crates/trust-dev/tests/cli_smoke.rs":
        target = ("trust-dev", "test", "cli_smoke")
    elif path == "crates/trust-runtime/src/io/gpio.rs":
        target = ("trust-runtime", "lib", None)
    elif path == (
        "crates/trust-runtime/src/bin/trust-runtime/run/tests.rs"
    ):
        target = ("trust-runtime", "bin", "trust-runtime")
    else:
        raise ValueError(
            f"{discovery_id} lacks a reviewed exact target identity"
        )
    return exact_identity(
        package=target[0],
        target_kind=target[1],
        target_name=target[2],
        full_name=EXACT_FULL_NAME_OVERRIDES.get(discovery_id, name),
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
            f"planned {plan['test_count']} runtime-other tests in "
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
        f"runtime-other tests in {campaign['group_count']} exact groups"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
