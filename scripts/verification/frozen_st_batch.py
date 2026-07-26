"""Execute and normalize one frozen Structured Text catalog batch."""

from __future__ import annotations

import argparse
import hashlib
import json
import shlex
import subprocess
import sys
import tempfile
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
RUNNER_PATH = Path("scripts/verification/frozen_st_batch.py")
SHIM_PATH = Path("scripts/run_frozen_st_batch.py")
DISCOVERY_SOURCE_KIND = "structured_text_test"


@dataclass(frozen=True)
class FrozenStSelection:
    """Exact names selected from one frozen ST denominator path."""

    path: str
    reviewed_nonmapping_count: int
    catalog_mapped_count: int
    names: tuple[str, ...]


def derive_frozen_selection(
    *,
    root: Path,
    path: str,
    expected_reviewed_nonmapping: int,
    expected_catalog_mapped: int,
) -> FrozenStSelection:
    """Select only reviewed nonmappings plus mapped extras for one ST path."""

    _validate_source_path(path)
    if expected_reviewed_nonmapping < 0 or expected_catalog_mapped < 0:
        raise ValueError("expected selection counts must be non-negative")
    if expected_reviewed_nonmapping + expected_catalog_mapped == 0:
        raise ValueError("frozen ST selection cannot be empty")

    denominator = _load_toml(root / DENOMINATOR_PATH)
    catalog = _load_toml(root / CATALOG_PATH)
    reviews = _table_array(denominator, "reviews", "denominator")
    tests = _table_array(catalog, "tests", "catalog")
    path_reviews = [row for row in reviews if row.get("path") == path]
    path_tests = [row for row in tests if row.get("path") == path]
    if any(
        row.get("disposition")
        not in {"reviewed_nonmapping", "catalog_mapped"}
        for row in path_reviews
    ):
        raise ValueError(f"{path} has rows outside the frozen ST selection")
    if any(row.get("status") != "mapped" for row in path_tests):
        raise ValueError(f"{path} has catalog rows that are not mapped")

    reviewed = [
        row
        for row in path_reviews
        if row.get("disposition") == "reviewed_nonmapping"
    ]
    mapped_reviews = [
        row
        for row in path_reviews
        if row.get("disposition") == "catalog_mapped"
    ]
    if len(reviewed) != expected_reviewed_nonmapping:
        raise ValueError(
            f"{path} has {len(reviewed)} reviewed_nonmapping rows, expected "
            f"{expected_reviewed_nonmapping}"
        )
    if len(mapped_reviews) != expected_catalog_mapped:
        raise ValueError(
            f"{path} has {len(mapped_reviews)} catalog_mapped denominator "
            f"rows, expected {expected_catalog_mapped}"
        )
    if len(path_tests) != expected_catalog_mapped:
        raise ValueError(
            f"{path} has {len(path_tests)} mapped catalog rows, expected "
            f"{expected_catalog_mapped}"
        )

    reviewed_identities = _review_identities(reviewed, "reviewed denominator")
    mapped_identities = _review_identities(
        mapped_reviews, "mapped denominator"
    )
    catalog_identities = _catalog_identities(path_tests)
    if set(mapped_identities) != set(catalog_identities):
        raise ValueError(
            "mapped ST denominator identities do not equal catalog identities"
        )
    combined = reviewed_identities + mapped_identities
    discovery_ids = [identity[0] for identity in combined]
    names = [identity[1] for identity in combined]
    if len(discovery_ids) != len(set(discovery_ids)):
        raise ValueError("frozen ST selection has duplicate discovery IDs")
    if len(names) != len(set(names)):
        raise ValueError("frozen ST selection has duplicate test names")
    return FrozenStSelection(
        path=path,
        reviewed_nonmapping_count=len(reviewed),
        catalog_mapped_count=len(mapped_reviews),
        names=tuple(sorted(names)),
    )


