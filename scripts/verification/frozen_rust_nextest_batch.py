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
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation, ROUND_CEILING
from pathlib import Path, PurePosixPath
from typing import Any

from .frozen_rust_nextest_target import (
    ADS_SERVER_CFG,
    ADS_WIRE_DISABLED_CFG,
    RUNTIME_CONTROL_PROFILE,
    TARGET_PROFILES,
    TERMINAL_EVENTS,
    FrozenSelection,
    FrozenRustTargetProfile,
    TestShard,
    build_catalog_command as build_profile_catalog_command,
    build_nextest_command as build_profile_nextest_command,
    get_target_profile,
    normalize_event_test_name,
    qualify_test_names,
    selected_names_digest as _selected_names_digest,
    selection_from_combined_evidence,
    split_feature_shards as split_profile_feature_shards,
)


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
RUNNER_PATH = Path("scripts/verification/frozen_rust_nextest_batch.py")
SHIM_PATH = Path("scripts/run_frozen_rust_nextest_batch.py")
PACKAGE = RUNTIME_CONTROL_PROFILE.package
BINARY_ID = RUNTIME_CONTROL_PROFILE.binary_id
TEST_NAME_PREFIX = RUNTIME_CONTROL_PROFILE.test_prefix
ENCODED_TEST_NAME_PREFIX = RUNTIME_CONTROL_PROFILE.encoded_test_name_prefix
DEFAULT_FEATURE_CFG = ADS_SERVER_CFG
EXPECTED_REVIEWED_NONMAPPING = 69
EXPECTED_CATALOG_MAPPED = 2
EXPERIMENTAL_ENVIRONMENT = {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"}
def derive_frozen_selection(
    *,
    root: Path,
    path: str,
    expected_reviewed_nonmapping: int,
    expected_catalog_mapped: int,
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
    nonmapping_reason_code: str | None = None,
    selected_discovery_ids: frozenset[str] | None = None,
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

    eligible_reviewed = [
        row
        for row in path_reviews
        if row.get("disposition") == "reviewed_nonmapping"
        and (
            nonmapping_reason_code is None
            or row.get("reason_code") == nonmapping_reason_code
        )
    ]
    if selected_discovery_ids is not None:
        if not selected_discovery_ids:
            raise ValueError("selected discovery inventory cannot be empty")
        eligible_ids = {
            str(row.get("discovery_id")) for row in eligible_reviewed
        }
        unknown = sorted(selected_discovery_ids - eligible_ids)
        if unknown:
            raise ValueError(
                "selected discovery inventory contains ineligible rows: "
                + ", ".join(unknown)
            )
        reviewed = [
            row
            for row in eligible_reviewed
            if row.get("discovery_id") in selected_discovery_ids
        ]
    else:
        reviewed = eligible_reviewed
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

    reviewed_identities = _review_identities(
        reviewed,
        "reviewed denominator",
        discovery_source_kind=profile.discovery_source_kind,
    )
    mapped_denominator_identities = _review_identities(
        denominator_mapped,
        "mapped denominator",
        discovery_source_kind=profile.discovery_source_kind,
    )
    mapped_catalog_identities = _catalog_identities(
        catalog_mapped,
        discovery_source_kind=profile.discovery_source_kind,
    )
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
        full_names=qualify_test_names(profile, ordered_names),
    )


def derive_frozen_selection_from_evidence(
    *,
    root: Path,
    path: str,
    raw_path: Path,
    metadata: Mapping[str, Any],
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
) -> FrozenSelection:
    """Recover the frozen name set without consulting mutable live ledgers."""

    _validate_catalog_path(path)
    return selection_from_combined_evidence(
        profile=profile,
        path=path,
        raw=raw_path.read_text(),
        raw_results_path=_repo_relative(root, raw_path),
        raw_results_sha256=_digest_file(raw_path),
        source_sha256=_digest_file(root / path),
        metadata=metadata,
    )


def split_feature_shards(
    *,
    source: str,
    expected_full_names: Sequence[str],
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
) -> tuple[TestShard, ...]:
    """Derive exact feature shards under one closed target profile."""

    return split_profile_feature_shards(
        profile=profile,
        source=source,
        expected_full_names=expected_full_names,
    )


def build_nextest_command(
    full_names: Sequence[str],
    *,
    no_default_features: bool = False,
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
) -> list[str]:
    """Build one shell-free exact-name filter for the selected profile."""

    return build_profile_nextest_command(
        profile,
        full_names,
        no_default_features=no_default_features,
    )


def build_catalog_command(
    test_name: str,
    *,
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
    no_default_features: bool = False,
) -> str:
    """Return the canonical exact command for one selected catalog test."""

    return build_profile_catalog_command(
        profile,
        test_name,
        no_default_features=no_default_features,
    )


def normalize_nextest_jsonl(
    raw: str,
    *,
    expected_full_names: set[str],
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
) -> list[dict[str, Any]]:
    """Normalize exact passing libtest JSON events to catalog batch rows."""

    if not expected_full_names:
        raise ValueError("normalizer requires at least one expected test")
    if any(
        not name.startswith(profile.test_prefix)
        for name in expected_full_names
    ):
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
        raw_name = event.get("name")
        event_name = event.get("event")
        if not isinstance(raw_name, str) or not raw_name:
            raise ValueError(f"test event on line {line_number} lacks a name")
        name = normalize_event_test_name(profile, raw_name)
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
            "title": name.removeprefix(profile.test_prefix),
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
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
    reuse_frozen_evidence: bool = False,
) -> dict[str, Any]:
    shards = split_feature_shards(
        source=(root / selection.path).read_text(),
        expected_full_names=selection.full_names,
        profile=profile,
    )
    commands = [
        build_nextest_command(
            shard.full_names,
            no_default_features=shard.no_default_features,
            profile=profile,
        )
        for shard in shards
    ]
    _validate_execution_metadata(
        root=root,
        selection=selection,
        raw_path=raw_path,
        metadata=metadata,
        shards=shards,
        commands=commands,
        profile=profile,
        validate_live_ledgers=not reuse_frozen_evidence,
        require_shard_raws=not reuse_frozen_evidence,
    )
    raw = raw_path.read_text()
    tests = normalize_nextest_jsonl(
        raw,
        expected_full_names=set(selection.full_names),
        profile=profile,
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
            "package": profile.package,
            "target_kind": profile.target_kind,
            "binary_id": profile.binary_id,
            "test_prefix": profile.test_prefix,
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
    reuse_default_raw_path: Path | None = None,
    reuse_default_metadata_path: Path | None = None,
    profile: FrozenRustTargetProfile = RUNTIME_CONTROL_PROFILE,
) -> Mapping[str, Any]:
    """Execute exact source-derived shards, optionally reusing a default shard."""

    if raw_path.exists() or metadata_path.exists():
        raise ValueError(
            "raw results or execution metadata already exist; use "
            "--normalize-only to avoid rerunning tests"
        )
    raw_path.parent.mkdir(parents=True, exist_ok=True)
    metadata_path.parent.mkdir(parents=True, exist_ok=True)
    shards = split_feature_shards(
        source=(root / selection.path).read_text(),
        expected_full_names=selection.full_names,
        profile=profile,
    )
    commands = [
        build_nextest_command(
            shard.full_names,
            no_default_features=shard.no_default_features,
            profile=profile,
        )
        for shard in shards
    ]
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
    shard_metadata: list[dict[str, Any]] = []
    shard_paths: list[Path] = []
    reused = (reuse_default_raw_path, reuse_default_metadata_path)
    if (reused[0] is None) != (reused[1] is None):
        raise ValueError(
            "reused default raw results and metadata must be provided together"
        )
    for index, (shard, command) in enumerate(
        zip(shards, commands, strict=True)
    ):
        shard_path = raw_path.with_name(
            f"{raw_path.stem}.{shard.name}{raw_path.suffix}"
        )
        if shard_path.exists():
            raise ValueError(f"shard raw results already exist: {shard_path}")
        if index == 0 and reused[0] is not None:
            if shard.name != "default":
                raise ValueError("only the default feature shard can be reused")
            legacy_metadata = json.loads(reused[1].read_text())
            if not isinstance(legacy_metadata, Mapping):
                raise ValueError("reused execution metadata is not an object")
            _validate_legacy_execution_metadata(
                root=root,
                selection=selection,
                raw_path=reused[0],
                metadata=legacy_metadata,
                command=build_nextest_command(
                    selection.full_names,
                    profile=profile,
                ),
                profile=profile,
            )
            normalize_nextest_jsonl(
                reused[0].read_text(),
                expected_full_names=set(shard.full_names),
                profile=profile,
            )
            shard_path.write_bytes(reused[0].read_bytes())
            shard_record = {
                "name": shard.name,
                "command": legacy_metadata["command"],
                "argv": legacy_metadata["argv"],
                "selected_test_count": len(shard.full_names),
                "selected_names_sha256": _selected_names_digest(
                    shard.full_names
                ),
                "raw_results_path": _repo_relative(root, shard_path),
                "raw_results_sha256": _digest_file(shard_path),
                "exit_code": 0,
                "started_at": legacy_metadata["started_at"],
                "finished_at": legacy_metadata["finished_at"],
                "reused_from": _repo_relative(root, reused[0]),
                "reused_from_sha256": _digest_file(reused[0]),
            }
        else:
            shard_started_at = _utc_now()
            try:
                with shard_path.open("xb") as raw_file:
                    completed = subprocess.run(
                        command,
                        cwd=root,
                        env=environment,
                        stdout=raw_file,
                        stderr=subprocess.PIPE,
                    )
            except OSError:
                if shard_path.exists() and shard_path.stat().st_size == 0:
                    shard_path.unlink()
                raise
            shard_finished_at = _utc_now()
            if completed.stderr:
                sys.stderr.buffer.write(completed.stderr)
            shard_record = {
                "name": shard.name,
                "command": shlex.join(command),
                "argv": command,
                "selected_test_count": len(shard.full_names),
                "selected_names_sha256": _selected_names_digest(
                    shard.full_names
                ),
                "raw_results_path": _repo_relative(root, shard_path),
                "raw_results_sha256": _digest_file(shard_path),
                "exit_code": completed.returncode,
                "started_at": shard_started_at,
                "finished_at": shard_finished_at,
            }
        shard_paths.append(shard_path)
        shard_metadata.append(shard_record)

    with raw_path.open("xb") as combined:
        for shard_path in shard_paths:
            content = shard_path.read_bytes()
            combined.write(content)
            if content and not content.endswith(b"\n"):
                combined.write(b"\n")
    finished_at = _utc_now()
    exit_code = max(int(record["exit_code"]) for record in shard_metadata)
    metadata = {
        "schema_version": 2,
        "command": " && ".join(
            str(record["command"]) for record in shard_metadata
        ),
        "argv": [record["argv"] for record in shard_metadata],
        "environment": dict(EXPERIMENTAL_ENVIRONMENT),
        "nextest_version": version,
        "exit_code": exit_code,
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
        "shards": shard_metadata,
    }
    _write_json_atomic(metadata_path, metadata)
    return metadata


