"""Recoverable, symlink-safe multi-file transaction for mapping batches."""

from __future__ import annotations

import base64
import errno
import fcntl
import json
import os
import secrets
import stat
from collections.abc import Callable, Mapping, Sequence
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterator


JOURNAL_NAME = ".mapping-transaction.json"
LOCK_NAME = ".mapping-transaction.lock"
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW


@dataclass
class _Target:
    relative: str
    parent_fd: int
    name: str

    def close(self) -> None:
        os.close(self.parent_fd)


def apply_mapping_transaction(
    *,
    root: Path,
    replacements: Mapping[str, bytes],
    validate: Callable[[], Sequence[str]],
    expected_originals: Mapping[str, bytes | None] | None = None,
) -> None:
    """Replace a complete batch or restore it without following symlinks."""

    normalized = _normalize_entries(replacements, "replacement")
    if not normalized:
        raise ValueError("mapping transaction requires at least one replacement")
    if any(not isinstance(content, bytes) for content in normalized.values()):
        raise ValueError("mapping transaction replacements must be bytes")
    preconditions = _normalize_entries(
        expected_originals or {}, "precondition"
    )
    if any(
        content is not None and not isinstance(content, bytes)
        for content in preconditions.values()
    ):
        raise ValueError(
            "mapping transaction expected originals must be bytes or None"
        )
    forbidden = {JOURNAL_NAME, LOCK_NAME}
    if forbidden.intersection(normalized):
        raise ValueError("mapping transaction cannot replace its journal or lock")
    if forbidden.intersection(preconditions):
        raise ValueError(
            "mapping transaction cannot precondition its journal or lock"
        )

    root_fd = _open_root(root)
    try:
        with _exclusive_lock(root_fd):
            _recover_unlocked(root_fd)
            with _open_targets(
                root_fd,
                set(normalized) | set(preconditions),
                create_parents=set(normalized),
            ) as targets:
                _validate_target_aliases(targets)
                for relative, expected in sorted(preconditions.items()):
                    target = targets.get(relative)
                    actual = (
                        _read_target(target)
                        if target is not None
                        else None
                    )
                    if actual != expected:
                        raise ValueError(
                            "mapping transaction precondition failed: "
                            f"{relative}"
                        )
                entries = []
                for relative in sorted(normalized):
                    target = targets[relative]
                    if target is None:
                        raise ValueError(
                            f"mapping transaction target parent is missing: "
                            f"{relative}"
                        )
                    original = _read_target(target)
                    entries.append(
                        {
                            "path": relative,
                            "original_exists": original is not None,
                            "original_base64": base64.b64encode(
                                original or b""
                            ).decode("ascii"),
                        }
                    )
                journal = {
                    "schema_version": 1,
                    "state": "replacing",
                    "entries": entries,
                }
                _atomic_write_at(
                    root_fd, JOURNAL_NAME, _json_bytes(journal)
                )
                try:
                    for relative, content in sorted(normalized.items()):
                        target = targets[relative]
                        assert target is not None
                        _atomic_write_target(target, content)
                    failures = list(validate())
                    if failures:
                        raise ValueError(
                            "; ".join(sorted(set(failures)))
                        )
                    _validate_lexical_target_identities(
                        root_fd, targets
                    )
                except BaseException:
                    _restore_open_targets(targets, entries)
                    _remove_at(root_fd, JOURNAL_NAME)
                    raise
                _remove_at(root_fd, JOURNAL_NAME)
    finally:
        os.close(root_fd)


def recover_mapping_transaction(root: Path) -> bool:
    """Recover an interrupted transaction through no-follow handles."""

    root_fd = _open_root(root)
    try:
        with _exclusive_lock(root_fd):
            return _recover_unlocked(root_fd)
    finally:
        os.close(root_fd)


def _recover_unlocked(root_fd: int) -> bool:
    journal_bytes = _read_at(root_fd, JOURNAL_NAME)
    if journal_bytes is None:
        return False
    try:
        journal = json.loads(journal_bytes)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("mapping transaction journal is unreadable") from error
    entries = _validated_journal_entries(journal)
    paths = {relative for relative, _, _ in entries}
    required_parents = {
        relative for relative, original_exists, _ in entries if original_exists
    }
    with _open_targets(
        root_fd,
        paths,
        create_parents=required_parents,
    ) as targets:
        _validate_target_aliases(targets)
        _restore_validated_entries(targets, entries)
        _validate_lexical_target_identities(root_fd, targets)
    _remove_at(root_fd, JOURNAL_NAME)
    return True