def build_test_command(
    *,
    project_path: str,
    test_prefix: str,
    trust_dev_binary: Path | None = None,
    cargo_command: str | None = None,
) -> list[str]:
    """Build one shell-free trust-dev command for a shared ST prefix."""

    if bool(trust_dev_binary) == bool(cargo_command):
        raise ValueError(
            "supply exactly one trust-dev binary or cargo command"
        )
    if not project_path or not test_prefix:
        raise ValueError("project path and test prefix must be non-empty")
    if trust_dev_binary is not None:
        if (
            not trust_dev_binary.is_file()
            or trust_dev_binary.is_symlink()
        ):
            raise ValueError("trust-dev binary is not an existing regular file")
        prefix = [str(trust_dev_binary)]
    else:
        try:
            prefix = shlex.split(str(cargo_command))
        except ValueError as error:
            raise ValueError(f"cargo command is invalid: {error}") from error
        if (
            len(prefix) < 3
            or Path(prefix[0]).name not in {"cargo", "cargo.exe"}
            or prefix[1] != "run"
            or prefix[-1] != "--"
        ):
            raise ValueError(
                "cargo command must be a shell-free 'cargo run ... --' prefix"
            )
    return [
        *prefix,
        "test",
        "--project",
        project_path,
        "--filter",
        test_prefix,
        "--output",
        "json",
    ]


def normalize_st_json(
    raw: str,
    *,
    expected_names: set[str],
    expected_project: str | None = None,
) -> dict[str, Any]:
    """Normalize the closed trust-dev JSON contract into catalog results."""

    if not expected_names:
        raise ValueError("ST normalizer requires at least one expected test")
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ValueError(f"raw ST results are not JSON: {error.msg}") from error
    if not isinstance(document, Mapping):
        raise ValueError("raw ST results are not an object")
    if set(document) != {"version", "project", "summary", "tests"}:
        raise ValueError("raw ST results do not have the closed field set")
    if document.get("version") != 1:
        raise ValueError("raw ST results have an unsupported version")
    if not isinstance(document.get("project"), str) or not document["project"]:
        raise ValueError("raw ST results lack a project")
    if (
        expected_project is not None
        and document["project"] != expected_project
    ):
        raise ValueError("raw ST project does not match the frozen command")

    summary = document.get("summary")
    if not isinstance(summary, Mapping) or set(summary) != {
        "total",
        "passed",
        "failed",
        "errors",
        "duration_ms",
    }:
        raise ValueError("raw ST summary does not have the closed field set")
    for field in ("total", "passed", "failed", "errors", "duration_ms"):
        _nonnegative_integer(summary.get(field), f"ST summary {field}")

    raw_tests = document.get("tests")
    if not isinstance(raw_tests, Sequence) or isinstance(
        raw_tests, (str, bytes)
    ):
        raise ValueError("raw ST results lack tests")
    normalized_tests: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    counts = {"passed": 0, "failed": 0, "error": 0}
    required_test_fields = {
        "name",
        "kind",
        "status",
        "file",
        "line",
        "source",
        "message",
        "duration_ms",
    }
    for row in raw_tests:
        if not isinstance(row, Mapping) or set(row) != required_test_fields:
            raise ValueError(
                "raw ST test does not have the closed field set"
            )
        name = _required_string(row, "name", "raw ST test")
        if name in seen_names:
            raise ValueError(f"raw ST results duplicate test {name}")
        seen_names.add(name)
        if row.get("kind") not in {"TEST_PROGRAM", "TEST_FUNCTION_BLOCK"}:
            raise ValueError(f"raw ST test {name} has an invalid kind")
        state = row.get("status")
        if state not in counts:
            raise ValueError(f"raw ST test {name} has an invalid status")
        file_path = row.get("file")
        source = row.get("source")
        message = row.get("message")
        if not isinstance(file_path, str) or not file_path:
            raise ValueError(f"raw ST test {name} lacks a file")
        if source is not None and not isinstance(source, str):
            raise ValueError(f"raw ST test {name} has invalid source")
        if message is not None and not isinstance(message, str):
            raise ValueError(f"raw ST test {name} has invalid message")
        _positive_integer(row.get("line"), f"raw ST test {name} line")
        duration = _nonnegative_integer(
            row.get("duration_ms"), f"raw ST test {name} duration"
        )
        if state == "passed" and message is not None:
            raise ValueError(f"passing raw ST test {name} has a message")
        if state != "passed" and not message:
            raise ValueError(f"failing raw ST test {name} lacks a message")
        counts[str(state)] += 1
        normalized_tests.append(
            {"title": name, "duration": duration, "state": state}
        )
        if state != "passed":
            failures.append(
                {"title": name, "state": state, "message": message}
            )

    if seen_names != expected_names:
        raise ValueError("raw ST test names do not equal frozen selection")
    expected_summary = {
        "total": len(raw_tests),
        "passed": counts["passed"],
        "failed": counts["failed"],
        "errors": counts["error"],
    }
    for field, value in expected_summary.items():
        if summary.get(field) != value:
            raise ValueError(f"raw ST summary {field} does not match tests")
    normalized_tests.sort(key=lambda row: str(row["title"]))
    failures.sort(key=lambda row: str(row["title"]))
    return {
        "tests": normalized_tests,
        "failures": failures,
        "pending": [],
        "stats": {
            "tests": len(raw_tests),
            "passes": counts["passed"],
            "failures": counts["failed"] + counts["error"],
            "pending": 0,
        },
    }