def _validate_execution_metadata(
    *,
    root: Path,
    selection: FrozenSelection,
    raw_path: Path,
    metadata: Mapping[str, Any],
    shards: Sequence[TestShard],
    commands: Sequence[Sequence[str]],
    profile: FrozenRustTargetProfile,
    validate_live_ledgers: bool = True,
    require_shard_raws: bool = True,
) -> None:
    if metadata.get("schema_version") == 1:
        _validate_legacy_execution_metadata(
            root=root,
            selection=selection,
            raw_path=raw_path,
            metadata=metadata,
            command=build_nextest_command(
                selection.full_names,
                profile=profile,
            ),
            profile=profile,
            validate_live_ledgers=validate_live_ledgers,
        )
        return
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
        "shards",
    }
    if set(metadata) != required_fields:
        raise ValueError("execution metadata does not have the closed field set")
    records = metadata.get("shards")
    if (
        not isinstance(records, Sequence)
        or isinstance(records, (str, bytes))
        or len(records) != len(shards)
        or any(not isinstance(record, Mapping) for record in records)
    ):
        raise ValueError("execution metadata shards do not match feature split")
    expected = {
        "schema_version": 2,
        "command": " && ".join(
            str(record.get("command")) for record in records
        ),
        "argv": [record.get("argv") for record in records],
        "environment": dict(EXPERIMENTAL_ENVIRONMENT),
        "exit_code": 0,
        "path": selection.path,
        "reviewed_nonmapping_count": selection.reviewed_nonmapping_count,
        "catalog_mapped_count": selection.catalog_mapped_count,
        "selected_test_count": len(selection.full_names),
        "selected_names_sha256": _selected_names_digest(selection.full_names),
        "source_sha256": _digest_file(root / selection.path),
        "raw_results_path": _repo_relative(root, raw_path),
        "raw_results_sha256": _digest_file(raw_path),
        "module_path": RUNNER_PATH.as_posix(),
        "shim_path": SHIM_PATH.as_posix(),
    }
    if validate_live_ledgers:
        expected.update(
            {
                "denominator_sha256": _digest_file(
                    root / DENOMINATOR_PATH
                ),
                "catalog_sha256": _digest_file(root / CATALOG_PATH),
            }
        )
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
    for field in (
        "module_sha256",
        "shim_sha256",
        "denominator_sha256",
        "catalog_sha256",
    ):
        if not str(metadata[field]).startswith("sha256:"):
            raise ValueError(f"execution metadata {field} is not a SHA-256 digest")
    for shard, command, record in zip(shards, commands, records, strict=True):
        required = {
            "name",
            "command",
            "argv",
            "selected_test_count",
            "selected_names_sha256",
            "raw_results_path",
            "raw_results_sha256",
            "exit_code",
            "started_at",
            "finished_at",
        }
        allowed = required | {"reused_from", "reused_from_sha256"}
        if not required <= set(record) <= allowed:
            raise ValueError("execution metadata shard has an invalid field set")
        shard_path = root / _required_string(
            record, "raw_results_path", "execution shard"
        )
        expected_shard = {
            "name": shard.name,
            "selected_test_count": len(shard.full_names),
            "selected_names_sha256": _selected_names_digest(shard.full_names),
            "exit_code": 0,
        }
        if require_shard_raws:
            expected_shard["raw_results_sha256"] = _digest_file(shard_path)
        elif (
            not isinstance(record.get("raw_results_sha256"), str)
            or not str(record["raw_results_sha256"]).startswith("sha256:")
        ):
            raise ValueError(
                f"execution metadata shard {shard.name} raw digest is invalid"
            )
        if record.get("reused_from") is None:
            expected_shard.update(
                {"command": shlex.join(command), "argv": list(command)}
            )
        for field, value in expected_shard.items():
            if record.get(field) != value:
                raise ValueError(
                    f"execution metadata shard {shard.name} {field} mismatch"
                )
        if require_shard_raws:
            normalize_nextest_jsonl(
                shard_path.read_text(),
                expected_full_names=set(shard.full_names),
                profile=profile,
            )
        if require_shard_raws and record.get("reused_from") is not None:
            reused_path = root / _required_string(
                record, "reused_from", "execution shard"
            )
            if record.get("reused_from_sha256") != _digest_file(reused_path):
                raise ValueError("reused shard digest does not match source")


