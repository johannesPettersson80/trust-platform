"""Recoverable multi-file transaction for reviewed catalog batches."""

from __future__ import annotations

import base64
import fcntl
import json
import os
import tempfile
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any


JOURNAL_NAME = ".mapping-transaction.json"
LOCK_NAME = ".mapping-transaction.lock"


def apply_mapping_transaction(
    *,
    root: Path,
    replacements: Mapping[str, bytes],
    validate: Callable[[], Sequence[str]],
) -> None:
    """Replace a complete batch or restore every original byte on failure."""

    root = root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    normalized = {
        _relative_path(path): content for path, content in replacements.items()
    }
    if not normalized:
        raise ValueError("mapping transaction requires at least one replacement")
    if any(not isinstance(content, bytes) for content in normalized.values()):
        raise ValueError("mapping transaction replacements must be bytes")
    if JOURNAL_NAME in normalized or LOCK_NAME in normalized:
        raise ValueError("mapping transaction cannot replace its journal or lock")
    with _exclusive_lock(root):
        _recover_unlocked(root)
        entries = []
        for relative in sorted(normalized):
            target = _target(root, relative)
            original_exists = target.is_file()
            if target.exists() and not original_exists:
                raise ValueError(f"mapping transaction target is not a file: {relative}")
            original = target.read_bytes() if original_exists else b""
            entries.append(
                {
                    "path": relative,
                    "original_exists": original_exists,
                    "original_base64": base64.b64encode(original).decode("ascii"),
                }
            )
        journal = {
            "schema_version": 1,
            "state": "replacing",
            "entries": entries,
        }
        _atomic_write(root / JOURNAL_NAME, _json_bytes(journal))
        try:
            for relative, content in sorted(normalized.items()):
                _atomic_write(_target(root, relative), content)
            failures = list(validate())
            if failures:
                raise ValueError("; ".join(sorted(set(failures))))
        except BaseException:
            _restore_entries(root, entries)
            _remove_journal(root)
            raise
        _remove_journal(root)


def recover_mapping_transaction(root: Path) -> bool:
    """Recover an interrupted transaction before accepting any new work."""

    root = root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    with _exclusive_lock(root):
        return _recover_unlocked(root)


def _recover_unlocked(root: Path) -> bool:
    journal_path = root / JOURNAL_NAME
    if not journal_path.exists():
        return False
    try:
        journal = json.loads(journal_path.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("mapping transaction journal is unreadable") from error
    if not isinstance(journal, Mapping):
        raise ValueError("mapping transaction journal root must be an object")
    if set(journal) != {"schema_version", "state", "entries"}:
        raise ValueError("mapping transaction journal fields drift")
    if journal.get("schema_version") != 1 or journal.get("state") != "replacing":
        raise ValueError("mapping transaction journal version or state is invalid")
    entries = journal.get("entries")
    if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
        raise ValueError("mapping transaction journal entries must be an array")
    _restore_entries(root, entries)
    _remove_journal(root)
    return True


def _restore_entries(root: Path, entries: Sequence[Any]) -> None:
    seen: set[str] = set()
    validated: list[tuple[str, bool, bytes]] = []
    for index, entry in enumerate(entries):
        if not isinstance(entry, Mapping) or set(entry) != {
            "path",
            "original_exists",
            "original_base64",
        }:
            raise ValueError(f"mapping transaction journal entry {index} fields drift")
        relative = _relative_path(entry.get("path"))
        if relative in seen:
            raise ValueError(f"mapping transaction journal duplicates {relative}")
        seen.add(relative)
        exists = entry.get("original_exists")
        encoded = entry.get("original_base64")
        if not isinstance(exists, bool) or not isinstance(encoded, str):
            raise ValueError(f"mapping transaction journal entry {index} is malformed")
        try:
            original = base64.b64decode(encoded, validate=True)
        except ValueError as error:
            raise ValueError(
                f"mapping transaction journal entry {index} has invalid base64"
            ) from error
        if not exists and original:
            raise ValueError(
                f"mapping transaction journal absent entry {index} contains bytes"
            )
        validated.append((relative, exists, original))
    for relative, exists, original in validated:
        target = _target(root, relative)
        if exists:
            _atomic_write(target, original)
        elif target.exists():
            if not target.is_file():
                raise ValueError(f"recovery target is not a file: {relative}")
            target.unlink()
            _fsync_directory(target.parent)


def _atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".staged",
        dir=path.parent,
    )
    temporary_path = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
        _fsync_directory(path.parent)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def _remove_journal(root: Path) -> None:
    journal = root / JOURNAL_NAME
    if journal.exists():
        journal.unlink()
        _fsync_directory(root)


def _exclusive_lock(root: Path):
    class _Lock:
        def __enter__(self) -> None:
            self.handle = (root / LOCK_NAME).open("a+b")
            try:
                fcntl.flock(self.handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                self.handle.close()
                raise RuntimeError("another mapping transaction holds the lock") from error

        def __exit__(self, *_args: object) -> None:
            fcntl.flock(self.handle.fileno(), fcntl.LOCK_UN)
            self.handle.close()

    return _Lock()


def _relative_path(value: Any) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("mapping transaction path must be repository-relative")
    normalized = value.replace("\\", "/")
    parsed = PurePosixPath(normalized)
    if parsed.is_absolute() or any(part in {"", ".", ".."} for part in parsed.parts):
        raise ValueError(f"mapping transaction path must be repository-relative: {value}")
    return parsed.as_posix()


def _target(root: Path, relative: str) -> Path:
    target = (root / relative).resolve()
    try:
        target.relative_to(root)
    except ValueError as error:
        raise ValueError(
            f"mapping transaction path must be repository-relative: {relative}"
        ) from error
    return target


def _json_bytes(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode()


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
