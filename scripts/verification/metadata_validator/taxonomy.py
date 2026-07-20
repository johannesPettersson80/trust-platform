"""Markdown taxonomy drift checks for verification metadata."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Callable

from .constants import CASE_FAMILIES, ROOT, TEST_CLASSES

Fail = Callable[[Path, str], None]

TAXONOMY = ROOT / "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md"


def validate_taxonomy_drift(fail: Fail) -> None:
    """Keep the prose taxonomy and machine vocabularies in lockstep."""

    text = TAXONOMY.read_text()
    markdown_classes = _extract_test_classes(text)
    markdown_families = _extract_case_families(text)
    if markdown_classes != TEST_CLASSES:
        fail(
            TAXONOMY,
            "test class taxonomy drifts from TEST_CLASSES: "
            f"missing={sorted(TEST_CLASSES - markdown_classes)}, "
            f"extra={sorted(markdown_classes - TEST_CLASSES)}",
        )
    if markdown_families != CASE_FAMILIES:
        fail(
            TAXONOMY,
            "coverage dimensions drift from CASE_FAMILIES: "
            f"missing={sorted(CASE_FAMILIES - markdown_families)}, "
            f"extra={sorted(markdown_families - CASE_FAMILIES)}",
        )


def _extract_test_classes(text: str) -> set[str]:
    section = _between(text, "## Test Classes", "## Coverage Dimensions")
    classes: set[str] = set()
    for line in section.splitlines():
        match = re.match(r"^\| `([^`]+)` \|", line)
        if match:
            classes.add(match.group(1))
    return classes


def _extract_case_families(text: str) -> set[str]:
    section = _between(text, "Required dimensions:", "Rules:")
    families: set[str] = set()
    for line in section.splitlines():
        match = re.match(r"^- `([^`]+)`$", line.strip())
        if match:
            families.add(match.group(1))
    return families


def _between(text: str, start: str, end: str) -> str:
    start_index = text.index(start) + len(start)
    end_index = text.index(end, start_index)
    return text[start_index:end_index]