def _validate_legacy_execution_metadata(
    *,
    root: Path,
    selection: FrozenSelection,
    raw_path: Path,
    metadata: Mapping[str, Any],
    command: Sequence[str],
    profile: FrozenRustTargetProfile,
    validate_live_ledgers: bool = True,
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
        raise ValueError(
            "legacy execution metadata does not have the closed field set"
        )
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
        "raw_results_path": _repo_relative(root, raw_path),
        "raw_results_sha256": _digest_file(raw_path),
        "module_path": RUNNER_PATH.as_posix(),
        "shim_path": SHIM_PATH.as_posix(),
    }
    if validate_live_ledgers:
        expected.update(
            {
                "denominator_sha256": _digest_file(
                    root / DENOMINATOR_PATH
                ),
                "catalog_sha256": _digest_file(root / CATALOG_PATH),
            }
        )
    for field, value in expected.items():
        if metadata.get(field) != value:
            raise ValueError(
                f"legacy execution metadata {field} does not match "
                "the frozen batch"
            )
    for field in (
        "nextest_version",
        "started_at",
        "finished_at",
        "module_sha256",
        "shim_sha256",
    ):
        if not isinstance(metadata.get(field), str) or not metadata[field]:
            raise ValueError(
                f"legacy execution metadata requires non-empty {field}"
            )
    for field in (
        "module_sha256",
        "shim_sha256",
        "denominator_sha256",
        "catalog_sha256",
    ):
        if not str(metadata[field]).startswith("sha256:"):
            raise ValueError(
                f"legacy execution metadata {field} is not a SHA-256 digest"
            )


