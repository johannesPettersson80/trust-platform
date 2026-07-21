"""Exact Markdown-fragment resolution for Phase 17 specification mappings."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .metadata_validator.constants import ROOT
from .spec_source_markdown import scan_markdown_document


def source_fragment_exists(
    source: Mapping[str, Any],
    fragment: str,
    *,
    root: Path = ROOT,
) -> bool:
    """Return whether a tracked Markdown source owns the exact heading anchor."""

    path = source.get("path")
    if (
        not fragment
        or source.get("locator_kind") != "tracked_file"
        or not isinstance(path, str)
        or not path.endswith(".md")
    ):
        return False
    relative = Path(path)
    if relative.is_absolute() or ".." in relative.parts:
        return False
    candidate = root / relative
    try:
        resolved_root = root.resolve(strict=True)
        resolved_candidate = candidate.resolve(strict=True)
        if candidate.is_symlink() or not resolved_candidate.is_relative_to(resolved_root):
            return False
        scan = scan_markdown_document(
            path,
            resolved_candidate.read_text(encoding="utf-8"),
        )
    except (OSError, UnicodeDecodeError):
        return False
    return fragment in {heading.anchor for heading in scan.headings}
