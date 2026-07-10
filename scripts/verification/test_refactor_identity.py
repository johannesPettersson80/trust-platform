"""Shared scanner-identity and contained-path checks for test refactors."""

from __future__ import annotations

import re
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from .test_catalog_models import InferredTestFact
from .test_catalog_validation import SOURCE_KINDS


IDENTITY_FIELDS = {"discovery_id", "discovery_source_kind", "path", "name"}
IDENTITY_RE = re.compile(r"^DISC_[A-F0-9]{20}$")


def validate_identity(
    value: Any,
    label: str,
    failures: list[str],
) -> dict[str, str] | None:
    if not isinstance(value, Mapping):
        failures.append(f"{label} must be an object")
        return None
    missing = sorted(IDENTITY_FIELDS - set(value))
    extra = sorted(set(value) - IDENTITY_FIELDS)
    for field in missing:
        failures.append(f"{label} missing required field {field}")
    for field in extra:
        failures.append(f"{label} has additional field {field}")
    result: dict[str, str] = {}
    for field in IDENTITY_FIELDS:
        item = value.get(field)
        if not isinstance(item, str) or not item.strip():
            failures.append(f"{label} requires non-empty {field}")
        else:
            result[field] = item
    discovery_id = value.get("discovery_id")
    if not isinstance(discovery_id, str) or not IDENTITY_RE.fullmatch(discovery_id):
        failures.append(f"{label} has invalid discovery_id {discovery_id!r}")
    path = value.get("path")
    if isinstance(path, str) and not safe_relative_path(path):
        failures.append(f"{label} has unsafe path {path!r}")
    source_kind = value.get("discovery_source_kind")
    if isinstance(source_kind, str) and source_kind not in SOURCE_KINDS:
        failures.append(f"{label} has unsupported discovery_source_kind {source_kind!r}")
    return result if len(result) == len(IDENTITY_FIELDS) else None


def catalog_identity(record: Mapping[str, Any]) -> dict[str, Any]:
    return {field: record.get(field) for field in IDENTITY_FIELDS}


def fact_identity(fact: InferredTestFact) -> dict[str, str]:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
    }


def safe_relative_path(value: str) -> bool:
    path = PurePosixPath(value)
    return (
        bool(value)
        and "\\" not in value
        and not path.is_absolute()
        and ".." not in path.parts
        and "." not in path.parts
        and path.as_posix() == value
    )


def validate_live_path(
    root: Path,
    value: str,
    label: str,
    failures: list[str],
) -> None:
    if not safe_relative_path(value):
        return
    root_resolved = root.resolve()
    candidate = root_resolved / value
    current = root_resolved
    for part in PurePosixPath(value).parts:
        current /= part
        if current.is_symlink():
            failures.append(f"{label} path contains a symlink component: {value}")
            return
    try:
        candidate.resolve(strict=True).relative_to(root_resolved)
    except (OSError, ValueError):
        failures.append(f"{label} path escapes the workspace or does not exist: {value}")
        return
    if candidate.is_symlink() or not candidate.is_file():
        failures.append(f"{label} path must be a regular non-symlink file: {value}")