def build_results_document(
    *,
    root: Path,
    selection: FrozenStSelection,
    project_path: str,
    test_prefix: str,
    command: Sequence[str],
    raw_path: Path,
    exit_code: int,
) -> dict[str, Any]:
    """Bind normalized results to the exact live runner inputs."""

    if any(not name.startswith(test_prefix) for name in selection.names):
        raise ValueError("frozen ST names do not share the required prefix")
    normalized = normalize_st_json(
        raw_path.read_text(),
        expected_names=set(selection.names),
        expected_project=project_path,
    )
    if exit_code == 0 and normalized["failures"]:
        raise ValueError("trust-dev returned success with failing ST results")
    if exit_code != 0 and not normalized["failures"]:
        raise ValueError("trust-dev failed without a reported ST test failure")
    execution = {
        "kind": "st_json",
        "command": shlex.join(command),
        "runner_path": RUNNER_PATH.as_posix(),
        "runner_sha256": _digest_file(root / RUNNER_PATH),
        "shim_path": SHIM_PATH.as_posix(),
        "shim_sha256": _digest_file(root / SHIM_PATH),
        "source_path": selection.path,
        "source_sha256": _digest_file(root / selection.path),
        "raw_results_path": _repo_relative(root, raw_path),
        "raw_results_sha256": _digest_file(raw_path),
        "git_head": _git_head(root),
        "project_path": project_path,
        "test_prefix": test_prefix,
    }
    return {"execution": execution, **normalized}


def execute_batch(
    *,
    root: Path,
    selection: FrozenStSelection,
    project_path: str,
    test_prefix: str,
    command: Sequence[str],
    raw_path: Path,
) -> tuple[dict[str, Any], int]:
    """Run the one frozen command and return its normalized evidence."""

    if raw_path.exists():
        raise ValueError("raw ST results path already exists")
    raw_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with raw_path.open("xb") as raw_file:
            completed = subprocess.run(
                list(command),
                cwd=root,
                stdout=raw_file,
                stderr=subprocess.PIPE,
            )
    except OSError:
        if raw_path.exists() and raw_path.stat().st_size == 0:
            raw_path.unlink()
        raise
    if completed.stderr:
        sys.stderr.buffer.write(completed.stderr)
    return (
        build_results_document(
            root=root,
            selection=selection,
            project_path=project_path,
            test_prefix=test_prefix,
            command=command,
            raw_path=raw_path,
            exit_code=completed.returncode,
        ),
        completed.returncode,
    )


def _review_identities(
    rows: Sequence[Mapping[str, Any]], label: str
) -> list[tuple[str, str]]:
    identities: list[tuple[str, str]] = []
    for row in rows:
        if row.get("discovery_source_kind") != DISCOVERY_SOURCE_KIND:
            raise ValueError(f"{label} row is not a structured_text_test")
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
    identities: list[tuple[str, str]] = []
    for row in rows:
        if row.get("discovery_source_kind") != DISCOVERY_SOURCE_KIND:
            raise ValueError("mapped catalog row is not a structured_text_test")
        identities.append(
            (
                _required_string(row, "discovery_id", "mapped catalog"),
                _required_string(row, "name", "mapped catalog"),
            )
        )
    if len(identities) != len(set(identities)):
        raise ValueError("mapped catalog contains duplicate identities")
    return identities


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _table_array(
    document: Mapping[str, Any], field: str, label: str
) -> list[Mapping[str, Any]]:
    value = document.get(field)
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(row, Mapping) for row in value)
    ):
        raise ValueError(f"{label} {field} is not an array of tables")
    return list(value)