def _review_identities(
    rows: Sequence[Mapping[str, Any]],
    label: str,
    *,
    discovery_source_kind: str,
) -> list[tuple[str, str]]:
    identities: list[tuple[str, str]] = []
    for row in rows:
        if row.get("discovery_source_kind") != discovery_source_kind:
            raise ValueError(
                f"{label} row is not a {discovery_source_kind}"
            )
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
    *,
    discovery_source_kind: str,
) -> list[tuple[str, str]]:
    identities = []
    for row in rows:
        if row.get("discovery_source_kind") != discovery_source_kind:
            raise ValueError(
                "mapped catalog row is not a "
                f"{discovery_source_kind}"
            )
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
    parser.add_argument(
        "--profile",
        choices=sorted(TARGET_PROFILES),
        default=RUNTIME_CONTROL_PROFILE.key,
    )
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
    parser.add_argument(
        "--nonmapping-reason-code",
        help="select only reviewed_nonmapping rows with this exact reason code",
    )
    parser.add_argument(
        "--selection-inventory",
        type=Path,
        help=(
            "select reviewed_nonmapping discovery IDs from one saved assertion "
            "inventory while retaining mapped companions"
        ),
    )
    parser.add_argument("--raw-results", type=Path, required=True)
    parser.add_argument("--execution-metadata", type=Path)
    parser.add_argument(
        "--reuse-default-raw-results",
        type=Path,
        help="reuse a previously completed default-feature raw JSONL shard",
    )
    parser.add_argument(
        "--reuse-default-execution-metadata",
        type=Path,
        help="execution metadata for --reuse-default-raw-results",
    )
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
        profile = get_target_profile(args.profile, path=args.path)
        if not root.is_dir():
            raise ValueError("repository root does not exist")
        _checked_repo_file(root, DENOMINATOR_PATH, "test denominator")
        _checked_repo_file(root, CATALOG_PATH, "test catalog")
        _validate_catalog_path(args.path)
        _checked_repo_file(root, Path(args.path), "test source")
        selected_discovery_ids = None
        if args.selection_inventory is not None:
            selection_inventory_path = _checked_repo_file(
                root,
                args.selection_inventory,
                "selection inventory",
            )
            selection_inventory = json.loads(
                selection_inventory_path.read_text()
            )
            if not isinstance(selection_inventory, Mapping):
                raise ValueError("selection inventory must be a JSON object")
            inventories = selection_inventory.get("inventories")
            if not isinstance(inventories, Mapping):
                raise ValueError("selection inventory lacks inventories")
            selected_discovery_ids = frozenset(str(key) for key in inventories)
        raw_path = _checked_output_path(root, args.raw_results, "raw results")
        results_path = _checked_output_path(root, args.results, "results")
        metadata_value = args.execution_metadata or Path(
            str(args.raw_results) + ".execution.json"
        )
        metadata_path = _checked_output_path(
            root, metadata_value, "execution metadata"
        )
        reuse_raw_path = (
            _checked_output_path(
                root,
                args.reuse_default_raw_results,
                "reused default raw results",
            )
            if args.reuse_default_raw_results is not None
            else None
        )
        reuse_metadata_path = (
            _checked_output_path(
                root,
                args.reuse_default_execution_metadata,
                "reused default execution metadata",
            )
            if args.reuse_default_execution_metadata is not None
            else None
        )
        if len({raw_path, metadata_path, results_path}) != 3:
            raise ValueError(
                "raw results, execution metadata, and results paths must differ"
            )
        if args.normalize_only:
            if reuse_raw_path is not None or reuse_metadata_path is not None:
                raise ValueError(
                    "--normalize-only cannot be combined with reused shard inputs"
                )
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
            selection = derive_frozen_selection_from_evidence(
                root=root,
                path=args.path,
                raw_path=raw_path,
                metadata=metadata,
                profile=profile,
            )
        else:
            if (reuse_raw_path is None) != (reuse_metadata_path is None):
                raise ValueError(
                    "both reused default shard arguments are required"
                )
            if reuse_raw_path is not None:
                _checked_repo_file(
                    root,
                    Path(_repo_relative(root, reuse_raw_path)),
                    "reused default raw results",
                )
                _checked_repo_file(
                    root,
                    Path(_repo_relative(root, reuse_metadata_path)),
                    "reused default execution metadata",
                )
            selection = derive_frozen_selection(
                root=root,
                path=args.path,
                expected_reviewed_nonmapping=(
                    args.expected_reviewed_nonmapping
                ),
                expected_catalog_mapped=args.expected_catalog_mapped,
                profile=profile,
                nonmapping_reason_code=args.nonmapping_reason_code,
                selected_discovery_ids=selected_discovery_ids,
            )
            metadata = execute_nextest_batch(
                root=root,
                selection=selection,
                raw_path=raw_path,
                metadata_path=metadata_path,
                reuse_default_raw_path=reuse_raw_path,
                reuse_default_metadata_path=reuse_metadata_path,
                profile=profile,
            )
        document = build_normalized_document(
            root=root,
            selection=selection,
            raw_path=raw_path,
            metadata=metadata,
            profile=profile,
            reuse_frozen_evidence=args.normalize_only,
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
