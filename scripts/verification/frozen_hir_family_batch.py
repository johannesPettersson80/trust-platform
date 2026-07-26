"""Execute the frozen remaining trust-hir catalogue family once."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import subprocess
import tempfile
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .coverage_trace_filter import exact_name_matcher
from .frozen_rust_nextest_batch import normalize_nextest_jsonl
from .frozen_rust_nextest_target import (
    HIR_REMAINING_INTEGRATION_PATHS,
    HIR_REMAINING_PROFILE_KEY,
    HIR_REMAINING_UNIT_PREFIXES,
    MESSAGE_FORMAT_VERSION,
    FrozenRustTargetProfile,
    get_target_profile,
)
from .mapping_assertions import validate_assertion_batch


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
MODULE_PATH = Path("scripts/verification/frozen_hir_family_batch.py")
SHIM_PATH = Path("scripts/run_frozen_hir_family_batch.py")
EXPERIMENTAL_ENVIRONMENT = {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"}
TERMINAL_EVENTS = frozenset({"ok", "failed", "ignored", "timeout"})
MAPPING_OUTPUT_PATHS = frozenset(
    {
        "verification/test-catalog.toml",
        "verification/test-catalog-denominator.toml",
        "verification/test-catalog-proposals.toml",
        "verification/test-catalog-mapping-decisions.toml",
    }
)


@dataclass(frozen=True)
class HirFamilyTest:
    discovery_id: str
    path: str
    name: str
    disposition: str
    profile: FrozenRustTargetProfile

    @property
    def full_name(self) -> str:
        return self.profile.test_prefix + self.name

    @property
    def encoded_name(self) -> str:
        return self.profile.encoded_test_name_prefix + self.full_name


@dataclass(frozen=True)
class HirFamilySelection:
    tests: tuple[HirFamilyTest, ...]

    @property
    def reviewed_nonmapping_count(self) -> int:
        return sum(
            test.disposition == "reviewed_nonmapping" for test in self.tests
        )

    @property
    def catalog_mapped_count(self) -> int:
        return sum(test.disposition == "catalog_mapped" for test in self.tests)

    @property
    def paths(self) -> tuple[str, ...]:
        return tuple(sorted({test.path for test in self.tests}))


def hir_remaining_paths() -> frozenset[str]:
    return frozenset(
        {*HIR_REMAINING_INTEGRATION_PATHS, *HIR_REMAINING_UNIT_PREFIXES}
    )


def derive_hir_family_selection(root: Path) -> HirFamilySelection:
    """Freeze active denominator identities for the closed HIR path family."""

    denominator = _load_toml(root / DENOMINATOR_PATH)
    catalog = _load_toml(root / CATALOG_PATH)
    return _derive_hir_family_selection(
        denominator=denominator,
        catalog=catalog,
    )


def _derive_hir_family_selection(
    *,
    denominator: Mapping[str, Any],
    catalog: Mapping[str, Any],
) -> HirFamilySelection:
    paths = hir_remaining_paths()
    remaining_hir_rows = [
        row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and str(row.get("path", "")).startswith("crates/trust-hir/")
    ]
    outside = sorted(
        {
            _required_string(row, "path", "HIR denominator row")
            for row in remaining_hir_rows
            if row.get("path") not in paths
        }
    )
    if outside:
        raise ValueError(
            "reviewed HIR rows exist outside the closed HIR family: "
            + ", ".join(outside)
        )

    selected_rows = [
        row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") in paths
        and row.get("disposition")
        in {"reviewed_nonmapping", "catalog_mapped"}
    ]
    if not selected_rows:
        raise ValueError("closed HIR family selected no denominator rows")
    catalog_by_discovery = {
        _required_string(row, "discovery_id", "catalog row"): row
        for row in catalog.get("tests", [])
        if isinstance(row, Mapping) and row.get("path") in paths
    }
    tests: list[HirFamilyTest] = []
    seen_ids: set[str] = set()
    seen_encoded: set[str] = set()
    for row in selected_rows:
        discovery_id = _required_string(
            row, "discovery_id", "HIR denominator row"
        )
        path = _required_string(row, "path", discovery_id)
        name = _required_string(row, "name", discovery_id)
        disposition = _required_string(row, "disposition", discovery_id)
        if row.get("ignore_state") != "not_ignored":
            raise ValueError(f"{discovery_id} is not an active HIR test")
        profile = get_target_profile(HIR_REMAINING_PROFILE_KEY, path=path)
        if row.get("discovery_source_kind") != profile.discovery_source_kind:
            raise ValueError(f"{discovery_id} source kind does not match target")
        mapped = catalog_by_discovery.get(discovery_id)
        if disposition == "catalog_mapped":
            if (
                mapped is None
                or mapped.get("status") != "mapped"
                or mapped.get("path") != path
                or mapped.get("name") != name
            ):
                raise ValueError(
                    f"{discovery_id} mapped denominator/catalog identity drifted"
                )
        elif mapped is not None:
            raise ValueError(
                f"{discovery_id} reviewed row already exists in the catalog"
            )
        test = HirFamilyTest(
            discovery_id=discovery_id,
            path=path,
            name=name,
            disposition=disposition,
            profile=profile,
        )
        if discovery_id in seen_ids:
            raise ValueError(f"duplicate HIR discovery ID {discovery_id}")
        if test.encoded_name in seen_encoded:
            raise ValueError(
                f"duplicate HIR execution identity {test.encoded_name}"
            )
        seen_ids.add(discovery_id)
        seen_encoded.add(test.encoded_name)
        tests.append(test)
    return HirFamilySelection(
        tests=tuple(
            sorted(
                tests,
                key=lambda test: (
                    test.profile.binary_id,
                    test.full_name,
                    test.discovery_id,
                ),
            )
        )
    )


def build_hir_family_command(
    selection: HirFamilySelection,
) -> list[str]:
    """Build one exact nextest command over the closed HIR target set."""

    if not selection.tests:
        raise ValueError("HIR family command requires selected tests")
    target_names = sorted(
        {
            test.profile.target_name
            for test in selection.tests
            if test.profile.target_kind == "test"
            and test.profile.target_name is not None
        }
    )
    include_lib = any(
        test.profile.target_kind == "lib" for test in selection.tests
    )
    command = ["cargo", "nextest", "run", "-p", "trust-hir"]
    if include_lib:
        command.append("--lib")
    for target_name in target_names:
        command.extend(["--test", target_name])
    clauses = [
        (
            f"(binary_id({exact_name_matcher(test.profile.binary_id)}) & "
            f"test({exact_name_matcher(test.full_name)}))"
        )
        for test in selection.tests
    ]
    command.extend(
        [
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
    )
    return command


def project_raw_results(
    raw: str,
    selection: HirFamilySelection,
) -> dict[str, str]:
    """Project one exact combined JSONL run into path-bound raw evidence."""

    path_by_encoded = {
        test.encoded_name: test.path for test in selection.tests
    }
    projected: dict[str, list[str]] = defaultdict(list)
    seen_started: set[str] = set()
    seen_terminal: set[str] = set()
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(
                f"combined raw line {line_number} is not JSON: {error.msg}"
            ) from error
        if not isinstance(event, Mapping) or event.get("type") != "test":
            continue
        encoded_name = event.get("name")
        event_name = event.get("event")
        if not isinstance(encoded_name, str) or encoded_name not in path_by_encoded:
            raise ValueError(
                f"combined raw contains unexpected HIR test {encoded_name!r}"
            )
        if event_name == "started":
            if encoded_name in seen_started:
                raise ValueError(f"duplicate started event for {encoded_name}")
            seen_started.add(encoded_name)
        elif event_name in TERMINAL_EVENTS:
            if encoded_name in seen_terminal:
                raise ValueError(f"duplicate terminal event for {encoded_name}")
            if event_name != "ok":
                raise ValueError(
                    f"HIR family test {encoded_name} did not pass: {event_name}"
                )
            seen_terminal.add(encoded_name)
        else:
            raise ValueError(
                f"combined raw contains unknown event {event_name!r}"
            )
        projected[path_by_encoded[encoded_name]].append(line)
    expected = set(path_by_encoded)
    if seen_started != expected:
        raise ValueError("combined raw started identities do not match selection")
    if seen_terminal != expected:
        raise ValueError("combined raw terminal identities do not match selection")
    return {
        path: "\n".join(lines) + "\n"
        for path, lines in sorted(projected.items())
    }


def build_projected_documents(
    *,
    root: Path,
    selection: HirFamilySelection,
    raw_path: Path,
    metadata_path: Path,
    results_dir: Path,
    command: Sequence[str],
    nextest_version: str,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    """Write path projections and return combined metadata plus results."""

    raw = raw_path.read_text()
    projections = project_raw_results(raw, selection)
    tests_by_path: dict[str, list[HirFamilyTest]] = defaultdict(list)
    for test in selection.tests:
        tests_by_path[test.path].append(test)
    path_records = []
    result_documents: dict[str, dict[str, Any]] = {}
    results_dir.mkdir(parents=True, exist_ok=True)
    for path in selection.paths:
        tests = sorted(tests_by_path[path], key=lambda test: test.full_name)
        profile = tests[0].profile
        key = path_key(path)
        projected_path = results_dir / f"{key}.jsonl"
        _write_text_atomic(projected_path, projections[path])
        normalized = normalize_nextest_jsonl(
            projections[path],
            expected_full_names={test.full_name for test in tests},
            profile=profile,
        )
        path_records.append(
            {
                "path": path,
                "profile": HIR_REMAINING_PROFILE_KEY,
                "target_kind": profile.target_kind,
                "target_name": profile.target_name,
                "binary_id": profile.binary_id,
                "test_prefix": profile.test_prefix,
                "reviewed_nonmapping_count": sum(
                    test.disposition == "reviewed_nonmapping" for test in tests
                ),
                "catalog_mapped_count": sum(
                    test.disposition == "catalog_mapped" for test in tests
                ),
                "selected_names_sha256": _digest_document(
                    {"names": sorted(test.full_name for test in tests)}
                ),
                "source_sha256": _digest_file(root / path),
                "projected_raw_results_path": _repo_relative(
                    root, projected_path
                ),
                "projected_raw_results_sha256": _digest_file(projected_path),
            }
        )
        result_documents[path] = {
            "schema_version": 1,
            "tests": normalized,
            "failures": [],
            "pending": [],
            "stats": {
                "tests": len(normalized),
                "passes": len(normalized),
                "failures": 0,
                "pending": 0,
            },
        }

    metadata = {
        "schema_version": 1,
        "kind": "rust_nextest_jsonl_family",
        "source_commit": _git_head(root),
        "command": shlex.join(command),
        "argv": list(command),
        "environment": dict(EXPERIMENTAL_ENVIRONMENT),
        "nextest_version": nextest_version,
        "package": "trust-hir",
        "reviewed_nonmapping_count": selection.reviewed_nonmapping_count,
        "catalog_mapped_count": selection.catalog_mapped_count,
        "selected_test_count": len(selection.tests),
        "selection_sha256": hir_family_selection_digest(selection),
        "denominator_sha256": _digest_file(root / DENOMINATOR_PATH),
        "catalog_sha256": _digest_file(root / CATALOG_PATH),
        "module_path": MODULE_PATH.as_posix(),
        "module_sha256": _digest_file(root / MODULE_PATH),
        "shim_path": SHIM_PATH.as_posix(),
        "shim_sha256": _digest_file(root / SHIM_PATH),
        "raw_results_path": _repo_relative(root, raw_path),
        "raw_results_sha256": _digest_file(raw_path),
        "paths": path_records,
    }
    _write_json_atomic(metadata_path, metadata)
    metadata_digest = _digest_file(metadata_path)
    for record in path_records:
        path = str(record["path"])
        result = result_documents[path]
        result["execution"] = {
            "kind": "rust_nextest_jsonl_projection",
            "batch_command": metadata["command"],
            "runner_path": SHIM_PATH.as_posix(),
            "runner_sha256": metadata["shim_sha256"],
            "source_path": path,
            "source_sha256": record["source_sha256"],
            "raw_results_path": record["projected_raw_results_path"],
            "raw_results_sha256": record["projected_raw_results_sha256"],
            "combined_raw_results_path": metadata["raw_results_path"],
            "combined_raw_results_sha256": metadata["raw_results_sha256"],
            "execution_metadata_path": _repo_relative(root, metadata_path),
            "execution_metadata_sha256": metadata_digest,
            "nextest_version": nextest_version,
            "package": "trust-hir",
            "target_kind": record["target_kind"],
            "binary_id": record["binary_id"],
            "test_prefix": record["test_prefix"],
        }
        results_path = results_dir / f"{path_key(path)}-results.json"
        _write_json_atomic(results_path, result)
    return metadata, result_documents


def validate_projection_execution(
    *,
    root: Path,
    plan: Mapping[str, Any],
    execution: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> None:
    """Validate a path projection against its immutable combined execution."""

    expected_fields = {
        "kind",
        "batch_command",
        "runner_path",
        "runner_sha256",
        "source_path",
        "source_sha256",
        "raw_results_path",
        "raw_results_sha256",
        "combined_raw_results_path",
        "combined_raw_results_sha256",
        "execution_metadata_path",
        "execution_metadata_sha256",
        "nextest_version",
        "package",
        "target_kind",
        "binary_id",
        "test_prefix",
    }
    if set(execution) != expected_fields:
        raise ValueError("HIR projection lacks closed execution provenance")
    profile = get_target_profile(
        HIR_REMAINING_PROFILE_KEY,
        path=_required_string(plan, "path", "plan"),
    )
    expected = {
        "kind": "rust_nextest_jsonl_projection",
        "batch_command": _required_string(plan, "command", "plan"),
        "runner_path": _required_string(plan, "runner_path", "plan"),
        "source_path": _required_string(plan, "path", "plan"),
        "package": profile.package,
        "target_kind": profile.target_kind,
        "binary_id": profile.binary_id,
        "test_prefix": profile.test_prefix,
    }
    for field, value in expected.items():
        if execution.get(field) != value:
            raise ValueError(f"HIR projection {field} does not match plan")
    for path_field, digest_field in (
        ("runner_path", "runner_sha256"),
        ("source_path", "source_sha256"),
        ("raw_results_path", "raw_results_sha256"),
        ("combined_raw_results_path", "combined_raw_results_sha256"),
        ("execution_metadata_path", "execution_metadata_sha256"),
    ):
        path = root / _required_string(execution, path_field, "execution")
        if (
            not path.is_file()
            or path.is_symlink()
            or execution.get(digest_field) != _digest_file(path)
        ):
            raise ValueError(f"HIR projection {digest_field} is stale")
    metadata = json.loads(
        (root / str(execution["execution_metadata_path"])).read_text()
    )
    if not isinstance(metadata, Mapping):
        raise ValueError("HIR family metadata is not an object")
    source_commit = metadata.get("source_commit")
    if source_commit != _git_head(root):
        raise ValueError("HIR family execution commit does not match HEAD")
    evidence_paths = {
        str(execution[field])
        for field in (
            "raw_results_path",
            "combined_raw_results_path",
            "execution_metadata_path",
        )
    }
    _validate_post_execution_worktree(
        root,
        allowed_evidence_paths=evidence_paths,
    )
    committed_denominator = _git_file_at_commit(
        root,
        str(source_commit),
        DENOMINATOR_PATH.as_posix(),
    )
    committed_catalog = _git_file_at_commit(
        root,
        str(source_commit),
        CATALOG_PATH.as_posix(),
    )
    committed_selection = _derive_hir_family_selection(
        denominator=tomllib.loads(committed_denominator.decode()),
        catalog=tomllib.loads(committed_catalog.decode()),
    )
    expected_metadata = {
        "denominator_sha256": _digest_bytes(committed_denominator),
        "catalog_sha256": _digest_bytes(committed_catalog),
        "selection_sha256": hir_family_selection_digest(committed_selection),
        "reviewed_nonmapping_count": (
            committed_selection.reviewed_nonmapping_count
        ),
        "catalog_mapped_count": committed_selection.catalog_mapped_count,
        "selected_test_count": len(committed_selection.tests),
    }
    for field, value in expected_metadata.items():
        if metadata.get(field) != value:
            raise ValueError(f"HIR family metadata {field} is stale")
    for path_field, digest_field in (
        ("module_path", "module_sha256"),
        ("shim_path", "shim_sha256"),
    ):
        committed = _git_file_at_commit(
            root,
            str(source_commit),
            _required_string(metadata, path_field, "metadata"),
        )
        if metadata.get(digest_field) != _digest_bytes(committed):
            raise ValueError(f"HIR family metadata {digest_field} is stale")
    if metadata.get("command") != execution["batch_command"]:
        raise ValueError("HIR family command does not match projection")
    if metadata.get("raw_results_path") != execution["combined_raw_results_path"]:
        raise ValueError("HIR family raw path does not match projection")
    if metadata.get("raw_results_sha256") != execution[
        "combined_raw_results_sha256"
    ]:
        raise ValueError("HIR family raw digest does not match projection")
    path = str(execution["source_path"])
    records = [
        record
        for record in metadata.get("paths", [])
        if isinstance(record, Mapping) and record.get("path") == path
    ]
    if len(records) != 1:
        raise ValueError("HIR family metadata lacks one path projection")
    record = records[0]
    for field in (
        "source_sha256",
        "projected_raw_results_path",
        "projected_raw_results_sha256",
        "binary_id",
        "test_prefix",
        "target_kind",
    ):
        execution_field = {
            "projected_raw_results_path": "raw_results_path",
            "projected_raw_results_sha256": "raw_results_sha256",
        }.get(field, field)
        if record.get(field) != execution.get(execution_field):
            raise ValueError(f"HIR family path record {field} drifted")
    selection = derive_hir_family_selection(root)
    path_tests = [test for test in selection.tests if test.path == path]
    expected_projection = project_raw_results(
        (root / str(execution["combined_raw_results_path"])).read_text(),
        selection,
    )[path]
    if (root / str(execution["raw_results_path"])).read_text() != expected_projection:
        raise ValueError("HIR projected raw evidence is not an exact projection")
    normalized = normalize_nextest_jsonl(
        expected_projection,
        expected_full_names={test.full_name for test in path_tests},
        profile=profile,
    )
    if normalized != results_document.get("tests"):
        raise ValueError("HIR projected normalized results drifted")


def validate_projection_inventories(
    *,
    root: Path,
    facts: Mapping[str, Mapping[str, Any]],
    inventories: Mapping[str, Any],
) -> None:
    """Revalidate stored assertion inventories against every live source."""

    raw_inventories = []
    for discovery_id, inventory in sorted(inventories.items()):
        if not isinstance(inventory, Mapping):
            raise ValueError(f"{discovery_id} assertion inventory is invalid")
        raw_inventories.append(
            {
                key: value
                for key, value in inventory.items()
                if key != "inventory_digest"
            }
        )
    validated = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": raw_inventories},
        list(facts.values()),
    )
    if validated != dict(inventories):
        raise ValueError("HIR assertion inventories drifted from live sources")


def run_hir_family_batch(
    *,
    root: Path,
    raw_path: Path,
    metadata_path: Path,
    results_dir: Path,
) -> Mapping[str, Any]:
    _require_clean_execution_checkout(root)
    selection = derive_hir_family_selection(root)
    command = build_hir_family_command(selection)
    if raw_path.exists() or metadata_path.exists():
        raise ValueError("HIR family raw or metadata output already exists")
    version = subprocess.run(
        ["cargo", "nextest", "--version"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    environment = dict(os.environ)
    environment.update(EXPERIMENTAL_ENVIRONMENT)
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        env=environment,
    )
    _write_text_atomic(raw_path, completed.stdout)
    if completed.returncode != 0:
        raise ValueError(
            "HIR family nextest execution failed: "
            + (completed.stderr.strip() or f"exit {completed.returncode}")
        )
    metadata, _documents = build_projected_documents(
        root=root,
        selection=selection,
        raw_path=raw_path,
        metadata_path=metadata_path,
        results_dir=results_dir,
        command=command,
        nextest_version=version,
    )
    return metadata


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--raw-results", type=Path, required=True)
    parser.add_argument("--execution-metadata", type=Path, required=True)
    parser.add_argument("--results-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        raw_path = _output_path(root, args.raw_results)
        metadata_path = _output_path(root, args.execution_metadata)
        results_dir = _output_path(root, args.results_dir, directory=True)
        metadata = run_hir_family_batch(
            root=root,
            raw_path=raw_path,
            metadata_path=metadata_path,
            results_dir=results_dir,
        )
        print(
            "frozen HIR family recorded "
            f"{metadata['reviewed_nonmapping_count']} new mappings from "
            f"{metadata['selected_test_count']} passing identities"
        )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"frozen HIR family failed: {error}")
        return 2


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def hir_family_selection_digest(selection: HirFamilySelection) -> str:
    return _digest_document(
        {
            "tests": [
                {
                    "discovery_id": test.discovery_id,
                    "path": test.path,
                    "name": test.name,
                    "disposition": test.disposition,
                    "binary_id": test.profile.binary_id,
                    "full_name": test.full_name,
                }
                for test in selection.tests
            ]
        }
    )


def path_key(path: str) -> str:
    stem = Path(path).stem.replace("_", "-")
    digest = hashlib.sha256(path.encode()).hexdigest()[:12]
    return f"{stem}-{digest}"


def _digest_document(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _digest_file(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _git_head(root: Path) -> str:
    head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if len(head) != 40:
        raise ValueError("repository lacks a full Git HEAD")
    return head


def _git_file_at_commit(root: Path, commit: str, path: str) -> bytes:
    completed = subprocess.run(
        ["git", "show", f"{commit}:{path}"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"frozen commit lacks {path}")
    return completed.stdout


def _dirty_paths(root: Path) -> set[str]:
    tracked = subprocess.run(
        ["git", "diff", "--name-only", "-z", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
    ).stdout
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"],
        cwd=root,
        check=True,
        capture_output=True,
    ).stdout
    return {
        value.decode()
        for value in (tracked + untracked).split(b"\0")
        if value
    }


def _require_clean_execution_checkout(root: Path) -> None:
    dirty = sorted(_dirty_paths(root))
    if dirty:
        raise ValueError(
            "HIR family execution requires a clean frozen commit: "
            + ", ".join(dirty)
        )


def _validate_post_execution_worktree(
    root: Path,
    *,
    allowed_evidence_paths: set[str],
) -> None:
    allowed = MAPPING_OUTPUT_PATHS | allowed_evidence_paths
    disallowed = sorted(
        path
        for path in _dirty_paths(root)
        if path not in allowed
        and not path.startswith("verification/invariants/")
    )
    if disallowed:
        raise ValueError(
            "HIR family evidence is stale against dirty build or authority "
            "inputs: "
            + ", ".join(disallowed)
        )


def _repo_relative(root: Path, path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(root.resolve()).as_posix()
    except ValueError as error:
        raise ValueError(f"output path escapes repository: {path}") from error


def _output_path(
    root: Path,
    value: Path,
    *,
    directory: bool = False,
) -> Path:
    path = value if value.is_absolute() else root / value
    path = path.resolve()
    try:
        path.relative_to(root)
    except ValueError as error:
        raise ValueError(f"output path escapes repository: {value}") from error
    if path.is_symlink():
        raise ValueError(f"output path cannot be a symlink: {value}")
    if directory:
        path.mkdir(parents=True, exist_ok=True)
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
    return path


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    _write_text_atomic(
        path,
        json.dumps(document, indent=2, ensure_ascii=False) + "\n",
    )


def _write_text_atomic(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
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
