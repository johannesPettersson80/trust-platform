#!/usr/bin/env python3
"""Write a release's text: its title and its version's section of the changelog (Keep a Changelog)."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


HEADER_RE = re.compile(r"^## \[(?P<name>[^\]]+)\]")
# GitHub keeps this many characters of a release's text and drops the rest without a word.
RELEASE_TEXT_LIMIT = 125_000


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


def release_text(text: str, section: str, title: str) -> str:
    release = f"# {title}\n\n{extract_section(text, section)}\n"
    if len(release) > RELEASE_TEXT_LIMIT:
        raise ValueError(
            f"the release text from changelog section [{section}] has {len(release)} characters; "
            f"GitHub keeps {RELEASE_TEXT_LIMIT}"
        )
    return release


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--section", required=True)
    parser.add_argument("--title", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    release = release_text(Path(args.changelog).read_text(encoding="utf-8"), args.section, args.title)
    Path(args.output).write_text(release, encoding="utf-8")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
