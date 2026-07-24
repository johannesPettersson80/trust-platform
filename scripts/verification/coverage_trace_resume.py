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
) -> set[str]:
    """Return exact reusable attempts and remove only incomplete generated ones."""

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
        reusable = (
            record is not None
            and manifest.get("status") == "completed"
            and manifest.get("exit_code") == 0
            and manifest.get("binary_id") == record.get("nextest_binary_id")
            and manifest.get("test_name") == record.get("nextest_test_name")
            and manifest_executable == current_executable
            and executable_digest is not None
            and manifest.get("test_executable_digest") == executable_digest
            and any((attempt_dir / "profiles").glob("*.profraw"))
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
