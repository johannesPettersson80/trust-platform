"""Plan and execute one exact Phase 18 runtime-other behavior campaign."""

from __future__ import annotations

import argparse
import re
import shlex
import sys
import tomllib
from collections import defaultdict
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .coverage_trace_filter import exact_name_matcher
from .ads_behavior_proof_campaign import execute_campaign, _write_json
from .runtime_other_behavior_proof import build_assignment_plan


CATALOG_PATH = Path("verification/test-catalog.toml")
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
    catalog = tomllib.loads((root / CATALOG_PATH).read_text())
    catalog_tests = catalog.get("tests")
    if not isinstance(catalog_tests, list):
        raise ValueError("test catalog lacks test records")
    catalog_by_id = {
        str(test["id"]): test
        for test in catalog_tests
        if isinstance(test, Mapping) and isinstance(test.get("id"), str)
    }

    grouped: dict[
        tuple[str, str, str | None],
        list[dict[str, Any]],
    ] = defaultdict(list)
    for selected in assignment["assignments"]:
        test_id = (
            str(selected["catalog_test_id"])
            if selected["catalog_test_id"] is not None
            else str(selected["discovery_id"])
        )
        if selected["catalog_test_id"] is None:
            identity = _new_test_identity(selected)
        else:
            catalog_test = catalog_by_id.get(str(selected["catalog_test_id"]))
            if catalog_test is None:
                raise ValueError(f"catalog lacks {selected['catalog_test_id']}")
            identity = _catalog_test_identity(test_id, catalog_test)
        grouped[
            (
                identity["package"],
                identity["target_kind"],
                identity["target_name"],
            )
        ].append(
            {
                "test_id": test_id,
                "discovery_id": selected["discovery_id"],
                "path": selected["path"],
                **identity,
            }
        )

    groups = []
    seen_test_ids: set[str] = set()
    for index, (target, selected) in enumerate(sorted(grouped.items())):
        package, target_kind, target_name = target
        selected.sort(key=lambda row: row["full_name"])
        full_names = [str(row["full_name"]) for row in selected]
        if len(full_names) != len(set(full_names)):
            raise ValueError(
                f"{target} contains duplicate exact test names"
            )
        test_ids = [str(row["test_id"]) for row in selected]
        overlap = seen_test_ids.intersection(test_ids)
        if overlap:
            raise ValueError(
                "campaign assigns tests to multiple targets: "
                + ", ".join(sorted(overlap))
            )
        seen_test_ids.update(test_ids)
        binary_ids = {str(row["binary_id"]) for row in selected}
        prefixes = {str(row["encoded_test_name_prefix"]) for row in selected}
        if len(binary_ids) != 1 or len(prefixes) != 1:
            raise ValueError(f"{target} binary identity drifted")
        binary_id = next(iter(binary_ids))
        encoded_prefix = next(iter(prefixes))
        exact_terms = [
            f"(binary_id({exact_name_matcher(binary_id)}) & "
            f"test({exact_name_matcher(name)}))"
            for name in full_names
        ]
        target_args = {
            "lib": ["--lib"],
            "bin": ["--bin", str(target_name)],
            "test": ["--test", str(target_name)],
        }.get(target_kind)
        if target_args is None:
            raise ValueError(f"unsupported target kind {target_kind}")
        command = [
            "cargo",
            "nextest",
            "run",
            "-p",
            package,
            *target_args,
            "--no-fail-fast",
            "--message-format",
            "libtest-json-plus",
            "--message-format-version",
            "0.1",
            "--user-config-file",
            "none",
            "-E",
            " | ".join(exact_terms),
        ]
        groups.append(
            {
                "id": f"RUNTIME_OTHER_106_{index + 1:02d}",
                "package": package,
                "target_kind": target_kind,
                "target_name": target_name,
                "binary_id": binary_id,
                "encoded_test_name_prefix": encoded_prefix,
                "test_count": len(selected),
                "test_ids": test_ids,
                "full_names": full_names,
                "tests_by_full_name": {
                    str(row["full_name"]): str(row["test_id"])
                    for row in selected
                },
                "paths": sorted({str(row["path"]) for row in selected}),
                "exact_terms": exact_terms,
                "command": command,
            }
        )
    if len(seen_test_ids) != assignment["assigned_test_count"]:
        raise ValueError("campaign omitted assigned test identities")
    return {
        "schema_version": 1,
        "basis": "p18_runtime_other_exact_behavior_assignment_106",
        "behavior_count": assignment["demand_count"],
        "test_count": assignment["assigned_test_count"],
        "group_count": len(groups),
        "groups": groups,
    }


