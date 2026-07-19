"""Durable output and Git provenance for producer-authentic proof records."""

from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any


CANONICAL_EVIDENCE_INDEX = Path("verification/evidence-index.toml")
FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


class ProofOutputError(RuntimeError):
    pass


@dataclass
class ProofRevisionSession:
    """Acquire and hold the clean source revision for one proof command."""

    root: Path
    revision_provider: Any | None = None
    ancestry_checker: Any | None = None
    active_revision: str | None = None

    def __post_init__(self) -> None:
        if self.revision_provider is None:
            self.revision_provider = lambda: clean_head_revision(self.root)
        if self.ancestry_checker is None:
            self.ancestry_checker = lambda before, after: revision_is_ancestor(
                self.root, before, after
            )

    def begin(self) -> str:
        revision = str(self.revision_provider())
        if not FULL_COMMIT_RE.fullmatch(revision):
            raise ProofOutputError(
                f"proof source revision must be a clean full 40-hex commit, found {revision!r}"
            )
        self.active_revision = revision
        return revision

    def confirm(self) -> None:
        current = str(self.revision_provider())
        if self.active_revision is None or current != self.active_revision:
            raise ProofOutputError(
                "proof source revision changed during execution: "
                f"{self.active_revision!r} -> {current!r}"
            )

    def require_red_before_current(self, red_evidence: dict[str, Any]) -> None:
        red_revision = red_evidence.get("commit")
        if not isinstance(red_revision, str) or not FULL_COMMIT_RE.fullmatch(red_revision):
            raise ProofOutputError(
                f"{red_evidence.get('id')} proof requires a clean full 40-hex commit"
            )
        if self.active_revision is None:
            raise ProofOutputError("green proof source revision was not acquired")
        if red_revision == self.active_revision:
            raise ProofOutputError("red and green proof must use distinct commits")
        if not self.ancestry_checker(red_revision, self.active_revision):
            raise ProofOutputError(
                f"red commit {red_revision} is not an ancestor of green commit {self.active_revision}"
            )


def clean_head_revision(root: Path) -> str:
    """Return HEAD as a full SHA only when all tracked and untracked state is clean."""

    status = _git(root, "status", "--porcelain", "--untracked-files=all")
    if status:
        raise ProofOutputError("proof production requires a clean Git worktree")
    revision = _git(root, "rev-parse", "--verify", "HEAD^{commit}")
    if not FULL_COMMIT_RE.fullmatch(revision):
        raise ProofOutputError(f"proof source revision is not a full Git commit: {revision!r}")
    return revision


def revision_is_ancestor(root: Path, before: str, after: str) -> bool:
    if not FULL_COMMIT_RE.fullmatch(before) or not FULL_COMMIT_RE.fullmatch(after):
        return False
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", before, after],
        cwd=root,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def append_evidence_record(
    *,
    root: Path,
    evidence_index_path: Path,
    record: dict[str, Any],
) -> Path:
    """Atomically append one unique record to the canonical tracked evidence index."""

    root = root.resolve()
    canonical = root / CANONICAL_EVIDENCE_INDEX
    path = evidence_index_path.resolve()
    if path != canonical:
        raise ProofOutputError(
            f"proof output must use the canonical evidence index {CANONICAL_EVIDENCE_INDEX}"
        )
    _require_regular_nonsymlink_path(root, path)
    _require_tracked_not_ignored(root, CANONICAL_EVIDENCE_INDEX)

    try:
        current = path.read_text()
        payload = tomllib.loads(current)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ProofOutputError(f"cannot read evidence index {CANONICAL_EVIDENCE_INDEX}: {exc}") from exc
    records = payload.get("evidence")
    if not isinstance(records, list):
        raise ProofOutputError("canonical evidence index has no [[evidence]] records")
    record_id = record.get("id")
    if not isinstance(record_id, str) or not record_id:
        raise ProofOutputError("generated proof record has no id")
    if any(isinstance(item, dict) and item.get("id") == record_id for item in records):
        raise ProofOutputError(f"evidence id {record_id} already exists")
    if record.get("path") != CANONICAL_EVIDENCE_INDEX.as_posix():
        raise ProofOutputError(
            f"generated proof record path must be {CANONICAL_EVIDENCE_INDEX.as_posix()}"
        )

    separator = "" if current.endswith("\n\n") else "\n"
    try:
        rendered = render_evidence_record(record)
    except TypeError as exc:
        raise ProofOutputError(f"cannot render generated evidence record: {exc}") from exc
    updated = current + separator + rendered
    _atomic_write(path, updated)
    return path


def render_evidence_record(record: dict[str, Any]) -> str:
    lines = ["[[evidence]]"]
    for key, value in record.items():
        lines.append(f"{key} = {render_toml_value(value)}")
    return "\n".join(lines) + "\n"


def render_toml_value(value: Any) -> str:
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(render_toml_value(item) for item in value) + "]"
    if isinstance(value, Mapping):
        entries: list[str] = []
        for key in sorted(value):
            if not isinstance(key, str) or not key:
                raise TypeError(f"unsupported TOML evidence key {key!r}")
            rendered_key = key if re.fullmatch(r"[A-Za-z0-9_-]+", key) else json.dumps(key)
            entries.append(f"{rendered_key} = {render_toml_value(value[key])}")
        return "{ " + ", ".join(entries) + " }"
    raise TypeError(f"unsupported TOML evidence value {value!r}")


def _require_regular_nonsymlink_path(root: Path, path: Path) -> None:
    try:
        relative = path.relative_to(root)
    except ValueError as exc:
        raise ProofOutputError("canonical evidence index escapes the workspace") from exc
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ProofOutputError(f"proof output path contains a symlink: {relative}")
    if not path.is_file():
        raise ProofOutputError(f"proof output path is not a regular file: {relative}")


def _require_tracked_not_ignored(root: Path, relative: Path) -> None:
    tracked = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", relative.as_posix()],
        cwd=root,
        text=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if tracked.returncode != 0:
        raise ProofOutputError(f"proof output is not Git-tracked: {relative}")
    ignored = subprocess.run(
        ["git", "check-ignore", "-q", "--", relative.as_posix()],
        cwd=root,
        check=False,
    )
    if ignored.returncode == 0:
        raise ProofOutputError(f"proof output is Git-ignored: {relative}")
    if ignored.returncode != 1:
        raise ProofOutputError(f"git check-ignore failed for proof output: {relative}")


def _atomic_write(path: Path, content: str) -> None:
    mode = path.stat().st_mode
    temp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            delete=False,
        ) as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
            temp_path = Path(handle.name)
        os.chmod(temp_path, mode)
        os.replace(temp_path, path)
    except OSError as exc:
        if temp_path is not None:
            temp_path.unlink(missing_ok=True)
        raise ProofOutputError(f"failed to append durable proof evidence: {exc}") from exc


def _git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit {result.returncode}"
        raise ProofOutputError(f"git {' '.join(args)} failed: {detail}")
    return result.stdout.strip()
