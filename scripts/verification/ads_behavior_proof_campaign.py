"""Plan and execute the unified Phase 18 ADS exact-behavior campaign."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
import time
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .ads_behavior_proof_assignments import build_assignment_plan
from .coverage_trace_filter import exact_name_matcher


DEFAULT_GAP_REPORT = Path(
    "target/gate-artifacts/verification/p18-remaining-267/frozen-gaps.json"
)
DEFAULT_PLAN_OUT = Path(
    "target/gate-artifacts/verification/p18-ads-71/campaign-plan.json"
)
DEFAULT_RAW_DIR = Path(
    "target/gate-artifacts/verification/p18-ads-71/raw"
)
DEFAULT_CAMPAIGN_OUT = Path(
    "target/gate-artifacts/verification/p18-ads-71/campaign.json"
)
FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


def build_campaign_plan(*, root: Path, gap_report: Path) -> dict[str, Any]:
    """Build six exact nextest shards from the closed behavior assignment."""

    assignment = build_assignment_plan(
        root=root,
        gap_report=gap_report,
    )
    tests_by_id = {
        _string(test, "id", "assigned catalog test"): dict(test)
        for row in assignment["assignments"]
        for test in row["tests"]
    }
    grouped: dict[tuple[str, ...], list[dict[str, Any]]] = defaultdict(list)
    for test_id, test in sorted(tests_by_id.items()):
        command = _parse_catalog_command(test_id, test)
        grouped[command["cargo_prefix"]].append(
            {
                "test_id": test_id,
                "full_name": command["full_name"],
                "path": _string(test, "path", test_id),
                "package": command["package"],
                "target_kind": command["target_kind"],
                "target_name": command["target_name"],
                "feature_args": command["feature_args"],
                "binary_id": command["binary_id"],
                "encoded_test_name_prefix": command[
                    "encoded_test_name_prefix"
                ],
            }
        )

    groups = []
    seen_test_ids = set()
    for index, (cargo_prefix, selected) in enumerate(sorted(grouped.items())):
        selected.sort(key=lambda row: row["full_name"])
        identities = {
            (
                row["package"],
                row["target_kind"],
                row["target_name"],
                tuple(row["feature_args"]),
                row["binary_id"],
                row["encoded_test_name_prefix"],
            )
            for row in selected
        }
        if len(identities) != 1:
            raise ValueError("ADS campaign group target identity drifted")
        (
            package,
            target_kind,
            target_name,
            feature_args,
            binary_id,
            encoded_prefix,
        ) = next(iter(identities))
        full_names = [row["full_name"] for row in selected]
        if len(full_names) != len(set(full_names)):
            raise ValueError("ADS campaign group has duplicate exact test names")
        test_ids = [row["test_id"] for row in selected]
        duplicates = seen_test_ids.intersection(test_ids)
        if duplicates:
            raise ValueError(
                "ADS campaign assigns tests to multiple groups: "
                + ", ".join(sorted(duplicates))
            )
        seen_test_ids.update(test_ids)
        expression = " | ".join(
            f"(binary_id({exact_name_matcher(str(binary_id))}) & "
            f"test({exact_name_matcher(name)}))"
            for name in full_names
        )
        nextest_prefix = [
            "cargo",
            "nextest",
            "run",
            *cargo_prefix[2:],
        ]
        command = [
            *nextest_prefix,
            "--no-fail-fast",
            "--message-format",
            "libtest-json-plus",
            "--message-format-version",
            "0.1",
            "--user-config-file",
            "none",
            "-E",
            expression,
        ]
        groups.append(
            {
                "id": f"ADS71_{index + 1:02d}",
                "package": package,
                "target_kind": target_kind,
                "target_name": target_name,
                "feature_args": list(feature_args),
                "binary_id": binary_id,
                "encoded_test_name_prefix": encoded_prefix,
                "test_count": len(selected),
                "test_ids": test_ids,
                "full_names": full_names,
                "tests_by_full_name": {
                    row["full_name"]: row["test_id"] for row in selected
                },
                "paths": sorted({row["path"] for row in selected}),
                "command": command,
            }
        )

    if seen_test_ids != set(tests_by_id):
        raise ValueError("ADS campaign omitted assigned tests")
    return {
        "schema_version": 1,
        "basis": "p18_ads_exact_behavior_assignment_71",
        "behavior_count": assignment["assigned_behavior_count"],
        "test_count": len(tests_by_id),
        "path_count": assignment["assigned_path_count"],
        "group_count": len(groups),
        "assignment_gap_summary": assignment[
            "post_assignment_gap_summary"
        ],
        "groups": groups,
    }


def normalize_group_results(
    *,
    group: Mapping[str, Any],
    raw: str,
) -> list[dict[str, Any]]:
    """Normalize one exact nextest JSONL shard and reject incomplete proof."""

    prefix = _string(
        group,
        "encoded_test_name_prefix",
        "campaign group",
    )
    tests_by_name = group.get("tests_by_full_name")
    if not isinstance(tests_by_name, Mapping) or not tests_by_name:
        raise ValueError("campaign group lacks exact test identities")
    expected = {
        str(name): str(test_id)
        for name, test_id in tests_by_name.items()
    }
    terminal: dict[str, dict[str, Any]] = {}
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(
                f"raw result line {line_number} is not JSON: {error.msg}"
            ) from error
        if not isinstance(event, Mapping) or event.get("type") != "test":
            continue
        event_name = event.get("event")
        if event_name in {"started", "ignored"}:
            continue
        raw_name = event.get("name")
        if not isinstance(raw_name, str) or not raw_name.startswith(prefix):
            raise ValueError(
                f"terminal result has unexpected encoded name {raw_name!r}"
            )
        full_name = raw_name.removeprefix(prefix)
        if full_name not in expected:
            raise ValueError(f"unexpected terminal test {full_name}")
        if full_name in terminal:
            raise ValueError(f"duplicate terminal test {full_name}")
        if event_name != "ok":
            raise ValueError(
                f"selected test {full_name} did not pass: {event_name!r}"
            )
        duration = event.get("exec_time", 0)
        if (
            not isinstance(duration, (int, float))
            or isinstance(duration, bool)
            or duration < 0
        ):
            raise ValueError(f"selected test {full_name} has invalid duration")
        terminal[full_name] = {
            "test_id": expected[full_name],
            "full_name": full_name,
            "status": "passed",
            "duration_ms": int(round(float(duration) * 1000)),
        }
    missing = sorted(set(expected) - set(terminal))
    if missing:
        raise ValueError(
            "missing terminal results: " + ", ".join(missing)
        )
    return [terminal[name] for name in sorted(terminal)]


def execute_campaign(
    *,
    root: Path,
    plan: Mapping[str, Any],
    raw_dir: Path,
    source_commit: str,
) -> dict[str, Any]:
    """Execute all exact shards, retaining a complete pass/failure ledger."""

    if not FULL_COMMIT_RE.fullmatch(source_commit):
        raise ValueError("ADS campaign source commit must be full SHA")
    actual_commit = _git(root, "rev-parse", "HEAD")
    if actual_commit != source_commit:
        raise ValueError(
            f"ADS campaign checkout is {actual_commit}, expected {source_commit}"
        )
    if _git(root, "status", "--porcelain", "--untracked-files=all"):
        raise ValueError("ADS campaign requires a clean checkout")

    raw_dir.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["NEXTEST_EXPERIMENTAL_LIBTEST_JSON"] = "1"
    groups = _records(plan.get("groups"), "campaign groups")
    group_results = []
    all_results = []
    failures = []
    for group in groups:
        group_id = _string(group, "id", "campaign group")
        command = _strings(group.get("command"), f"{group_id} command")
        started = time.monotonic()
        completed = subprocess.run(
            command,
            cwd=root,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        duration_ms = int(round((time.monotonic() - started) * 1000))
        raw_path = raw_dir / f"{group_id}.jsonl"
        stderr_path = raw_dir / f"{group_id}.stderr.txt"
        raw_path.write_text(completed.stdout)
        stderr_path.write_text(completed.stderr)
        error = None
        normalized = []
        try:
            normalized = normalize_group_results(
                group=group,
                raw=completed.stdout,
            )
            if completed.returncode != 0:
                error = (
                    f"nextest exited {completed.returncode} despite complete "
                    "passing terminals"
                )
        except ValueError as exc:
            error = str(exc)
        if error is not None:
            failures.append(f"{group_id}: {error}")
        else:
            all_results.extend(normalized)
        group_results.append(
            {
                "id": group_id,
                "command": command,
                "exit_status": completed.returncode,
                "duration_ms": duration_ms,
                "expected_test_count": group.get("test_count"),
                "passed_test_count": len(normalized),
                "raw_path": raw_path.relative_to(root).as_posix(),
                "raw_sha256": _digest(raw_path),
                "stderr_path": stderr_path.relative_to(root).as_posix(),
                "stderr_sha256": _digest(stderr_path),
                "status": "passed" if error is None else "failed",
                "error": error,
            }
        )

    all_results.sort(key=lambda row: row["test_id"])
    expected_count = plan.get("test_count")
    if not failures and len(all_results) != expected_count:
        failures.append(
            f"campaign normalized {len(all_results)} tests, "
            f"expected {expected_count}"
        )
    return {
        "schema_version": 1,
        "basis": plan.get("basis"),
        "source_commit": source_commit,
        "hostname": _hostname(),
        "checkout": root.as_posix(),
        "group_count": len(groups),
        "test_count": expected_count,
        "passed_count": len(all_results),
        "all_passed": not failures,
        "failures": failures,
        "groups": group_results,
        "results": all_results,
    }


def _parse_catalog_command(
    test_id: str,
    test: Mapping[str, Any],
) -> dict[str, Any]:
    tokens = shlex.split(_string(test, "command", test_id))
    if len(tokens) < 8 or tokens[:2] != ["cargo", "test"]:
        raise ValueError(f"{test_id} lacks a supported cargo test command")
    try:
        separator = tokens.index("--")
        package_index = tokens.index("-p")
    except ValueError as error:
        raise ValueError(f"{test_id} lacks exact cargo command fields") from error
    if tokens[separator + 1 : separator + 2] != ["--exact"]:
        raise ValueError(f"{test_id} command is not exact")
    full_name = tokens[separator - 1]
    cargo_prefix = tuple(tokens[: separator - 1])
    package = tokens[package_index + 1]
    if "--lib" in cargo_prefix:
        target_kind = "lib"
        target_name = None
        binary_id = package
        encoded_prefix = f"{package}::{package.replace('-', '_')}$"
        target_tokens = {"--lib"}
    elif "--test" in cargo_prefix:
        target_kind = "test"
        target_index = cargo_prefix.index("--test")
        target_name = cargo_prefix[target_index + 1]
        binary_id = f"{package}::{target_name}"
        encoded_prefix = f"{binary_id}$"
        target_tokens = {"--test", target_name}
    else:
        raise ValueError(f"{test_id} has unsupported Cargo target")
    feature_args = []
    index = 2
    while index < len(cargo_prefix):
        token = cargo_prefix[index]
        if token == "-p":
            index += 2
        elif token in target_tokens:
            index += 2 if token == "--test" else 1
        elif token == "--features":
            feature_args.extend((token, cargo_prefix[index + 1]))
            index += 2
        elif token == "--no-default-features":
            feature_args.append(token)
            index += 1
        else:
            raise ValueError(
                f"{test_id} has unsupported Cargo option {token!r}"
            )
    return {
        "cargo_prefix": cargo_prefix,
        "package": package,
        "target_kind": target_kind,
        "target_name": target_name,
        "feature_args": feature_args,
        "binary_id": binary_id,
        "encoded_test_name_prefix": encoded_prefix,
        "full_name": full_name,
    }


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=root,
        text=True,
        capture_output=True,
        check=True,
    )
    return completed.stdout.strip()


def _hostname() -> str:
    completed = subprocess.run(
        ["hostname"],
        text=True,
        capture_output=True,
        check=True,
    )
    return completed.stdout.strip()


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def _records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a list")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} contains a non-object")
    return list(value)


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string list")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} contains an invalid string")
    return list(value)


def _string(row: Mapping[str, Any], key: str, label: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {key}")
    return value


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--gap-report", type=Path, default=DEFAULT_GAP_REPORT)
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
    gap_report = (
        args.gap_report
        if args.gap_report.is_absolute()
        else root / args.gap_report
    )
    plan = build_campaign_plan(root=root, gap_report=gap_report)
    plan_out = args.plan_out if args.plan_out.is_absolute() else root / args.plan_out
    _write_json(plan_out, plan)
    if not args.execute:
        print(
            f"planned {plan['test_count']} ADS tests in "
            f"{plan['group_count']} exact groups"
        )
        return 0
    if not args.source_commit:
        parser.error("--source-commit is required with --execute")
    raw_dir = args.raw_dir if args.raw_dir.is_absolute() else root / args.raw_dir
    campaign = execute_campaign(
        root=root,
        plan=plan,
        raw_dir=raw_dir,
        source_commit=args.source_commit,
    )
    campaign_out = (
        args.campaign_out
        if args.campaign_out.is_absolute()
        else root / args.campaign_out
    )
    _write_json(campaign_out, campaign)
    if not campaign["all_passed"]:
        for failure in campaign["failures"]:
            print(failure, file=sys.stderr)
        return 1
    print(
        f"passed {campaign['passed_count']}/{campaign['test_count']} "
        f"ADS tests in {campaign['group_count']} exact groups"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