def _validated_journal_entries(
    journal: Any,
) -> list[tuple[str, bool, bytes]]:
    if not isinstance(journal, Mapping):
        raise ValueError("mapping transaction journal root must be an object")
    if set(journal) != {"schema_version", "state", "entries"}:
        raise ValueError("mapping transaction journal fields drift")
    if journal.get("schema_version") != 1 or journal.get("state") != "replacing":
        raise ValueError(
            "mapping transaction journal version or state is invalid"
        )
    raw_entries = journal.get("entries")
    if not isinstance(raw_entries, Sequence) or isinstance(
        raw_entries, (str, bytes)
    ):
        raise ValueError("mapping transaction journal entries must be an array")
    seen: set[str] = set()
    entries = []
    for index, entry in enumerate(raw_entries):
        if not isinstance(entry, Mapping) or set(entry) != {
            "path",
            "original_exists",
            "original_base64",
        }:
            raise ValueError(
                f"mapping transaction journal entry {index} fields drift"
            )
        relative = _relative_path(entry.get("path"))
        if relative in seen:
            raise ValueError(
                f"mapping transaction journal duplicates {relative}"
            )
        seen.add(relative)
        exists = entry.get("original_exists")
        encoded = entry.get("original_base64")
        if not isinstance(exists, bool) or not isinstance(encoded, str):
            raise ValueError(
                f"mapping transaction journal entry {index} is malformed"
            )
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
        entries.append((relative, exists, original))
    return entries


def _restore_open_targets(
    targets: Mapping[str, _Target | None],
    entries: Sequence[Mapping[str, Any]],
) -> None:
    validated = _validated_journal_entries(
        {
            "schema_version": 1,
            "state": "replacing",
            "entries": list(entries),
        }
    )
    _restore_validated_entries(targets, validated)


def _restore_validated_entries(
    targets: Mapping[str, _Target | None],
    entries: Sequence[tuple[str, bool, bytes]],
) -> None:
    for relative, exists, original in entries:
        target = targets.get(relative)
        if target is None:
            if exists:
                raise ValueError(
                    f"recovery target parent is missing: {relative}"
                )
            continue
        if exists:
            _atomic_write_target(target, original)
        else:
            _unlink_target(target)


def _open_root(root: Path) -> int:
    root = root.absolute()
    root.mkdir(parents=True, exist_ok=True)
    try:
        return os.open(root, _DIRECTORY_FLAGS)
    except OSError as error:
        raise ValueError(
            "mapping transaction root must be a non-symlink directory"
        ) from error


@contextmanager
def _open_targets(
    root_fd: int,
    relatives: set[str],
    *,
    create_parents: set[str],
) -> Iterator[dict[str, _Target | None]]:
    targets: dict[str, _Target | None] = {}
    try:
        for relative in sorted(relatives):
            targets[relative] = _open_target(
                root_fd,
                relative,
                create_parent=relative in create_parents,
            )
        yield targets
    finally:
        for target in targets.values():
            if target is not None:
                target.close()


def _open_target(
    root_fd: int, relative: str, *, create_parent: bool
) -> _Target | None:
    parts = PurePosixPath(relative).parts
    parent_fd = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            try:
                next_fd = os.open(part, _DIRECTORY_FLAGS, dir_fd=parent_fd)
            except FileNotFoundError:
                if not create_parent:
                    os.close(parent_fd)
                    return None
                try:
                    os.mkdir(part, mode=0o755, dir_fd=parent_fd)
                except FileExistsError:
                    pass
                try:
                    next_fd = os.open(
                        part, _DIRECTORY_FLAGS, dir_fd=parent_fd
                    )
                except OSError as error:
                    raise ValueError(
                        "mapping transaction target contains symlink or "
                        f"non-directory: {relative}"
                    ) from error
            except OSError as error:
                raise ValueError(
                    "mapping transaction target contains symlink or "
                    f"non-directory: {relative}"
                ) from error
            os.close(parent_fd)
            parent_fd = next_fd
        return _Target(relative=relative, parent_fd=parent_fd, name=parts[-1])
    except BaseException:
        try:
            os.close(parent_fd)
        except OSError:
            pass
        raise


def _read_target(target: _Target) -> bytes | None:
    return _read_at(target.parent_fd, target.name, relative=target.relative)


def _read_at(
    parent_fd: int, name: str, *, relative: str | None = None
) -> bytes | None:
    try:
        descriptor = os.open(
            name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=parent_fd
        )
    except FileNotFoundError:
        return None
    except OSError as error:
        raise ValueError(
            f"mapping transaction target is not a regular file: "
            f"{relative or name}"
        ) from error
    try:
        mode = os.fstat(descriptor).st_mode
        if not stat.S_ISREG(mode):
            raise ValueError(
                f"mapping transaction target is not a regular file: "
                f"{relative or name}"
            )
        chunks = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                return b"".join(chunks)
            chunks.append(chunk)
    finally:
        os.close(descriptor)


def _atomic_write_target(target: _Target, content: bytes) -> None:
    _atomic_write_at(
        target.parent_fd, target.name, content, relative=target.relative
    )


