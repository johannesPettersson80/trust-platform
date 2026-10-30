#!/usr/bin/env python3
"""Whether a pull request or a push to `main` changes a path of a workflow's list.

Specification 22, "Pull-request checks and change detection": a workflow starts on every pull
request, and its first job asks this script whether the change touches the workflow's paths. It
compares the whole change with `git diff --name-only`; GitHub's own path filters compare only the
first 300 changed files, so 0.25.0's 10,252-file pull request ran no documentation workflow.

Usage, in a job whose checkout has the full history (`fetch-depth: 0`):

    python3 scripts/ci_changes.py --event "$EVENT" --base "$BASE" --head "$HEAD" --paths-file paths.txt

Prints the matched files and writes `relevant=true` or `relevant=false` to `$GITHUB_OUTPUT`.
A manual or scheduled run, and a push whose previous commit is unknown, is always relevant. A
failing `git diff` fails the script, and so the workflow, instead of skipping its checks.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Sequence

NULL_SHA = "0" * 40


def pattern_regex(pattern: str) -> re.Pattern[str]:
    """A path glob as GitHub writes them: `**` crosses directories; `*` and `?` stay in one."""
    parts: list[str] = []
    index = 0
    while index < len(pattern):
        if pattern.startswith("**/", index):
            parts.append("(?:.*/)?")
            index += 3
        elif pattern.startswith("**", index):
            parts.append(".*")
            index += 2
        elif pattern[index] == "*":
            parts.append("[^/]*")
            index += 1
        elif pattern[index] == "?":
            parts.append("[^/]")
            index += 1
        else:
            parts.append(re.escape(pattern[index]))
            index += 1
    return re.compile("".join(parts) + r"\Z")


def read_patterns(text: str) -> list[str]:
    """One glob per line; blank lines and `#` comments are ignored."""
    return [
        line.strip()
        for line in text.splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]


def changed_files(event: str, base: str, head: str, cwd: Path | None = None) -> list[str]:
    """The files the change touches: a pull request against its merge base, a push since `base`."""
    revisions = f"{base}...{head}" if event == "pull_request" else f"{base}..{head}"
    result = subprocess.run(
        ["git", "diff", "--name-only", revisions],
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


def decide(
    event: str, base: str, head: str, patterns: Sequence[str], cwd: Path | None = None
) -> tuple[bool, list[str]]:
    """Whether the workflow's checks must run, and the changed files that made them run."""
    if event not in {"pull_request", "push"}:
        return True, []
    if not base or base == NULL_SHA:
        return True, []
    regexes = [pattern_regex(pattern) for pattern in patterns]
    matched = [
        path
        for path in changed_files(event, base, head, cwd)
        if any(regex.match(path) for regex in regexes)
    ]
    return bool(matched), matched


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--event", required=True)
    parser.add_argument("--base", default="")
    parser.add_argument("--head", required=True)
    parser.add_argument("--paths-file", required=True, type=Path)
    args = parser.parse_args(argv)

    patterns = read_patterns(args.paths_file.read_text(encoding="utf-8"))
    if not patterns:
        print("ci_changes: the path list is empty", file=sys.stderr)
        return 2
    relevant, matched = decide(args.event, args.base.strip(), args.head.strip(), patterns)
    if args.event not in {"pull_request", "push"}:
        print(f"ci_changes: a {args.event} run checks everything")
    elif not args.base.strip() or args.base.strip() == NULL_SHA:
        print("ci_changes: the previous commit is unknown, so everything is checked")
    else:
        print(f"ci_changes: {len(matched)} changed file(s) match the workflow's paths")
        for path in matched[:50]:
            print(f"  {path}")
    line = f"relevant={'true' if relevant else 'false'}"
    print(line)
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as handle:
            handle.write(line + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
