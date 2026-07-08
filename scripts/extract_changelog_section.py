#!/usr/bin/env python3
"""Extract a Keep a Changelog section into a release body file."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


HEADER_RE = re.compile(r"^## \[(?P<name>[^\]]+)\]")


def extract_section(text: str, section: str) -> str:
    lines = text.splitlines()
    start: int | None = None
    end = len(lines)
    for index, line in enumerate(lines):
        match = HEADER_RE.match(line)
        if not match:
            continue
        if start is None and match.group("name") == section:
            start = index + 1
            continue
        if start is not None:
            end = index
            break
    if start is None:
        raise ValueError(f"Could not find changelog section [{section}]")
    body = "\n".join(lines[start:end]).strip()
    if not body:
        raise ValueError(f"Changelog section [{section}] is empty")
    return body


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--section", default="Unreleased")
    parser.add_argument("--title", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    body = extract_section(Path(args.changelog).read_text(encoding="utf-8"), args.section)
    Path(args.output).write_text(f"# {args.title}\n\n{body}\n", encoding="utf-8")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