def _atomic_write_at(
    parent_fd: int,
    name: str,
    content: bytes,
    *,
    relative: str | None = None,
) -> None:
    temporary = f".{name}.{secrets.token_hex(12)}.staged"
    descriptor = None
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600,
            dir_fd=parent_fd,
        )
        view = memoryview(content)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = None
        os.replace(
            temporary,
            name,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
        )
        os.fsync(parent_fd)
    except OSError as error:
        if error.errno in {errno.ELOOP, errno.ENOTDIR}:
            raise ValueError(
                "mapping transaction target contains symlink or "
                f"non-directory: {relative or name}"
            ) from error
        raise
    finally:
        if descriptor is not None:
            os.close(descriptor)
        try:
            os.unlink(temporary, dir_fd=parent_fd)
        except FileNotFoundError:
            pass


def _unlink_target(target: _Target) -> None:
    try:
        mode = os.stat(
            target.name,
            dir_fd=target.parent_fd,
            follow_symlinks=False,
        ).st_mode
    except FileNotFoundError:
        return
    if not stat.S_ISREG(mode):
        raise ValueError(
            f"recovery target is not a regular file: {target.relative}"
        )
    os.unlink(target.name, dir_fd=target.parent_fd)
    os.fsync(target.parent_fd)


def _remove_at(parent_fd: int, name: str) -> None:
    try:
        mode = os.stat(name, dir_fd=parent_fd, follow_symlinks=False).st_mode
    except FileNotFoundError:
        return
    if not stat.S_ISREG(mode):
        raise ValueError(
            f"mapping transaction internal file is not regular: {name}"
        )
    os.unlink(name, dir_fd=parent_fd)
    os.fsync(parent_fd)


@contextmanager
def _exclusive_lock(root_fd: int) -> Iterator[None]:
    try:
        descriptor = os.open(
            LOCK_NAME,
            os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW,
            0o600,
            dir_fd=root_fd,
        )
    except OSError as error:
        raise ValueError(
            "mapping transaction lock must be a regular non-symlink file"
        ) from error
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise ValueError(
                "mapping transaction lock must be a regular non-symlink file"
            )
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError(
                "another mapping transaction holds the lock"
            ) from error
        yield
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


def _relative_path(value: Any) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("mapping transaction path must be repository-relative")
    normalized = value.replace("\\", "/")
    parsed = PurePosixPath(normalized)
    if parsed.is_absolute() or any(
        part in {"", ".", ".."} for part in parsed.parts
    ):
        raise ValueError(
            f"mapping transaction path must be repository-relative: {value}"
        )
    return parsed.as_posix()


def _normalize_entries(
    values: Mapping[str, Any],
    label: str,
) -> dict[str, Any]:
    normalized = {}
    raw_by_relative = {}
    for raw_path, value in values.items():
        relative = _relative_path(raw_path)
        previous = raw_by_relative.get(relative)
        if previous is not None:
            raise ValueError(
                f"mapping transaction {label} path alias: "
                f"{previous!r}, {raw_path!r}"
            )
        raw_by_relative[relative] = raw_path
        normalized[relative] = value
    return normalized


def _validate_target_aliases(
    targets: Mapping[str, _Target | None],
) -> None:
    seen: dict[tuple[int, int, str], str] = {}
    for relative, target in targets.items():
        if target is None:
            continue
        parent = os.fstat(target.parent_fd)
        identity = (parent.st_dev, parent.st_ino, target.name)
        previous = seen.get(identity)
        if previous is not None and previous != relative:
            raise ValueError(
                f"mapping transaction resolved target alias: "
                f"{previous}, {relative}"
            )
        seen[identity] = relative


def _validate_lexical_target_identities(
    root_fd: int,
    targets: Mapping[str, _Target | None],
) -> None:
    """Reject a renamed or replaced lexical parent before committing."""

    reopened: list[_Target] = []
    try:
        for relative, held in targets.items():
            if held is None:
                continue
            try:
                current = _open_target(
                    root_fd, relative, create_parent=False
                )
            except ValueError as error:
                raise ValueError(
                    "mapping transaction lexical parent drift: "
                    f"{relative}"
                ) from error
            if current is None:
                raise ValueError(
                    "mapping transaction lexical parent drift: "
                    f"{relative}"
                )
            reopened.append(current)
            held_stat = os.fstat(held.parent_fd)
            current_stat = os.fstat(current.parent_fd)
            if (
                held_stat.st_dev,
                held_stat.st_ino,
                held.name,
            ) != (
                current_stat.st_dev,
                current_stat.st_ino,
                current.name,
            ):
                raise ValueError(
                    "mapping transaction lexical parent drift: "
                    f"{relative}"
                )
    finally:
        for target in reopened:
            target.close()


def _json_bytes(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        )
        + "\n"
    ).encode()
