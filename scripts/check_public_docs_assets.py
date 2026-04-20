#!/usr/bin/env python3

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


DOCS_ROOT = Path("docs/public")
SOURCE_ROOT = Path("docs/public/assets")
SITE_ROOT = Path("site/public/assets")
CAPTURE_INVENTORY = SOURCE_ROOT / "capture-inventory.json"
IMAGE_REF_RE = re.compile(r"!\[[^\]]*\]\(([^)]+)\)")


def tracked_asset_files(root: Path) -> list[Path]:
    if not root.exists():
        return []
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file() and not any(part.startswith(".") for part in path.relative_to(root).parts)
    )


def expected_capture_files() -> list[Path]:
    if not CAPTURE_INVENTORY.exists():
        return []
    payload = json.loads(CAPTURE_INVENTORY.read_text())
    captures = payload.get("captures")
    if not isinstance(captures, list):
        raise ValueError("capture inventory must contain a list under 'captures'")
    expected: list[Path] = []
    for index, capture in enumerate(captures):
        if not isinstance(capture, dict):
            raise ValueError(f"capture inventory entry {index} must be an object")
        raw_path = capture.get("path")
        if not isinstance(raw_path, str) or not raw_path:
            raise ValueError(f"capture inventory entry {index} is missing a string 'path'")
        expected.append(Path(raw_path))
    return expected


def referenced_asset_files() -> list[Path]:
    if not DOCS_ROOT.exists():
        return []
    referenced: set[Path] = set()
    for doc in DOCS_ROOT.rglob("*.md"):
        text = doc.read_text()
        for match in IMAGE_REF_RE.finditer(text):
            raw_ref = match.group(1).strip()
            if raw_ref.startswith(("http://", "https://", "#")):
                continue
            resolved = (doc.parent / raw_ref).resolve()
            try:
                relative = resolved.relative_to(SOURCE_ROOT.resolve())
            except ValueError:
                continue
            referenced.add(relative)
    return sorted(referenced)


def main() -> int:
    source_files = tracked_asset_files(SOURCE_ROOT)
    try:
        capture_files = expected_capture_files()
    except ValueError as exc:
        print(f"public docs asset check failed: {exc}", file=sys.stderr)
        return 1
    referenced_files = referenced_asset_files()
    if not source_files:
        print("public docs asset check passed (no tracked assets yet)")
        return 0

    if not SITE_ROOT.exists():
        print(f"missing built asset root: {SITE_ROOT}", file=sys.stderr)
        return 1

    failures: list[str] = []
    capture_file_set = set(capture_files)
    for relative in capture_files:
        source_capture = SOURCE_ROOT / relative
        if not source_capture.exists():
            failures.append(f"missing capture inventory asset {source_capture}")
    for relative in referenced_files:
        if relative not in capture_file_set:
            failures.append(f"missing capture inventory entry for referenced asset {SOURCE_ROOT / relative}")
    for source in source_files:
        relative = source.relative_to(SOURCE_ROOT)
        built = SITE_ROOT / relative
        if not built.exists():
            failures.append(f"missing built asset {built}")

    if failures:
        print("public docs asset check failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("public docs asset check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
