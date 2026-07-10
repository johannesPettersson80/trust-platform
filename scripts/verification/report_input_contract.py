"""Shared provenance closure and path checks for verification reports."""

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path, PurePosixPath


def validator_code_input_paths(root: Path) -> set[str]:
    """Return validator code inputs without mutable evidence/data registries."""

    root = root.resolve()
    paths: set[str] = {
        "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
        "scripts/gen_cases.py",
        "scripts/validate_verification_metadata.py",
        "verification/malformed-input-taxonomy.md",
        "verification/malformed-input-taxonomy.toml",
        "verification/schemas/malformed-input-taxonomy.schema.json",
    }
    paths.update(
        path
        for path in _files_under(root, root / "scripts/verification")
        if path.endswith(".py")
    )
    return paths


def validate_bound_input_paths(root: Path, input_paths: Iterable[str]) -> list[str]:
    """Reject missing, escaping, or symlinked report inputs."""

    root = root.resolve()
    failures: list[str] = []
    for value in sorted(set(input_paths)):
        if not _is_safe_relative_path(value):
            failures.append(f"input path must be normalized and workspace-relative: {value!r}")
            continue
        candidate = root
        for part in PurePosixPath(value).parts:
            candidate /= part
            if candidate.is_symlink():
                failures.append(f"input path contains a symlink component: {value}")
                break
        try:
            resolved = (root / value).resolve(strict=True)
        except OSError as exc:
            failures.append(f"input path cannot be resolved as a regular file: {value}: {exc}")
            continue
        try:
            resolved.relative_to(root)
        except ValueError:
            failures.append(f"input path escapes the workspace after resolution: {value}")
        if not resolved.is_file():
            failures.append(f"input path is not a regular file: {value}")
    return sorted(set(failures))


def _files_under(root: Path, directory: Path) -> set[str]:
    if not directory.is_dir():
        return set()
    return {
        path.relative_to(root).as_posix()
        for path in directory.rglob("*")
        if (path.is_file() or path.is_symlink())
        and "__pycache__" not in path.parts
        and path.suffix != ".pyc"
    }


def _is_safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts
