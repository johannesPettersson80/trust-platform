"""Nextest run wrapper for attempt-isolated LLVM coverage profiles."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping
from pathlib import Path


TRACE_ROOT_ENV = "TRUST_CATALOG_TRACE_ROOT"
NEXTEST_IDENTITY_ENV = (
    "NEXTEST_TEST_NAME",
    "NEXTEST_BINARY_ID",
    "NEXTEST_ATTEMPT_ID",
)
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
REQUESTS_NAME = "requested-tests.json"


def prepare_attempt_environment(
    environment: Mapping[str, str],
    *,
    command: list[str] | None = None,
) -> tuple[dict[str, str], Path, dict[str, object]]:
    """Return the child environment, attempt directory, and attempt manifest."""

    values: dict[str, str] = {}
    for field in (TRACE_ROOT_ENV, *NEXTEST_IDENTITY_ENV):
        value = environment.get(field)
        if not isinstance(value, str) or not value:
            raise ValueError(f"coverage trace wrapper requires {field}")
        values[field] = value
    trace_root = Path(values[TRACE_ROOT_ENV])
    if not trace_root.is_absolute():
        raise ValueError(f"{TRACE_ROOT_ENV} must be an absolute path")
    if not command:
        raise ValueError("coverage trace wrapper requires a test executable")
    test_executable = Path(command[0]).resolve()
    discovery_id = _requested_discovery_id(
        trace_root,
        binary_id=values["NEXTEST_BINARY_ID"],
        binary_path=test_executable,
        test_name=values["NEXTEST_TEST_NAME"],
    )
    identity = {
        "discovery_id": discovery_id,
        "test_name": values["NEXTEST_TEST_NAME"],
        "binary_id": values["NEXTEST_BINARY_ID"],
        "attempt_id": values["NEXTEST_ATTEMPT_ID"],
    }
    identity_bytes = json.dumps(
        identity, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    attempt_key = hashlib.sha256(identity_bytes).hexdigest()
    attempt_dir = trace_root / "attempts" / attempt_key
    profiles_dir = attempt_dir / "profiles"
    profiles_dir.mkdir(parents=True, exist_ok=True)
    manifest = {
        **identity,
        "attempt_key": attempt_key,
        "profile_pattern": (profiles_dir / "%m-%p.profraw").as_posix(),
        "test_executable": str(test_executable),
        "test_executable_digest": (
            _file_digest(test_executable)
            if test_executable.is_file()
            else None
        ),
        "test_arguments": command[1:],
        "status": "started",
    }
    _atomic_write_json(attempt_dir / "attempt.json", manifest)
    child_environment = dict(environment)
    child_environment["LLVM_PROFILE_FILE"] = manifest["profile_pattern"]
    return child_environment, attempt_dir, manifest


def _file_digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    command = list(sys.argv[1:] if argv is None else argv)
    if not command:
        print("coverage trace wrapper requires a test command", file=sys.stderr)
        return 2
    try:
        child_environment, attempt_dir, manifest = prepare_attempt_environment(
            os.environ,
            command=command,
        )
    except (OSError, ValueError) as exc:
        print(f"coverage trace wrapper failed: {exc}", file=sys.stderr)
        return 2
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(command, env=child_environment, check=False)
        exit_code = completed.returncode
    except OSError as exc:
        print(f"coverage trace child failed to start: {exc}", file=sys.stderr)
        exit_code = 127
    duration_ms = max(0, (time.monotonic_ns() - started) // 1_000_000)
    complete_attempt_manifest(
        attempt_dir,
        manifest,
        exit_code=exit_code,
        duration_ms=duration_ms,
    )
    return exit_code


def _atomic_write_json(path: Path, value: Mapping[str, object]) -> None:
    rendered = json.dumps(dict(value), indent=2, sort_keys=True) + "\n"
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            delete=False,
        ) as stream:
            temporary_name = stream.name
            stream.write(rendered)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_name, path)
    finally:
        if temporary_name is not None:
            temporary = Path(temporary_name)
            if temporary.exists():
                temporary.unlink()


def complete_attempt_manifest(
    attempt_dir: Path,
    manifest: Mapping[str, object],
    *,
    exit_code: int,
    duration_ms: int,
) -> dict[str, object]:
    if manifest.get("status") != "started":
        raise ValueError("coverage trace attempt is not in started state")
    if not isinstance(exit_code, int) or isinstance(exit_code, bool):
        raise ValueError("coverage trace exit code must be an integer")
    if (
        not isinstance(duration_ms, int)
        or isinstance(duration_ms, bool)
        or duration_ms < 0
    ):
        raise ValueError("coverage trace duration must be a nonnegative integer")
    completed = {
        **manifest,
        "status": "completed",
        "exit_code": exit_code,
        "duration_ms": duration_ms,
    }
    _atomic_write_json(attempt_dir / "attempt.json", completed)
    return completed


def _requested_discovery_id(
    trace_root: Path,
    *,
    binary_id: str,
    binary_path: Path,
    test_name: str,
) -> str:
    path = trace_root / REQUESTS_NAME
    try:
        document = json.loads(path.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"coverage trace wrapper cannot load {REQUESTS_NAME}") from error
    if not isinstance(document, Mapping) or set(document) != {
        "schema_version",
        "tests",
    }:
        raise ValueError(f"{REQUESTS_NAME} root fields drift")
    if document.get("schema_version") != 2:
        raise ValueError(f"{REQUESTS_NAME} schema_version must equal 2")
    tests = document.get("tests")
    if not isinstance(tests, list):
        raise ValueError(f"{REQUESTS_NAME} tests must be an array")
    by_identity: dict[tuple[str, str, Path], str] = {}
    discovery_ids: set[str] = set()
    for index, record in enumerate(tests):
        if not isinstance(record, Mapping) or set(record) != {
            "discovery_id",
            "nextest_binary_id",
            "nextest_binary_path",
            "nextest_test_name",
        }:
            raise ValueError(f"{REQUESTS_NAME} test[{index}] fields drift")
        discovery_id = record.get("discovery_id")
        requested_binary = record.get("nextest_binary_id")
        requested_path = record.get("nextest_binary_path")
        requested_name = record.get("nextest_test_name")
        if (
            not isinstance(discovery_id, str)
            or DISCOVERY_ID_RE.fullmatch(discovery_id) is None
            or not isinstance(requested_binary, str)
            or not requested_binary
            or not isinstance(requested_path, str)
            or not requested_path
            or not Path(requested_path).is_absolute()
            or not isinstance(requested_name, str)
            or not requested_name
        ):
            raise ValueError(f"{REQUESTS_NAME} test[{index}] identity is invalid")
        identity = (
            requested_binary,
            requested_name,
            Path(requested_path).resolve(),
        )
        if identity in by_identity or discovery_id in discovery_ids:
            raise ValueError(f"{REQUESTS_NAME} contains duplicate identity")
        by_identity[identity] = discovery_id
        discovery_ids.add(discovery_id)
    try:
        return by_identity[(binary_id, test_name, binary_path.resolve())]
    except KeyError as error:
        raise ValueError(
            f"runtime identity {binary_id}${test_name} is not in requested-tests"
        ) from error


if __name__ == "__main__":
    raise SystemExit(main())
