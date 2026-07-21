"""Input closure and source-revision checks for Phase 17 reports."""

from __future__ import annotations

import hashlib
import re
import subprocess
from pathlib import Path
from typing import Iterable

from .code_spec_common import normalize_repo_path


class CodeSpecProvenanceError(ValueError):
    """Raised when report provenance is not reproducible."""


def input_closure_digest(root: Path, paths: Iterable[str]) -> str:
    normalized = []
    for raw_path in paths:
        try:
            normalized.append(normalize_repo_path(raw_path))
        except ValueError as error:
            raise CodeSpecProvenanceError(str(error)) from error
    if len(normalized) != len(set(normalized)):
        raise CodeSpecProvenanceError("input closure contains duplicate path identities")
    digest = hashlib.sha256()
    for path in sorted(normalized):
        content = _read_input(root, path)
        encoded_path = path.encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return "sha256:" + digest.hexdigest()


def validate_clean_source_revision(root: Path, commit: str, paths: Iterable[str]) -> list[str]:
    failures: list[str] = []
    normalized = list(paths)
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        failures.append("commit must identify a clean full Git SHA")
    head = _git(root, "rev-parse", "HEAD")
    if head.returncode != 0:
        failures.append("repository HEAD cannot be resolved")
    elif head.stdout.strip() != commit:
        failures.append("report commit does not equal repository HEAD")
    status = _git(root, "status", "--porcelain", "--untracked-files=all")
    if status.returncode != 0:
        failures.append("repository clean state cannot be determined")
    elif status.stdout:
        failures.append("repository must be clean for report provenance")
    try:
        input_closure_digest(root, normalized)
    except CodeSpecProvenanceError as error:
        failures.append(str(error))
    for path in sorted(set(normalized)):
        tracked = _git(root, "ls-files", "--error-unmatch", "--", path)
        if tracked.returncode != 0:
            failures.append(f"report input is not tracked: {path}")
    return sorted(set(failures))


def _read_input(root: Path, path: str) -> bytes:
    current = root
    for part in Path(path).parts:
        current = current / part
        if current.is_symlink():
            raise CodeSpecProvenanceError(f"input closure rejects symlink path {path}")
    try:
        canonical_root = root.resolve(strict=True)
        resolved = (root / path).resolve(strict=True)
        resolved.relative_to(canonical_root)
    except (FileNotFoundError, ValueError) as error:
        raise CodeSpecProvenanceError(f"input closure path is unreadable or escapes: {path}") from error
    if not resolved.is_file():
        raise CodeSpecProvenanceError(f"input closure path is not a file: {path}")
    return resolved.read_bytes()


def _git(root: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
