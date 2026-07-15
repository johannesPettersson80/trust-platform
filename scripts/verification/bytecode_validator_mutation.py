"""Run a focused bytecode-validator mutation shard in an isolated Git archive."""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import platform
import subprocess
import tempfile
import time
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterator, Sequence

from .metadata_validator.constants import ROOT
from .metadata_validator.mutation_shards import (
    MutationContract,
    MutationContractError,
    MutationSpec,
    has_infrastructure_failure,
    load_mutation_contract,
    validate_mutation_report,
)


RUNNER_VERSION = "bytecode-validator-mutation.py v1"
TOOL_NAME = "cargo-mutants-single-file-adapter"
DEFAULT_TEST_ID = "TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"


@dataclass(frozen=True)
class CommandResult:
    command: tuple[str, ...]
    returncode: int | None
    stdout: str
    stderr: str
    timed_out: bool
    duration_seconds: float


def select_generated_mutant(candidates: list[dict[str, Any]], config: dict[str, Any]) -> dict[str, Any]:
    matches = [
        candidate
        for candidate in candidates
        if candidate.get("function", {}).get("function_name") == config.get("function")
        and candidate.get("genre") == config.get("genre")
        and candidate.get("replacement") == config.get("replacement")
    ]
    if len(matches) != 1:
        raise MutationContractError(
            f"mutation selector for {config.get('function')} found {len(matches)} generated mutants"
        )
    return matches[0]


def apply_generated_mutant(source: str, candidate: dict[str, Any]) -> str:
    span = candidate.get("span", {})
    start = source_offset(source, span.get("start", {}))
    end = source_offset(source, span.get("end", {}))
    if start > end:
        raise MutationContractError("generated mutant span is reversed")
    replacement = candidate.get("replacement")
    if not isinstance(replacement, str):
        raise MutationContractError("generated mutant replacement is not text")
    return source[:start] + replacement + source[end:]


def source_offset(source: str, position: dict[str, Any]) -> int:
    line = position.get("line")
    column = position.get("column")
    if not isinstance(line, int) or not isinstance(column, int) or line < 1 or column < 1:
        raise MutationContractError(f"invalid one-based source position {position!r}")
    lines = source.splitlines(keepends=True)
    if line > len(lines):
        raise MutationContractError(f"source line {line} is out of range")
    line_text = lines[line - 1]
    if column - 1 > len(line_text):
        raise MutationContractError(f"source column {column} is out of range on line {line}")
    return sum(len(item) for item in lines[: line - 1]) + column - 1


def classify_mutant(build: CommandResult, test: CommandResult | None) -> str:
    if command_has_infrastructure_failure(build) or (
        test is not None and command_has_infrastructure_failure(test)
    ):
        return "error"
    if build.timed_out or (test is not None and test.timed_out):
        return "timeout"
    if build.returncode != 0:
        return "unviable"
    if test is None:
        return "error"
    if test.returncode == 0:
        return "survived"
    return "caught"


def build_report(
    *,
    contract: MutationContract,
    outcomes: list[dict[str, Any]],
    source_commit: str,
    tool_version: str,
    platform: str,
    started_at: str,
    finished_at: str,
    baseline_commands: list[dict[str, Any]],
) -> dict[str, Any]:
    results = {result: 0 for result in ("caught", "survived", "unviable", "timeout", "error")}
    survivors: list[dict[str, Any]] = []
    for outcome in outcomes:
        results[outcome["result"]] += 1
        if outcome["result"] == "survived":
            survivors.append(
                {
                    "id": outcome["id"],
                    "related_case_ids": list(outcome["related_case_ids"]),
                    "action": outcome["survivor_action"],
                }
            )
    return {
        "schema_version": 1,
        "id": "MUTATION_REPORT_BYTECODE_VALIDATOR_20260709",
        "status": "complete",
        "shard_id": contract.shard_id,
        "test_id": contract.test_id,
        "runner": RUNNER_VERSION,
        "tool": TOOL_NAME,
        "tool_version": tool_version,
        "source_commit": source_commit,
        "platform": platform,
        "started_at": started_at,
        "finished_at": finished_at,
        "case_file": contract.case_file,
        "case_file_digest": contract.case_file_digest,
        "case_semantics": contract.case_semantics,
        "blocked_case_ids_executed": False,
        "baseline_commands": baseline_commands,
        "mutations": outcomes,
        "summary": {"total": len(contract.mutations), **results},
        "survivors": survivors,
        "out_of_scope_case_ids": list(contract.out_of_scope_case_ids),
        "out_of_scope_reason": contract.out_of_scope_reason,
    }


