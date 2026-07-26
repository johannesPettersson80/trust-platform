"""Execute and normalize one frozen Rust nextest catalog batch."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shlex
import subprocess
import sys
import tempfile
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation, ROUND_CEILING
from pathlib import Path, PurePosixPath
from typing import Any

from .coverage_trace_filter import exact_name_matcher


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
RUNNER_PATH = Path("scripts/verification/frozen_rust_nextest_batch.py")
SHIM_PATH = Path("scripts/run_frozen_rust_nextest_batch.py")
PACKAGE = "trust-runtime"
BINARY_ID = "trust-runtime"
TEST_NAME_PREFIX = "control::tests::"
MESSAGE_FORMAT_VERSION = "0.1"
EXPECTED_REVIEWED_NONMAPPING = 69
EXPECTED_CATALOG_MAPPED = 2
EXPERIMENTAL_ENVIRONMENT = {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"}
TERMINAL_EVENTS = frozenset({"ok", "failed", "ignored", "timeout"})


@dataclass(frozen=True)
class FrozenSelection:
    path: str
    reviewed_nonmapping_count: int
    catalog_mapped_count: int
    names: tuple[str, ...]
    full_names: tuple[str, ...]


def derive_frozen_selection(
    *,
    root: Path,
    path: str,
    expected_reviewed_nonmapping: int,
    expected_catalog_mapped: int,
) -> FrozenSelection:
    """Select only frozen nonmapping and catalog-mapped tests for one path."""

    _validate_catalog_path(path)
    if expected_reviewed_nonmapping < 0 or expected_catalog_mapped < 0:
        raise ValueError("expected selection counts must be non-negative")
    if expected_reviewed_nonmapping + expected_catalog_mapped == 0:
        raise ValueError("frozen selection cannot be empty")

    denominator = _load_toml(root / DENOMINATOR_PATH)
    catalog = _load_toml(root / CATALOG_PATH)
    reviews = _table_array(denominator, "reviews", "denominator")
    tests = _table_array(catalog, "tests", "catalog")
    path_reviews = [row for row in reviews if row.get("path") == path]
    path_tests = [row for row in tests if row.get("path") == path]
    unsupported_reviews = [
        row
        for row in path_reviews
        if row.get("disposition")
        not in {"reviewed_nonmapping", "catalog_mapped"}
    ]
    if unsupported_reviews:
        raise ValueError(
            f"{path} has denominator rows outside the frozen selection"
        )
    if any(row.get("status") != "mapped" for row in path_tests):
        raise ValueError(f"{path} has catalog rows that are not mapped")

    reviewed = [
        row
        for row in path_reviews
        if row.get("disposition") == "reviewed_nonmapping"
    ]
    denominator_mapped = [
        row
        for row in path_reviews
        if row.get("disposition") == "catalog_mapped"
    ]
    catalog_mapped = [
        row for row in path_tests if row.get("status") == "mapped"
    ]

    if len(reviewed) != expected_reviewed_nonmapping:
        raise ValueError(
            f"{path} has {len(reviewed)} reviewed_nonmapping rows, expected "
            f"{expected_reviewed_nonmapping}"
        )
    if len(denominator_mapped) != expected_catalog_mapped:
        raise ValueError(
            f"{path} has {len(denominator_mapped)} catalog_mapped denominator "
            f"rows, expected {expected_catalog_mapped}"
        )
    if len(catalog_mapped) != expected_catalog_mapped:
        raise ValueError(
            f"{path} has {len(catalog_mapped)} mapped catalog rows, expected "
            f"{expected_catalog_mapped}"
        )

    reviewed_identities = _review_identities(reviewed, "reviewed denominator")
    mapped_denominator_identities = _review_identities(
        denominator_mapped, "mapped denominator"
    )
    mapped_catalog_identities = _catalog_identities(catalog_mapped)
    if set(mapped_denominator_identities) != set(mapped_catalog_identities):
        raise ValueError(
            "mapped denominator identities do not equal mapped catalog identities"
        )

    combined = reviewed_identities + mapped_denominator_identities
    discovery_ids = [discovery_id for discovery_id, _ in combined]
    if len(discovery_ids) != len(set(discovery_ids)):
        raise ValueError("duplicate selected discovery ID")
    names = [name for _, name in combined]
    if len(names) != len(set(names)):
        raise ValueError("duplicate selected test name")
    ordered_names = tuple(sorted(names))
    return FrozenSelection(
        path=path,
        reviewed_nonmapping_count=len(reviewed),
        catalog_mapped_count=len(denominator_mapped),
        names=ordered_names,
        full_names=tuple(TEST_NAME_PREFIX + name for name in ordered_names),
    )


def build_nextest_command(full_names: Sequence[str]) -> list[str]:
    """Build one shell-free exact-name filter for the selected library tests."""

    ordered = tuple(sorted(full_names))
    if not ordered:
        raise ValueError("nextest batch requires at least one test")
    if len(ordered) != len(set(ordered)):
        raise ValueError("nextest batch contains duplicate full test names")
    clauses = []
    for name in ordered:
        if not isinstance(name, str) or not name.startswith(TEST_NAME_PREFIX):
            raise ValueError(
                f"nextest test name is outside required prefix {TEST_NAME_PREFIX}"
            )
        clauses.append(
            f"(binary_id({exact_name_matcher(BINARY_ID)}) & "
            f"test({exact_name_matcher(name)}))"
        )
    return [
        "cargo",
        "nextest",
        "run",
        "-p",
        PACKAGE,
        "--lib",
        "--no-fail-fast",
        "--message-format",
        "libtest-json-plus",
        "--message-format-version",
        MESSAGE_FORMAT_VERSION,
        "--user-config-file",
        "none",
        "-E",
        " | ".join(clauses),
    ]


def build_catalog_command(test_name: str) -> str:
    """Return the canonical exact command for one selected catalog test."""

    if (
        not isinstance(test_name, str)
        or not test_name
        or test_name.startswith(TEST_NAME_PREFIX)
    ):
        raise ValueError("catalog test name must be a nonempty unqualified name")
    full_name = TEST_NAME_PREFIX + test_name
    expression = (
        f"binary_id({exact_name_matcher(BINARY_ID)}) & "
        f"test({exact_name_matcher(full_name)})"
    )
    return shlex.join(
        [
            "cargo",
            "nextest",
            "run",
            "-p",
            PACKAGE,
            "--lib",
            "--no-fail-fast",
            "--user-config-file",
            "none",
            "-E",
            expression,
        ]
    )


def normalize_nextest_jsonl(
    raw: str,
    *,
    expected_full_names: set[str],
) -> list[dict[str, Any]]:
    """Normalize exact passing libtest JSON events to catalog batch rows."""

    if not expected_full_names:
        raise ValueError("normalizer requires at least one expected test")
    if any(not name.startswith(TEST_NAME_PREFIX) for name in expected_full_names):
        raise ValueError("expected test name is outside required prefix")

    started: set[str] = set()
    terminal: dict[str, dict[str, Any]] = {}
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(
                f"raw nextest line {line_number} is not JSON: {error.msg}"
            ) from error
        if not isinstance(event, Mapping):
            raise ValueError(f"raw nextest line {line_number} is not an object")
        if event.get("type") != "test":
            continue
        name = event.get("name")
        event_name = event.get("event")
        if not isinstance(name, str) or not name:
            raise ValueError(f"test event on line {line_number} lacks a name")
        if not name.startswith(TEST_NAME_PREFIX):
            raise ValueError(
                f"test result {name!r} is outside required prefix "
                f"{TEST_NAME_PREFIX}"
            )
        if name not in expected_full_names:
            raise ValueError(f"unexpected test result {name}")
        if event_name == "started":
            if name in started:
                raise ValueError(f"duplicate started result for {name}")
            started.add(name)
            continue
        if event_name not in TERMINAL_EVENTS:
            raise ValueError(
                f"test result {name} has unknown event {event_name!r}"
            )
        if name in terminal:
            raise ValueError(f"duplicate terminal result for {name}")
        if event_name != "ok":
            raise ValueError(f"test {name} did not pass: {event_name}")
        duration_ms = _duration_milliseconds(event.get("exec_time"), name=name)
        terminal[name] = {
            "title": name.removeprefix(TEST_NAME_PREFIX),
            "duration": duration_ms,
            "state": "passed",
        }

    actual_names = set(terminal)
    missing = sorted(expected_full_names - actual_names)
    if missing:
        raise ValueError("missing test results: " + ", ".join(missing))
    unexpected = sorted(actual_names - expected_full_names)
    if unexpected:
        raise ValueError("unexpected test results: " + ", ".join(unexpected))
    return [terminal[name] for name in sorted(terminal)]


def build_normalized_document(
    *,
    root: Path,
    selection: FrozenSelection,
    raw_path: Path,
    metadata: Mapping[str, Any],
) -> dict[str, Any]:
    command = build_nextest_command(selection.full_names)
    _validate_execution_metadata(
        root=root,
        selection=selection,
        raw_path=raw_path,
        metadata=metadata,
        command=command,
    )
    raw = raw_path.read_text()
    tests = normalize_nextest_jsonl(
        raw,
        expected_full_names=set(selection.full_names),
    )
    return {
        "schema_version": 1,
        "execution": {
            "kind": "rust_nextest_jsonl",
            "batch_command": metadata["command"],
            "runner_path": SHIM_PATH.as_posix(),
            "runner_sha256": _digest_file(root / SHIM_PATH),
            "source_path": selection.path,
            "source_sha256": _digest_file(root / selection.path),
            "raw_results_path": _repo_relative(root, raw_path),
            "raw_results_sha256": _digest_file(raw_path),
            "nextest_version": metadata["nextest_version"],
            "package": PACKAGE,
            "target_kind": "lib",
            "binary_id": BINARY_ID,
            "test_prefix": TEST_NAME_PREFIX,
        },
        "tests": tests,
        "failures": [],
        "pending": [],
        "stats": {
            "tests": len(tests),
            "passes": len(tests),
            "failures": 0,
            "pending": 0,
        },
    }


def execute_nextest_batch(
    *,
    root: Path,
    selection: FrozenSelection,
    raw_path: Path,
    metadata_path: Path,
) -> Mapping[str, Any]:
    """Execute once, retaining raw JSONL and immutable execution metadata."""

    if raw_path.exists() or metadata_path.exists():
        raise ValueError(
            "raw results or execution metadata already exist; use "
            "--normalize-only to avoid rerunning tests"
        )
    raw_path.parent.mkdir(parents=True, exist_ok=True)
    metadata_path.parent.mkdir(parents=True, exist_ok=True)
    command = build_nextest_command(selection.full_names)
    version = subprocess.run(
        ["cargo", "nextest", "--version"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if not version:
        raise ValueError("cargo nextest --version returned an empty version")

    started_at = _utc_now()
    environment = dict(os.environ)
    environment.update(EXPERIMENTAL_ENVIRONMENT)
    try:
        with raw_path.open("xb") as raw_file:
            completed = subprocess.run(
                command,
                cwd=root,
                env=environment,
                stdout=raw_file,
                stderr=subprocess.PIPE,
            )
    except OSError:
        if raw_path.exists() and raw_path.stat().st_size == 0:
            raw_path.unlink()
        raise
    finished_at = _utc_now()
    if completed.stderr:
        sys.stderr.buffer.write(completed.stderr)

    metadata = {
        "schema_version": 1,
        "command": shlex.join(command),
        "argv": command,
        "environment": dict(EXPERIMENTAL_ENVIRONMENT),
        "nextest_version": version,
        "exit_code": completed.returncode,
        "started_at": started_at,
        "finished_at": finished_at,
        "path": selection.path,
        "reviewed_nonmapping_count": selection.reviewed_nonmapping_count,
        "catalog_mapped_count": selection.catalog_mapped_count,
        "selected_test_count": len(selection.full_names),
        "selected_names_sha256": _selected_names_digest(selection.full_names),
        "source_sha256": _digest_file(root / selection.path),
        "denominator_sha256": _digest_file(root / DENOMINATOR_PATH),
        "catalog_sha256": _digest_file(root / CATALOG_PATH),
        "module_path": RUNNER_PATH.as_posix(),
        "module_sha256": _digest_file(root / RUNNER_PATH),
        "shim_path": SHIM_PATH.as_posix(),
        "shim_sha256": _digest_file(root / SHIM_PATH),
        "raw_results_path": _repo_relative(root, raw_path),
        "raw_results_sha256": _digest_file(raw_path),
    }
    _write_json_atomic(metadata_path, metadata)
    return metadata


def _validate_execution_metadata(
    *,
    root: Path,
    selection: FrozenSelection,
    raw_path: Path,
    metadata: Mapping[str, Any],
    command: Sequence[str],
) -> None:
    required_fields = {
        "schema_version",
        "command",
        "argv",
        "environment",
        "nextest_version",
        "exit_code",
        "started_at",
        "finished_at",
        "path",
        "reviewed_nonmapping_count",
        "catalog_mapped_count",
        "selected_test_count",
        "selected_names_sha256",
        "source_sha256",
        "denominator_sha256",
        "catalog_sha256",
        "module_path",
        "module_sha256",
        "shim_path",
        "shim_sha256",
        "raw_results_path",
        "raw_results_sha256",
    }
    if set(metadata) != required_fields:
        raise ValueError("execution metadata does not have the closed field set")
    expected = {
        "schema_version": 1,
        "command": shlex.join(command),
        "argv": list(command),
        "environment": dict(EXPERIMENTAL_ENVIRONMENT),
        "exit_code": 0,
        "path": selection.path,
        "reviewed_nonmapping_count": selection.reviewed_nonmapping_count,
        "catalog_mapped_count": selection.catalog_mapped_count,
        "selected_test_count": len(selection.full_names),
        "selected_names_sha256": _selected_names_digest(selection.full_names),
        "source_sha256": _digest_file(root / selection.path),
        "denominator_sha256": _digest_file(root / DENOMINATOR_PATH),
        "catalog_sha256": _digest_file(root / CATALOG_PATH),
        "raw_results_path": _repo_relative(root, raw_path),
        "raw_results_sha256": _digest_file(raw_path),
        "module_path": RUNNER_PATH.as_posix(),
        "shim_path": SHIM_PATH.as_posix(),
    }
    for field, value in expected.items():
        if metadata.get(field) != value:
            raise ValueError(
                f"execution metadata {field} does not match the frozen batch"
            )
    for field in (
        "nextest_version",
        "started_at",
        "finished_at",
        "module_sha256",
        "shim_sha256",
    ):
        if not isinstance(metadata.get(field), str) or not metadata[field]:
            raise ValueError(f"execution metadata requires non-empty {field}")
    for field in ("module_sha256", "shim_sha256"):
        if not str(metadata[field]).startswith("sha256:"):
            raise ValueError(f"execution metadata {field} is not a SHA-256 digest")


def _review_identities(
    rows: Sequence[Mapping[str, Any]],
    label: str,
) -> list[tuple[str, str]]:
    identities: list[tuple[str, str]] = []
    for row in rows:
        if row.get("discovery_source_kind") != "rust_unit_test":
            raise ValueError(f"{label} row is not a rust_unit_test")
        if row.get("ignore_state") != "not_ignored":
            raise ValueError(f"{label} row is not an active test")
        identities.append(
            (
                _required_string(row, "discovery_id", label),
                _required_string(row, "name", label),
            )
        )
    if len(identities) != len(set(identities)):
        raise ValueError(f"{label} contains duplicate identities")
    return identities


def _catalog_identities(
    rows: Sequence[Mapping[str, Any]],
) -> list[tuple[str, str]]:
    identities = []
    for row in rows:
        if row.get("discovery_source_kind") != "rust_unit_test":
            raise ValueError("mapped catalog row is not a rust_unit_test")
        identities.append(
            (
                _required_string(row, "discovery_id", "mapped catalog"),
                _required_string(row, "name", "mapped catalog"),
            )
        )
    if len(identities) != len(set(identities)):
        raise ValueError("mapped catalog contains duplicate identities")
    return identities


def _duration_milliseconds(value: Any, *, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"passing test {name} lacks numeric exec_time")
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError(f"passing test {name} has non-finite exec_time")
    try:
        seconds = Decimal(str(value))
    except InvalidOperation as error:
        raise ValueError(f"passing test {name} has invalid exec_time") from error
    if not seconds.is_finite() or seconds < 0:
        raise ValueError(f"passing test {name} has invalid exec_time")
    return int(
        (seconds * Decimal(1000)).to_integral_value(rounding=ROUND_CEILING)
    )


def _selected_names_digest(full_names: Sequence[str]) -> str:
    encoded = json.dumps(
        sorted(full_names),
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode()
    return _digest_bytes(encoded)


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _table_array(
    document: Mapping[str, Any],
    field: str,
    label: str,
) -> list[Mapping[str, Any]]:
    value = document.get(field)
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} lacks {field} array")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} {field} contains a non-table row")
    return list(value)


def _required_string(
    row: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _validate_catalog_path(path: str) -> None:
    parsed = PurePosixPath(path)
    if (
        not path
        or parsed.is_absolute()
        or ".." in parsed.parts
        or parsed.as_posix() != path
    ):
        raise ValueError("test path must be a normalized repository-relative path")


def _checked_repo_file(root: Path, relative: Path, label: str) -> Path:
    unresolved = root / relative
    if unresolved.is_symlink():
        raise ValueError(f"{label} must be a regular non-symlink file")
    path = unresolved.resolve()
    try:
        path.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} escapes the repository") from error
    if not path.is_file():
        raise ValueError(f"{label} must be a regular non-symlink file")
    return path


def _checked_output_path(root: Path, value: Path, label: str) -> Path:
    unresolved = value if value.is_absolute() else root / value
    if unresolved.is_symlink():
        raise ValueError(f"{label} cannot be a symlink")
    path = unresolved.resolve()
    try:
        relative = path.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} escapes the repository") from error
    if relative.parts[:2] != ("target", "gate-artifacts"):
        raise ValueError(f"{label} must live under target/gate-artifacts")
    return path


def _repo_relative(root: Path, path: Path) -> str:
    return path.resolve().relative_to(root).as_posix()


def _digest_file(path: Path) -> str:
    return _digest_bytes(path.read_bytes())


def _digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = json.dumps(document, indent=2, ensure_ascii=False) + "\n"
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as temporary:
        temporary.write(content)
        temporary_path = Path(temporary.name)
    os.replace(temporary_path, path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--path", required=True)
    parser.add_argument(
        "--expected-reviewed-nonmapping",
        type=int,
        default=EXPECTED_REVIEWED_NONMAPPING,
    )
    parser.add_argument(
        "--expected-catalog-mapped",
        type=int,
        default=EXPECTED_CATALOG_MAPPED,
    )
    parser.add_argument("--raw-results", type=Path, required=True)
    parser.add_argument("--execution-metadata", type=Path)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument(
        "--normalize-only",
        action="store_true",
        help="reuse existing raw JSONL and execution metadata without running tests",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        if not root.is_dir():
            raise ValueError("repository root does not exist")
        _checked_repo_file(root, DENOMINATOR_PATH, "test denominator")
        _checked_repo_file(root, CATALOG_PATH, "test catalog")
        _validate_catalog_path(args.path)
        _checked_repo_file(root, Path(args.path), "test source")
        raw_path = _checked_output_path(root, args.raw_results, "raw results")
        results_path = _checked_output_path(root, args.results, "results")
        metadata_value = args.execution_metadata or Path(
            str(args.raw_results) + ".execution.json"
        )
        metadata_path = _checked_output_path(
            root, metadata_value, "execution metadata"
        )
        if len({raw_path, metadata_path, results_path}) != 3:
            raise ValueError(
                "raw results, execution metadata, and results paths must differ"
            )
        selection = derive_frozen_selection(
            root=root,
            path=args.path,
            expected_reviewed_nonmapping=args.expected_reviewed_nonmapping,
            expected_catalog_mapped=args.expected_catalog_mapped,
        )
        if args.normalize_only:
            _checked_repo_file(
                root,
                Path(_repo_relative(root, raw_path)),
                "raw results",
            )
            _checked_repo_file(
                root,
                Path(_repo_relative(root, metadata_path)),
                "execution metadata",
            )
            metadata = json.loads(metadata_path.read_text())
            if not isinstance(metadata, Mapping):
                raise ValueError("execution metadata is not a JSON object")
        else:
            metadata = execute_nextest_batch(
                root=root,
                selection=selection,
                raw_path=raw_path,
                metadata_path=metadata_path,
            )
        document = build_normalized_document(
            root=root,
            selection=selection,
            raw_path=raw_path,
            metadata=metadata,
        )
        _write_json_atomic(results_path, document)
        print(
            "frozen Rust nextest batch normalized: "
            f"{len(document['tests'])} passing tests -> "
            f"{_repo_relative(root, results_path)}"
        )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"frozen Rust nextest batch failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
