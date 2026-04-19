#!/usr/bin/env python3

from __future__ import annotations

import sys
from pathlib import Path


SOURCE_ROOT = Path("docs/public/assets")
SITE_ROOT = Path("site/public/assets")


def tracked_asset_files(root: Path) -> list[Path]:
    if not root.exists():
        return []
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file() and not any(part.startswith(".") for part in path.relative_to(root).parts)
    )


def main() -> int:
    source_files = tracked_asset_files(SOURCE_ROOT)
    if not source_files:
        print("public docs asset check passed (no tracked assets yet)")
        return 0

    if not SITE_ROOT.exists():
        print(f"missing built asset root: {SITE_ROOT}", file=sys.stderr)
        return 1

    failures: list[str] = []
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