def render_markdown(report: dict[str, Any]) -> str:
    summary = report["summary"]
    lines = [
        "# Bytecode Validator Mutation Shard Report",
        "",
        f"Source commit: `{report['source_commit']}`",
        f"Runner: `{report['runner']}`",
        f"Tool: `{report['tool_version']}`",
        f"Platform: `{report['platform']}`",
        "",
        "The associated case IDs were not executed by the mutation runner. They identify the",
        "committed validator risks exercised by each source mutant; this report is test-adequacy",
        "evidence only and does not claim case execution or behavior proof.",
        "",
        "## Summary",
        "",
        f"- Total mutants: {summary['total']}",
        f"- Caught: {summary['caught']}",
        f"- Survivors: {summary['survived']}",
        f"- Unviable: {summary['unviable']}",
        f"- Timeouts: {summary['timeout']}",
        f"- Errors: {summary['error']}",
        "",
        "## Outcomes",
        "",
        "| Mutant | Result | Related committed case IDs | Action if survivor |",
        "| --- | --- | --- | --- |",
    ]
    for outcome in report["mutations"]:
        case_ids = "<br>".join(f"`{case_id}`" for case_id in outcome["related_case_ids"])
        lines.append(
            f"| `{outcome['id']}` | `{outcome['result']}` | {case_ids} | {outcome['survivor_action']} |"
        )
    lines.extend(["", "## Out Of Scope", ""])
    for case_id in report["out_of_scope_case_ids"]:
        lines.append(f"- `{case_id}`")
    lines.extend(["", report["out_of_scope_reason"], ""])
    return "\n".join(lines)


def execute_shard(
    *,
    contract: MutationContract,
    root: Path,
    target_dir: Path | None,
    build_timeout: float,
    test_timeout: float,
) -> dict[str, Any]:
    started_at = timestamp()
    source_commit = git_output(root, ["rev-parse", "HEAD"])
    ensure_validator_sources_match_head(root, contract)
    tool_version = command_output(root, ["cargo", "mutants", "--version"])
    if RUNNER_VERSION != contract.runner or TOOL_NAME != contract.tool or tool_version != contract.tool_version:
        raise MutationContractError("installed mutation runner/tool does not match the catalog contract")
    environment = os.environ.copy()
    if target_dir is not None:
        target_dir.mkdir(parents=True, exist_ok=True)
        environment["CARGO_TARGET_DIR"] = str(target_dir.resolve())

    with archived_head(root) as scratch:
        clean_mutation_target(scratch, environment, build_timeout)
        baseline_commands: list[dict[str, Any]] = []
        seen_commands: set[tuple[str, ...]] = set()
        for mutation in contract.mutations:
            if mutation.test_command in seen_commands:
                continue
            seen_commands.add(mutation.test_command)
            baseline = run_command(mutation.test_command, cwd=scratch, env=environment, timeout=test_timeout)
            baseline_commands.append(command_summary(baseline))
            if baseline.timed_out or baseline.returncode != 0:
                raise MutationContractError(
                    f"baseline command failed for {mutation.id}: {list(mutation.test_command)}"
                )

        generated_by_file: dict[str, list[dict[str, Any]]] = {}
        outcomes: list[dict[str, Any]] = []
        for mutation in contract.mutations:
            clean_mutation_target(scratch, environment, build_timeout)
            source_path = scratch / mutation.source_file
            original = source_path.read_text()
            candidates = generated_by_file.get(mutation.source_file)
            if candidates is None:
                candidates = discover_mutants(source_path, scratch, environment)
                generated_by_file[mutation.source_file] = candidates
            candidate = select_generated_mutant(candidates, mutation.__dict__)
            source_path.write_text(apply_generated_mutant(original, candidate))
            try:
                build = run_command(mutation.build_command, cwd=scratch, env=environment, timeout=build_timeout)
                test = None
                if not build.timed_out and build.returncode == 0:
                    test = run_command(mutation.test_command, cwd=scratch, env=environment, timeout=test_timeout)
                result = classify_mutant(build, test)
                outcomes.append(outcome_record(mutation, candidate, build, test, result))
                if result == "error":
                    raise MutationContractError(f"infrastructure failure while executing {mutation.id}")
            finally:
                source_path.write_text(original)
                clean_mutation_target(scratch, environment, build_timeout)

    report = build_report(
        contract=contract,
        outcomes=outcomes,
        source_commit=source_commit,
        tool_version=tool_version,
        platform=f"{platform.system().lower()}-{platform.machine()}",
        started_at=started_at,
        finished_at=timestamp(),
        baseline_commands=baseline_commands,
    )
    failures = validate_mutation_report(report, contract)
    if failures:
        raise MutationContractError("; ".join(failures))
    return report