def _required_string(
    record: Mapping[str, Any], field: str, label: str
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _nonnegative_integer(value: Any, label: str) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < 0
    ):
        raise ValueError(f"{label} must be a non-negative integer")
    return value


def _positive_integer(value: Any, label: str) -> int:
    result = _nonnegative_integer(value, label)
    if result == 0:
        raise ValueError(f"{label} must be positive")
    return result


def _validate_source_path(path: str) -> None:
    candidate = Path(path)
    if (
        not path
        or candidate.is_absolute()
        or candidate.suffix.lower() != ".st"
        or ".." in candidate.parts
    ):
        raise ValueError("frozen ST path must be a repository-relative .st file")


def _checked_repo_path(root: Path, value: Path, label: str) -> Path:
    path = value if value.is_absolute() else root / value
    resolved = path.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} must stay inside the repository") from error
    return resolved


def _repo_relative(root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(root).as_posix()
    except ValueError as error:
        raise ValueError("evidence path must stay inside the repository") from error


def _digest_file(path: Path) -> str:
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"provenance input is not a regular file: {path}")
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _git_head(root: Path) -> str:
    head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if len(head) != 40 or any(
        character not in "0123456789abcdef" for character in head
    ):
        raise ValueError("repository lacks a full Git HEAD")
    return head


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        raise ValueError(f"output already exists: {path}")
    encoded = (json.dumps(document, indent=2, ensure_ascii=False) + "\n").encode()
    with tempfile.NamedTemporaryFile(
        dir=path.parent, prefix=f".{path.name}.", delete=False
    ) as temporary:
        temporary.write(encoded)
        temporary_path = Path(temporary.name)
    try:
        temporary_path.replace(path)
    except BaseException:
        temporary_path.unlink(missing_ok=True)
        raise


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--path", required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--test-prefix", required=True)
    parser.add_argument("--expected-reviewed-nonmapping", type=int, required=True)
    parser.add_argument("--expected-catalog-mapped", type=int, required=True)
    parser.add_argument("--raw-results", type=Path, required=True)
    parser.add_argument("--results", type=Path, required=True)
    command = parser.add_mutually_exclusive_group(required=True)
    command.add_argument("--trust-dev-binary", type=Path)
    command.add_argument("--cargo-command")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        source_path = _checked_repo_path(root, Path(args.path), "ST source")
        project = _checked_repo_path(root, args.project, "ST project")
        if not source_path.is_file() or source_path.is_symlink():
            raise ValueError("ST source is not a regular file")
        if not project.is_dir() or project.is_symlink():
            raise ValueError("ST project is not a regular directory")
        source_path.relative_to(project)
        project_path = project.relative_to(root).as_posix()
        raw_path = _checked_repo_path(root, args.raw_results, "raw results")
        results_path = _checked_repo_path(root, args.results, "results")
        if raw_path == results_path:
            raise ValueError("raw results and normalized results must differ")
        binary = args.trust_dev_binary
        if binary is not None:
            binary = _checked_repo_path(root, binary, "trust-dev binary")
        selection = derive_frozen_selection(
            root=root,
            path=source_path.relative_to(root).as_posix(),
            expected_reviewed_nonmapping=args.expected_reviewed_nonmapping,
            expected_catalog_mapped=args.expected_catalog_mapped,
        )
        if any(
            not name.startswith(args.test_prefix)
            for name in selection.names
        ):
            raise ValueError("selected ST names do not share the test prefix")
        command = build_test_command(
            project_path=project_path,
            test_prefix=args.test_prefix,
            trust_dev_binary=binary,
            cargo_command=args.cargo_command,
        )
        document, exit_code = execute_batch(
            root=root,
            selection=selection,
            project_path=project_path,
            test_prefix=args.test_prefix,
            command=command,
            raw_path=raw_path,
        )
        _write_json_atomic(results_path, document)
        if exit_code != 0:
            print(
                f"frozen ST batch recorded {len(document['failures'])} failures"
            )
            return 2
        print(f"frozen ST batch recorded {len(selection.names)} passing tests")
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"frozen ST batch failed: {error}")
        return 2
