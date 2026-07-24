"""Exact resumable-attempt validation shared by execution trace adapters."""

from __future__ import annotations

import hashlib
import json
import shutil
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


def reusable_completed_attempt_ids(
    *,
    trace_root: Path,
    resolved: Sequence[Mapping[str, Any]],
    purge_incomplete: bool,
    allow_nonzero_exit: bool = False,
    require_profile_digest: bool = False,
) -> set[str]:
    """Return exact reusable attempts and remove only incomplete generated ones.

    Nonzero executions are reusable only for adapters that explicitly preserve
    them as non-semantic failed trace records.
    """

    expected: dict[str, Mapping[str, Any]] = {}
    for record in resolved:
        discovery_id = _required_string(
            record, "discovery_id", "resolved test"
        )
        if discovery_id in expected:
            raise ValueError(f"resolved tests duplicate {discovery_id}")
        expected[discovery_id] = record
    completed_ids: set[str] = set()
    executable_digests: dict[Path, tuple[str, tuple[int, ...]]] = {}
    attempts_root = trace_root / "attempts"
    if not attempts_root.is_dir():
        return completed_ids
    attempts: list[tuple[Path, Mapping[str, Any] | None]] = []
    observed_discovery_ids: set[str] = set()
    for attempt_dir in sorted(
        path for path in attempts_root.iterdir() if path.is_dir()
    ):
        manifest_path = attempt_dir / "attempt.json"
        try:
            manifest = json.loads(manifest_path.read_text())
        except (OSError, UnicodeError, json.JSONDecodeError):
            manifest = None
        if not isinstance(manifest, Mapping):
            manifest = None
        discovery_id = (
            manifest.get("discovery_id") if manifest is not None else None
        )
        if isinstance(discovery_id, str) and discovery_id:
            if discovery_id in observed_discovery_ids:
                raise ValueError(
                    f"trace duplicates attempt discovery ID {discovery_id}"
                )
            observed_discovery_ids.add(discovery_id)
        attempts.append((attempt_dir, manifest))

    for attempt_dir, manifest in attempts:
        discovery_id = (
            manifest.get("discovery_id")
            if manifest is not None
            else None
        )
        record = expected.get(str(discovery_id))
        executable_value = (
            manifest.get("test_executable")
            if isinstance(manifest, Mapping)
            else None
        )
        manifest_executable = (
            Path(executable_value).resolve()
            if isinstance(executable_value, str)
            else None
        )
        current_executable = (
            Path(
                _required_string(
                    record,
                    "nextest_binary_path",
                    f"resolved test {discovery_id}",
                )
            ).resolve()
            if record is not None
            else None
        )
        executable_digest = None
        executable_signature = None
        if current_executable is not None and current_executable.is_file():
            cached = executable_digests.get(current_executable)
            if cached is None:
                before = _file_signature(current_executable)
                executable_digest = _file_digest(current_executable)
                after = _file_signature(current_executable)
                if before != after:
                    raise ValueError(
                        f"test executable changed during resume validation: "
                        f"{current_executable}"
                    )
                executable_signature = after
                executable_digests[current_executable] = (
                    executable_digest,
                    executable_signature,
                )
            else:
                executable_digest, executable_signature = cached
                if _file_signature(current_executable) != executable_signature:
                    raise ValueError(
                        f"test executable changed during resume validation: "
                        f"{current_executable}"
                    )
        exit_code = (
            manifest.get("exit_code")
            if isinstance(manifest, Mapping)
            else None
        )
        accepted_exit = (
            isinstance(exit_code, int)
            and not isinstance(exit_code, bool)
            and exit_code >= 0
            and (allow_nonzero_exit or exit_code == 0)
        )
        expected_arguments = (
            record.get("test_arguments")
            if isinstance(record, Mapping)
            else None
        )
        arguments_match = (
            expected_arguments is None
            or (
                isinstance(expected_arguments, list)
                and all(isinstance(item, str) for item in expected_arguments)
                and isinstance(manifest, Mapping)
                and manifest.get("test_arguments") == expected_arguments
            )
        )
        profiles_dir = attempt_dir / "profiles"
        raw_profiles = sorted(profiles_dir.glob("*.profraw"))
        recorded_profile_digest = (
            manifest.get("profile_digest")
            if isinstance(manifest, Mapping)
            else None
        )
        must_check_profile_digest = (
            require_profile_digest or recorded_profile_digest is not None
        )
        current_profile_digest = (
            profile_digest(profiles_dir)
            if raw_profiles and must_check_profile_digest
            else None
        )
        profile_digest_matches = (
            not must_check_profile_digest
            or recorded_profile_digest == current_profile_digest
        )
        reusable = (
            isinstance(manifest, Mapping)
            and record is not None
            and manifest.get("status") == "completed"
            and accepted_exit
            and manifest.get("binary_id") == record.get("nextest_binary_id")
            and manifest.get("test_name") == record.get("nextest_test_name")
            and arguments_match
            and manifest_executable == current_executable
            and executable_digest is not None
            and manifest.get("test_executable_digest") == executable_digest
            and bool(raw_profiles)
            and profile_digest_matches
        )
        if reusable:
            if _file_signature(current_executable) != executable_signature:
                raise ValueError(
                    f"test executable changed during resume validation: "
                    f"{current_executable}"
                )
            completed_ids.add(str(discovery_id))
            continue
        if not purge_incomplete:
            raise ValueError(f"trace contains incomplete attempt {attempt_dir.name}")
        shutil.rmtree(attempt_dir)
    return completed_ids


def profile_digest(profiles_dir: Path) -> str:
    """Hash exact profile names and bytes for stable attempt provenance."""

    profiles = sorted(profiles_dir.glob("*.profraw"))
    if not profiles:
        raise ValueError(f"trace attempt has no raw profiles: {profiles_dir}")
    digest = hashlib.sha256()
    for path in profiles:
        digest.update(path.name.encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()


def _file_digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _file_signature(path: Path) -> tuple[int, ...]:
    stat = path.stat()
    return (
        stat.st_dev,
        stat.st_ino,
        stat.st_size,
        stat.st_mtime_ns,
        stat.st_ctime_ns,
    )


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