def discover_mutants(source_path: Path, cwd: Path, env: dict[str, str]) -> list[dict[str, Any]]:
    result = run_command(
        ("cargo", "mutants", "--Zmutate-file", str(source_path), "--list", "--json"),
        cwd=cwd,
        env=env,
        timeout=120,
    )
    if result.timed_out or result.returncode != 0:
        raise MutationContractError(f"cargo-mutants single-file discovery failed for {source_path}")
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise MutationContractError(f"cargo-mutants returned invalid JSON for {source_path}: {exc}") from exc
    if not isinstance(data, list):
        raise MutationContractError(f"cargo-mutants returned non-list JSON for {source_path}")
    return data


def clean_mutation_target(cwd: Path, env: dict[str, str], timeout: float) -> None:
    result = run_command(
        ("cargo", "clean", "-p", "trust-runtime"),
        cwd=cwd,
        env=env,
        timeout=timeout,
    )
    if result.timed_out or result.returncode != 0:
        raise MutationContractError("failed to clean trust-runtime mutation outputs")


def outcome_record(
    mutation: MutationSpec,
    candidate: dict[str, Any],
    build: CommandResult,
    test: CommandResult | None,
    result: str,
) -> dict[str, Any]:
    return {
        "id": mutation.id,
        "source_file": mutation.source_file,
        "function": mutation.function,
        "genre": candidate.get("genre"),
        "replacement": candidate.get("replacement"),
        "generated_mutant_name": candidate.get("name"),
        "result": result,
        "related_case_ids": list(mutation.related_case_ids),
        "survivor_action": mutation.survivor_action,
        "build_command": list(build.command),
        "build_exit_status": build.returncode,
        "build_timed_out": build.timed_out,
        "build_output_tail": output_tail(build),
        "test_command": list(test.command) if test else list(mutation.test_command),
        "test_exit_status": test.returncode if test else None,
        "test_timed_out": test.timed_out if test else False,
        "test_output_tail": output_tail(test) if test else "",
        "duration_seconds": round(build.duration_seconds + (test.duration_seconds if test else 0.0), 3),
    }