def _catalog_test_identity(
    test_id: str,
    test: Mapping[str, Any],
) -> dict[str, Any]:
    command = test.get("command")
    if not isinstance(command, str) or not command:
        raise ValueError(f"{test_id} lacks a command")
    tokens = shlex.split(command)
    if tokens[:2] == ["cargo", "test"]:
        return _cargo_test_identity(test_id, tokens)
    if tokens[:3] == ["cargo", "nextest", "run"]:
        return _nextest_identity(test_id, tokens)
    raise ValueError(f"{test_id} has unsupported command {command!r}")


def _cargo_test_identity(
    test_id: str,
    tokens: list[str],
) -> dict[str, Any]:
    try:
        package = tokens[tokens.index("-p") + 1]
        separator = tokens.index("--")
    except (ValueError, IndexError) as error:
        raise ValueError(f"{test_id} lacks exact cargo test fields") from error
    if tokens[separator + 1 : separator + 2] != ["--exact"]:
        raise ValueError(f"{test_id} cargo command is not exact")
    if separator < 1:
        raise ValueError(f"{test_id} cargo command lacks a test name")
    full_name = tokens[separator - 1]
    target_kind, target_name = _target(tokens[:separator], package)
    return _identity(
        package=package,
        target_kind=target_kind,
        target_name=target_name,
        full_name=EXACT_FULL_NAME_OVERRIDES.get(test_id, full_name),
    )


def _nextest_identity(
    test_id: str,
    tokens: list[str],
) -> dict[str, Any]:
    try:
        package = tokens[tokens.index("-p") + 1]
        expression = tokens[tokens.index("-E") + 1]
    except (ValueError, IndexError) as error:
        raise ValueError(f"{test_id} lacks exact nextest fields") from error
    target_kind, target_name = _target(tokens, package)
    binary_match = re.search(r"binary_id\(=([^)]*)\)", expression)
    test_match = re.search(r"test\(([=~])([^)]*)\)", expression)
    if binary_match is None or test_match is None:
        raise ValueError(f"{test_id} nextest expression is unsupported")
    identity = _identity(
        package=package,
        target_kind=target_kind,
        target_name=target_name,
        full_name=EXACT_FULL_NAME_OVERRIDES.get(
            test_id,
            test_match.group(2),
        ),
    )
    if identity["binary_id"] != binary_match.group(1):
        raise ValueError(f"{test_id} nextest binary identity drifted")
    return identity


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
    return _identity(
        package=target[0],
        target_kind=target[1],
        target_name=target[2],
        full_name=EXACT_FULL_NAME_OVERRIDES.get(discovery_id, name),
    )


def _target(
    tokens: list[str],
    package: str,
) -> tuple[str, str | None]:
    if "--test" in tokens:
        index = tokens.index("--test")
        return "test", tokens[index + 1]
    if "--bin" in tokens:
        index = tokens.index("--bin")
        return "bin", tokens[index + 1]
    if "--lib" in tokens:
        return "lib", None
    return "lib", None


def _identity(
    *,
    package: str,
    target_kind: str,
    target_name: str | None,
    full_name: str,
) -> dict[str, Any]:
    if target_kind == "lib":
        binary_id = package
        encoded_prefix = f"{package}::{package.replace('-', '_')}$"
    else:
        if not target_name:
            raise ValueError(f"{package} {target_kind} target lacks a name")
        binary_id = (
            f"{package}::bin/{target_name}"
            if target_kind == "bin"
            else f"{package}::{target_name}"
        )
        encoded_prefix = (
            f"{package}::{target_name}$"
            if target_kind == "bin"
            else f"{binary_id}$"
        )
    return {
        "package": package,
        "target_kind": target_kind,
        "target_name": target_name,
        "binary_id": binary_id,
        "encoded_test_name_prefix": encoded_prefix,
        "full_name": full_name,
    }


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
