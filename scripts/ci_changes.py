#!/usr/bin/env python3
"""Whether a pull request, a merge group or a push to `main` changes a path of a workflow's list.

Specification 22, "Pull-request checks and change detection": a workflow starts on every pull
request, and its first job asks this script whether the change touches the workflow's paths. It
compares the whole change with `git diff --name-only`; GitHub's own path filters compare only the
first 300 changed files, so 0.25.0's 10,252-file pull request ran no documentation workflow.

Usage, in a job whose checkout has the full history (`fetch-depth: 0`):

    python3 scripts/ci_changes.py --event "$EVENT" --base "$BASE" --head "$HEAD" --paths-file paths.txt

Prints the matched files and writes `relevant=true` or `relevant=false` to `$GITHUB_OUTPUT`.
A merge group compares its commit with the `main` it was built on. A push to `main` that names its
workflow (`--workflow`, its file under `.github/workflows`) compares with the commit of that
workflow's newest passed run on a push to `main`, as GitHub lists it: a newer push replaces a run
that still waits, so comparing with the push's previous commit would never compare the replaced
push's changes. A manual or scheduled run, and a push without a known passed run or previous commit,
is always relevant. A failing `git diff` fails the script, and so the workflow, instead of skipping
its checks.
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
# The events whose checks follow the changed files; every other event checks everything.
CHANGE_EVENTS = {"pull_request", "merge_group", "push"}


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
    """The files the change touches: a pull request against its merge base, a push or a merge group
    since `base`. A moved file counts at both its old and its new path (`--no-renames`), so moving a
    page out of a watched tree still runs the checks that publish it."""
    revisions = f"{base}...{head}" if event == "pull_request" else f"{base}..{head}"
    result = subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", revisions],
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


def last_passed_head(repository: str, workflow: str, run=subprocess.run) -> str | None:
    """The commit of the newest run of `workflow` that a push to `main` started and that passed, as
    GitHub lists the runs (`gh api`); `None` when there is none or the runs cannot be listed."""
    if not repository or not workflow:
        return None
    result = run(
        [
            "gh",
            "api",
            f"repos/{repository}/actions/workflows/{workflow}/runs"
            "?branch=main&event=push&status=success&per_page=1",
            "--jq",
            ".workflow_runs[0].head_sha // empty",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    head = result.stdout.strip()
    return head if result.returncode == 0 and re.fullmatch(r"[0-9a-f]{40}", head) else None


def compared_base(
    event: str, base: str, workflow: str, repository: str, passed_head=last_passed_head
) -> str:
    """The commit that a run compares its head with: for a push to `main` that names its workflow,
    the commit of the workflow's newest passed run, empty when none is known so that everything is
    checked; for every other run, `base`."""
    if event != "push" or not workflow:
        return base
    return passed_head(repository, workflow) or ""


def decide(
    event: str, base: str, head: str, patterns: Sequence[str], cwd: Path | None = None
) -> tuple[bool, list[str]]:
    """Whether the workflow's checks must run, and the changed files that made them run."""
    if event not in CHANGE_EVENTS:
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
    parser.add_argument("--workflow", default="")
    args = parser.parse_args(argv)

    patterns = read_patterns(args.paths_file.read_text(encoding="utf-8"))
    if not patterns:
        print("ci_changes: the path list is empty", file=sys.stderr)
        return 2
    base = compared_base(
        args.event, args.base.strip(), args.workflow, os.environ.get("GITHUB_REPOSITORY", "")
    )
    relevant, matched = decide(args.event, base, args.head.strip(), patterns)
    if args.event not in CHANGE_EVENTS:
        print(f"ci_changes: a {args.event} run checks everything")
    elif args.event == "push" and args.workflow and not base:
        print(f"ci_changes: no passed run of {args.workflow} on main is known, so everything is checked")
    elif not base or base == NULL_SHA:
        print("ci_changes: the previous commit is unknown, so everything is checked")
    else:
        if args.event == "push" and args.workflow:
            print(f"ci_changes: compared with {base}, the newest passed run of {args.workflow} on main")
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