def run_command(
    command: Sequence[str],
    *,
    cwd: Path,
    env: dict[str, str],
    timeout: float,
) -> CommandResult:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            list(command),
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout,
        )
        return CommandResult(
            tuple(command),
            completed.returncode,
            completed.stdout,
            completed.stderr,
            False,
            time.monotonic() - started,
        )
    except subprocess.TimeoutExpired as exc:
        return CommandResult(
            tuple(command),
            None,
            decode_timeout_output(exc.stdout),
            decode_timeout_output(exc.stderr),
            True,
            time.monotonic() - started,
        )
    except OSError as exc:
        return CommandResult(
            tuple(command),
            None,
            "",
            str(exc),
            False,
            time.monotonic() - started,
        )


def command_has_infrastructure_failure(result: CommandResult) -> bool:
    return (result.returncode is None and not result.timed_out) or has_infrastructure_failure(
        result.returncode,
        result.stdout + "\n" + result.stderr,
    )


@contextlib.contextmanager
def archived_head(root: Path) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="trust-bytecode-validator-mutation-") as temp:
        scratch = Path(temp)
        archive = subprocess.Popen(
            [
                "git",
                "archive",
                "--format=tar",
                "HEAD",
                "Cargo.toml",
                "Cargo.lock",
                ".cargo",
                "crates",
                "xtask",
                "third_party",
            ],
            cwd=root,
            stdout=subprocess.PIPE,
        )
        assert archive.stdout is not None
        extract = subprocess.run(["tar", "-x", "-C", str(scratch)], stdin=archive.stdout, check=False)
        archive.stdout.close()
        archive_status = archive.wait()
        if archive_status != 0 or extract.returncode != 0:
            raise MutationContractError("failed to create isolated Git archive workspace")
        yield scratch


def ensure_validator_sources_match_head(root: Path, contract: MutationContract) -> None:
    for source_file in sorted({mutation.source_file for mutation in contract.mutations}):
        result = subprocess.run(
            ["git", "diff", "--quiet", "HEAD", "--", source_file],
            cwd=root,
            check=False,
        )
        if result.returncode != 0:
            raise MutationContractError(
                f"validator source differs from HEAD; commit or isolate product changes before mutation: {source_file}"
            )


def command_summary(result: CommandResult) -> dict[str, Any]:
    return {
        "command": list(result.command),
        "exit_status": result.returncode,
        "timed_out": result.timed_out,
        "duration_seconds": round(result.duration_seconds, 3),
    }


def output_tail(result: CommandResult | None, limit: int = 4000) -> str:
    if result is None:
        return ""
    combined = (result.stdout + "\n" + result.stderr).strip()
    return combined[-limit:]


def decode_timeout_output(value: str | bytes | None) -> str:
    if value is None:
        return ""
    return value.decode(errors="replace") if isinstance(value, bytes) else value


def git_output(root: Path, args: list[str]) -> str:
    return command_output(root, ["git", *args])


def command_output(cwd: Path, command: list[str]) -> str:
    completed = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        raise MutationContractError(f"command failed: {command}: {completed.stderr.strip()}")
    return completed.stdout.strip()


def timestamp() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test", default=DEFAULT_TEST_ID)
    parser.add_argument(
        "--output-json",
        type=Path,
        default=ROOT / "target/gate-artifacts/verification/bytecode-validator-mutation.json",
    )
    parser.add_argument(
        "--output-markdown",
        type=Path,
        default=ROOT / "target/gate-artifacts/verification/bytecode-validator-mutation.md",
    )
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--build-timeout", type=float, default=1800.0)
    parser.add_argument("--test-timeout", type=float, default=900.0)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        contract = load_mutation_contract(args.test, root=ROOT)
        report = execute_shard(
            contract=contract,
            root=ROOT,
            target_dir=args.target_dir,
            build_timeout=args.build_timeout,
            test_timeout=args.test_timeout,
        )
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_markdown.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        args.output_markdown.write_text(render_markdown(report))
        print(
            "bytecode-validator mutation shard complete: "
            f"{report['summary']['caught']} caught, {report['summary']['survived']} survived, "
            f"{report['summary']['unviable']} unviable"
        )
        return 0
    except MutationContractError as exc:
        print(f"bytecode-validator mutation shard failed: {exc}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
