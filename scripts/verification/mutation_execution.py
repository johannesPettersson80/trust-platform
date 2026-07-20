"""Shared, side-effect-contained primitives for focused source mutation."""

from __future__ import annotations

import contextlib
import json
import re
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterator, Mapping, Sequence

from .metadata_validator.mutation_contracts import MutationContractError
from .metadata_validator.mutation_reports import has_infrastructure_failure


@dataclass(frozen=True)
class CommandResult:
    command: tuple[str, ...]
    returncode: int | None
    stdout: str
    stderr: str
    timed_out: bool
    duration_seconds: float


def select_generated_mutant(
    candidates: list[dict[str, Any]], config: Mapping[str, Any]
) -> dict[str, Any]:
    matches = [
        candidate
        for candidate in candidates
        if candidate.get("function", {}).get("function_name") == config.get("function")
        and candidate.get("genre") == config.get("genre")
        and candidate.get("replacement") == config.get("replacement")
        and (
            config.get("selector_name") is None
            or candidate.get("name") == config.get("selector_name")
        )
    ]
    if len(matches) != 1:
        raise MutationContractError(
            f"mutation selector for {config.get('function')} found {len(matches)} generated mutants"
        )
    return matches[0]


def apply_generated_mutant(source: str, candidate: Mapping[str, Any]) -> str:
    span = candidate.get("span", {})
    if not isinstance(span, Mapping):
        raise MutationContractError("generated mutant span is not an object")
    start_value = span.get("start", {})
    end_value = span.get("end", {})
    if not isinstance(start_value, Mapping) or not isinstance(end_value, Mapping):
        raise MutationContractError("generated mutant positions are not objects")
    start = source_offset(source, start_value)
    end = source_offset(source, end_value)
    if start > end:
        raise MutationContractError("generated mutant span is reversed")
    replacement = candidate.get("replacement")
    if not isinstance(replacement, str):
        raise MutationContractError("generated mutant replacement is not text")
    return source[:start] + replacement + source[end:]


def source_offset(source: str, position: Mapping[str, Any]) -> int:
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


def package_from_build_command(command: Sequence[str]) -> str:
    packages: list[str] = []
    for index, item in enumerate(command):
        if item in {"-p", "--package"} and index + 1 < len(command):
            packages.append(command[index + 1])
        elif item.startswith("--package="):
            packages.append(item.partition("=")[2])
    if len(packages) != 1 or not packages[0]:
        raise ValueError("focused mutation build command must name exactly one package")
    return packages[0]


def classify_focused_mutant(
    *,
    source_file: str,
    selected_test_name: str,
    build: CommandResult,
    test: CommandResult | None,
) -> str:
    if command_has_infrastructure_failure(build) or (
        test is not None and command_has_infrastructure_failure(test)
    ):
        return "error"
    if build.timed_out or (test is not None and test.timed_out):
        return "timeout"
    if build.returncode != 0:
        output = combined_output(build)
        if source_file in output and re.search(r"(?m)^error(?:\[[A-Z0-9]+\])?:", output):
            return "unviable"
        return "error"
    if test is None:
        return "error"
    if test.returncode == 0:
        return "survived"
    return "caught" if selected_test_failed(test, selected_test_name) else "error"


def selected_test_failed(result: CommandResult, selected_test_name: str) -> bool:
    output = combined_output(result)
    name = re.escape(selected_test_name)
    failed_line = re.search(rf"(?m)^test .*{name} .*\.\.\. FAILED\s*$", output)
    failure_list = re.search(rf"(?m)^\s*(?:.*::)?{name}\s*$", output)
    failed_summary = "test result: FAILED" in output
    return bool(failed_line or (failure_list and failed_summary))


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


def discover_mutants(
    source_path: Path, cwd: Path, env: dict[str, str]
) -> list[dict[str, Any]]:
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
        raise MutationContractError(
            f"cargo-mutants returned invalid JSON for {source_path}: {exc}"
        ) from exc
    if not isinstance(data, list) or not all(isinstance(item, dict) for item in data):
        raise MutationContractError(f"cargo-mutants returned non-object list for {source_path}")
    return data


def clean_mutation_package(
    package: str, cwd: Path, env: dict[str, str], timeout: float
) -> None:
    result = run_command(
        ("cargo", "clean", "-p", package), cwd=cwd, env=env, timeout=timeout
    )
    if result.timed_out or result.returncode != 0:
        raise MutationContractError(f"failed to clean {package} mutation outputs")


@contextlib.contextmanager
def archived_head(root: Path, *, prefix: str) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix=prefix) as temp:
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
        extract = subprocess.run(
            ["tar", "-x", "-C", str(scratch)], stdin=archive.stdout, check=False
        )
        archive.stdout.close()
        archive_status = archive.wait()
        if archive_status != 0 or extract.returncode != 0:
            raise MutationContractError("failed to create isolated Git archive workspace")
        yield scratch


def command_has_infrastructure_failure(result: CommandResult) -> bool:
    return (result.returncode is None and not result.timed_out) or has_infrastructure_failure(
        result.returncode, combined_output(result)
    )


def command_summary(result: CommandResult) -> dict[str, Any]:
    return {
        "command": list(result.command),
        "exit_status": result.returncode,
        "timed_out": result.timed_out,
        "duration_seconds": round(result.duration_seconds, 3),
    }


def combined_output(result: CommandResult) -> str:
    return result.stdout + "\n" + result.stderr


def decode_timeout_output(value: str | bytes | None) -> str:
    if value is None:
        return ""
    return value.decode(errors="replace") if isinstance(value, bytes) else value
