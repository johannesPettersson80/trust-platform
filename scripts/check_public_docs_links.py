#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path


DOCS_ROOT = Path("docs/public")
LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")


def normalize_target(source: Path, target: str) -> Path | None:
    cleaned = target.strip()
    if not cleaned or cleaned.startswith("#"):
        return None
    if "://" in cleaned or cleaned.startswith("mailto:"):
        return None
    cleaned = cleaned.split("#", 1)[0]
    if not cleaned:
        return None
    path = (source.parent / cleaned).resolve() if not cleaned.startswith("/") else (Path.cwd() / cleaned.lstrip("/")).resolve()
    return path


def main() -> int:
    if not DOCS_ROOT.exists():
        print(f"docs root missing: {DOCS_ROOT}", file=sys.stderr)
        return 1

    failures: list[str] = []
    for source in sorted(DOCS_ROOT.rglob("*.md")):
        text = source.read_text(encoding="utf-8")
        for raw_target in LINK_RE.findall(text):
            target = normalize_target(source, raw_target)
            if target is None:
                continue
            if target.is_dir():
                target = target / "index.md"
            if not target.exists():
                failures.append(f"{source}: missing target {raw_target}")

    if failures:
        print("public docs link check failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("public docs link check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
