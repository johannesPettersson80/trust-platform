"""Symlink-free repository quarantine path validation."""

from __future__ import annotations

from pathlib import Path


QUARANTINE_ROOT = "target/gate-artifacts"


def quarantine_path(
    root: Path,
    path: Path,
    *,
    label: str,
    must_exist: bool,
) -> tuple[str, Path]:
    """Resolve one quarantine file without following repository symlinks."""

    root = root.resolve()
    candidate = path if path.is_absolute() else root / path
    try:
        lexical_relative = candidate.relative_to(root)
    except ValueError as error:
        raise ValueError(
            f"{label} must stay under {QUARANTINE_ROOT}"
        ) from error
    if any(part in {"", ".", ".."} for part in lexical_relative.parts):
        raise ValueError(f"{label} path is not canonical")
    current = root
    for part in lexical_relative.parts:
        current /= part
        if current.is_symlink():
            raise ValueError(f"{label} path contains symlink: {current}")
    resolved = candidate.resolve()
    quarantine = (root / QUARANTINE_ROOT).resolve()
    try:
        relative = resolved.relative_to(quarantine)
    except ValueError as error:
        raise ValueError(
            f"{label} must stay under {QUARANTINE_ROOT}"
        ) from error
    if not relative.parts:
        raise ValueError(f"{label} must be a quarantine file")
    if must_exist and not resolved.is_file():
        raise ValueError(f"{label} is absent")
    if not must_exist and resolved.exists() and not resolved.is_file():
        raise ValueError(f"{label} output is not a regular file")
    return resolved.relative_to(root).as_posix(), resolved
