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

    expected = {
        _required_string(record, "discovery_id", "resolved test"): record
        for record in resolved
    }
    completed_ids: set[str] = set()
    attempts_root = trace_root / "attempts"
    if not attempts_root.is_dir():
        return completed_ids
    for attempt_dir in sorted(path for path in attempts_root.iterdir() if path.is_dir()):
        manifest_path = attempt_dir / "attempt.json"
        try:
            manifest = json.loads(manifest_path.read_text())
        except (OSError, UnicodeError, json.JSONDecodeError):
            manifest = None
        discovery_id = (
            manifest.get("discovery_id")
            if isinstance(manifest, Mapping)
            else None
        )
        record = expected.get(str(discovery_id))
        reusable = (
            record is not None
            and manifest.get("status") == "completed"
            and manifest.get("exit_code") == 0
            and manifest.get("binary_id") == record.get("nextest_binary_id")
            and manifest.get("test_name") == record.get("nextest_test_name")
            and isinstance(manifest.get("test_executable"), str)
            and Path(str(manifest["test_executable"])).is_file()
            and manifest.get("test_executable_digest")
            == _file_digest(Path(str(manifest["test_executable"])))
            and any((attempt_dir / "profiles").glob("*.profraw"))
        )
        if reusable:
            if str(discovery_id) in completed_ids:
                raise ValueError(
                    f"resume trace duplicates completed attempt {discovery_id}"
                )
            completed_ids.add(str(discovery_id))
            continue
        if not purge_incomplete:
            raise ValueError(f"trace contains incomplete attempt {attempt_dir.name}")
        shutil.rmtree(attempt_dir)
    return completed_ids


def _file_digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
