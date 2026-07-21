"""Shared source safety and canonical ownership routing for Phase 17 adapters."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from .area_routing import AreaRoutingError, classify_changed_path
from .code_spec_common import normalize_repo_path


class CodeSpecSourceError(ValueError):
    """Raised when a source fact cannot be read or routed safely."""


def governed_source_bytes(root: Path, raw_path: str) -> tuple[str, bytes]:
    try:
        path = normalize_repo_path(raw_path)
    except ValueError as error:
        raise CodeSpecSourceError(str(error)) from error
    canonical_root = root.resolve(strict=True)
    current = root
    for part in Path(path).parts:
        current = current / part
        if current.is_symlink():
            raise CodeSpecSourceError(f"symlinked source is not allowed: {path}")
    try:
        resolved = (root / path).resolve(strict=True)
        resolved.relative_to(canonical_root)
    except (FileNotFoundError, ValueError) as error:
        raise CodeSpecSourceError(f"source escapes or is unreadable: {path}") from error
    if not resolved.is_file():
        raise CodeSpecSourceError(f"source is not a file: {path}")
    return path, resolved.read_bytes()


def candidate_area_owners(matrix: Mapping[str, Any], path: str) -> dict[str, str]:
    try:
        route = classify_changed_path(matrix, path)
    except AreaRoutingError as error:
        raise CodeSpecSourceError(f"cannot classify source {path}: {error}") from error
    owners = {
        record.get("id"): record.get("owner")
        for record in matrix.get("areas", [])
        if isinstance(record, Mapping)
    }
    result: dict[str, str] = {}
    for area in route.area_ids:
        owner = owners.get(area)
        if not isinstance(owner, str) or not owner:
            raise CodeSpecSourceError(f"canonical area {area} has no owner")
        result[area] = owner
    return result


def select_area_owner(
    code_id: str,
    candidates: Mapping[str, str],
    area_bindings: Mapping[str, str],
) -> tuple[str, str]:
    requested_area = area_bindings.get(code_id)
    if requested_area is not None:
        if requested_area not in candidates:
            raise CodeSpecSourceError(
                f"{code_id}: area {requested_area} is not permitted by path routing "
                f"{sorted(candidates)}"
            )
        return requested_area, candidates[requested_area]
    if len(candidates) == 1:
        return next(iter(candidates.items()))
    return "unmapped", "unmapped"
