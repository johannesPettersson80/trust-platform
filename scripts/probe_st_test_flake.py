#!/usr/bin/env python3
"""Probe ST test flake rate by repeating trust-dev test runs."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run repeated trust-dev test invocations and report flake sample stats."
    )
    parser.add_argument(
        "--test-bin",
        "--runtime-bin",
        dest="runtime_bin",
        required=True,
        help="Path to trust-dev executable; --runtime-bin is retained as a compatibility alias.",
    )
    parser.add_argument("--project", required=True, help="Project directory passed to --project")
    parser.add_argument("--runs", type=int, default=20, help="Number of repeated runs (default: 20)")
    parser.add_argument("--filter", default=None, help="Optional --filter value for trust-dev test")
    parser.add_argument(
        "--output-json", required=True, help="Path to output JSON report file"
    )
    parser.add_argument("--output-md", default=None, help="Optional markdown report file")
    parser.add_argument(
        "--max-failures",
        type=int,
        default=0,
        help="Fail probe if failures exceed this count (default: 0)",
    )
    return parser.parse_args()


def build_command(runtime_bin: str, project: str, filter_value: str | None) -> list[str]:
    command = [
        runtime_bin,
        "test",
        "--project",
        project,
        "--ci",
        "--output",
        "json",
    ]
    if filter_value:
        command.extend(["--filter", filter_value])
    return command


def parse_summary(stdout: str) -> dict[str, int] | None:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError:
        return None

    summary = payload.get("summary")
    if not isinstance(summary, dict):
        return None

    total = summary.get("total")
    passed = summary.get("passed")
    failed = summary.get("failed")
    errors = summary.get("errors")

    if not all(isinstance(value, int) for value in [total, passed, failed, errors]):
        return None

    return {
        "total": int(total),
        "passed": int(passed),
        "failed": int(failed),
        "errors": int(errors),
    }


def run_probe(command: list[str], runs: int) -> tuple[list[dict[str, Any]], int]:
    samples: list[dict[str, Any]] = []
    failures = 0

    for index in range(1, runs + 1):
        result = subprocess.run(command, capture_output=True, text=True)
        summary = parse_summary(result.stdout)

        is_failure = result.returncode != 0
        reason = ""

        if summary is None:
            is_failure = True
            reason = "invalid_json_output"
        elif summary["failed"] > 0 or summary["errors"] > 0:
            is_failure = True
            reason = "nonzero_failed_or_errors"

        if is_failure and not reason:
            reason = "nonzero_exit"

        if is_failure:
            failures += 1

        stderr_lines = [line.strip() for line in result.stderr.splitlines() if line.strip()]
        sample: dict[str, Any] = {
            "run": index,
            "status": "fail" if is_failure else "pass",
            "exit_code": int(result.returncode),
            "reason": reason or None,
            "stderr": stderr_lines[0] if stderr_lines else None,
            "summary": summary,
        }
        samples.append(sample)

    return samples, failures


def write_markdown(
    path: Path,
    payload: dict[str, Any],
) -> None:
    lines = [
        "# ST Test Flake Probe",
        "",
        f"- Generated: `{payload['generated_at']}`",
        f"- Project: `{payload['project']}`",
        f"- Command: `{' '.join(payload['command'])}`",
        f"- Runs: `{payload['runs']}`",
        f"- Passes: `{payload['passes']}`",
        f"- Failures: `{payload['failures']}`",
        f"- Flake rate: `{payload['flake_rate_percent']:.2f}%`",
        "",
        "## Samples",
        "| Run | Status | Exit | Reason |",
        "|---|---|---:|---|",
    ]

    for sample in payload["samples"]:
        lines.append(
            f"| {sample['run']} | {sample['status']} | {sample['exit_code']} | {sample.get('reason') or ''} |"
        )

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    if args.runs <= 0:
        print("--runs must be > 0", file=sys.stderr)
        return 2

    command = build_command(args.runtime_bin, args.project, args.filter)
    samples, failures = run_probe(command, args.runs)
    passes = args.runs - failures
    flake_rate_percent = round((failures / args.runs) * 100.0, 2)

    payload: dict[str, Any] = {
        "version": 1,
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "project": args.project,
        "command": command,
        "runs": args.runs,
        "passes": passes,
        "failures": failures,
        "flake_rate_percent": flake_rate_percent,
        "samples": samples,
    }

    output_json = Path(args.output_json)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if args.output_md:
        write_markdown(Path(args.output_md), payload)

    print(
        f"flake probe complete: runs={args.runs} passes={passes} failures={failures} "
        f"rate={flake_rate_percent:.2f}%"
    )

    if failures > args.max_failures:
        print(
            f"flake probe failed: failures {failures} exceeds max {args.max_failures}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
