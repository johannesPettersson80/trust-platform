"""Bounded execution for every registered fuzz target and smoke."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any

from .fuzz_campaign_contract import validate_campaign_payload
from .fuzz_program_contract import load_fuzz_program, validate_fuzz_program_contract
from .metadata_validator.constants import ROOT
from .metadata_validator.core import Validator


GENERATOR = "bounded-fuzz-campaign"
GENERATOR_VERSION = 1
DEFAULT_OUTPUT = Path("target/gate-artifacts/verification/fuzz-campaign.json")
DEFAULT_LOG_ROOT = Path("target/gate-artifacts/fuzz-campaign/logs")
EXECUTION_RE = re.compile(rb"#([0-9]+).*\bDONE\b")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--runs", type=int, default=10_000)
    parser.add_argument("--max-total-time-seconds", type=int, default=120)
    parser.add_argument("--timeout-seconds", type=int, default=10)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        output = _output_path(root, args.json_out)
        source_commit = _clean_head(root)
        program = load_fuzz_program(root)
        failures = validate_fuzz_program_contract(root, program)
        if failures:
            raise ValueError("; ".join(failures))
        validator = Validator()
        validator.load_records()
        validator.validate()
        if validator.failures:
            raise ValueError(
                "metadata validation failed: "
                + "; ".join(item.message for item in validator.failures)
            )
        for value, label in (
            (args.runs, "runs"),
            (args.max_total_time_seconds, "max-total-time-seconds"),
            (args.timeout_seconds, "timeout-seconds"),
        ):
            if value <= 0:
                raise ValueError(f"{label} must be positive")
    except (OSError, ValueError) as exc:
        print(f"fuzz campaign refused: {exc}", file=sys.stderr)
        return 2

    started_at = _now()
    results = []
    for target in program["targets"]:
        row = _run_target(
            root,
            target,
            runs=args.runs,
            max_total_time_seconds=args.max_total_time_seconds,
            timeout_seconds=args.timeout_seconds,
        )
        results.append(row)
        print(
            f"{target['id']}: exit={row['exit_status']} "
            f"executions={row['executions']} artifacts={len(row['artifact_files'])}",
            flush=True,
        )
    artifact_count = sum(len(row["artifact_files"]) for row in results)
    passed = sum(
        row["exit_status"] == 0
        and row["timed_out"] is False
        and not row["artifact_files"]
        for row in results
    )
    payload = {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "source_commit": source_commit,
        "started_at": started_at,
        "finished_at": _now(),
        "platform": f"{platform.system().lower()}-{platform.machine().lower()}",
        "requested_runs": args.runs,
        "max_total_time_seconds": args.max_total_time_seconds,
        "timeout_seconds": args.timeout_seconds,
        "results": results,
        "regressions": [],
        "summary": {
            "targets": len(results),
            "passed": passed,
            "infrastructure_failures": sum(
                row["exit_status"] != 0 and not row["artifact_files"] for row in results
            ),
            "crash_artifacts": artifact_count,
            "regressions": 0,
        },
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    failures = validate_campaign_payload(payload, program=program, tests=validator.tests)
    if failures:
        for failure in failures:
            print(f"fuzz campaign incomplete: {failure}", file=sys.stderr)
        return 1
    print(f"bounded fuzz campaign passed: {len(results)}/{len(results)} targets")
    return 0


def _run_target(
    root: Path,
    target: dict[str, Any],
    *,
    runs: int,
    max_total_time_seconds: int,
    timeout_seconds: int,
) -> dict[str, Any]:
    target_id = target["id"]
    log_path = root / DEFAULT_LOG_ROOT / f"{target_id}.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    if target["target_kind"] == "cargo_fuzz":
        command = [
            "cargo",
            "+nightly",
            "fuzz",
            "run",
            target["name"],
            "--",
            f"-runs={runs}",
            f"-max_total_time={max_total_time_seconds}",
            f"-timeout={timeout_seconds}",
            "-max_len=65536",
        ]
        cwd = root / shlex.split(target["command"])[1]
        process_timeout = max_total_time_seconds + 1_800
    else:
        command = shlex.split(target["command"])
        cwd = root
        process_timeout = 1_800
    timed_out = False
    try:
        with log_path.open("wb") as log:
            completed = subprocess.run(
                command,
                cwd=cwd,
                stdout=log,
                stderr=subprocess.STDOUT,
                env=os.environ.copy(),
                check=False,
                timeout=process_timeout,
            )
        exit_status = completed.returncode
    except subprocess.TimeoutExpired:
        timed_out = True
        exit_status = 124
    log_bytes = log_path.read_bytes()
    artifacts = _artifacts(root, target.get("artifact_path"))
    executions = 1
    if target["target_kind"] == "cargo_fuzz":
        matches = [int(value) for value in EXECUTION_RE.findall(log_bytes)]
        executions = max(matches, default=0)
    return {
        "target_id": target_id,
        "target_kind": target["target_kind"],
        "command": " ".join(command),
        "exit_status": exit_status,
        "timed_out": timed_out,
        "executions": executions,
        "log_sha256": "sha256:" + hashlib.sha256(log_bytes).hexdigest(),
        "artifact_files": artifacts,
    }


def _artifacts(root: Path, relative: object) -> list[dict[str, Any]]:
    if not isinstance(relative, str):
        return []
    directory = root / relative
    if not directory.exists():
        return []
    return [
        {
            "path": path.relative_to(root).as_posix(),
            "sha256": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest(),
            "size": path.stat().st_size,
        }
        for path in sorted(item for item in directory.rglob("*") if item.is_file())
    ]


def _clean_head(root: Path) -> str:
    commit = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    status = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
        check=True,
        capture_output=True,
    ).stdout
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or status:
        raise ValueError("source commit must identify a clean full Git SHA")
    return commit


def _output_path(root: Path, relative: Path) -> Path:
    raw = relative.as_posix()
    path = PurePosixPath(raw)
    if relative.is_absolute() or "\\" in raw or ".." in path.parts or "." in path.parts:
        raise ValueError("output path must be normalized and workspace-relative")
    candidate = root / path
    candidate.resolve(strict=False).relative_to(root)
    return candidate


def _now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")
