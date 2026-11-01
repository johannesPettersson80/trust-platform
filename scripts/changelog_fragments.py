#!/usr/bin/env python3
"""Changelog fragments (specification 22, "Change and release workflow"): a pull request with a
user-visible change adds one file `changelog.d/<topic>.md` instead of editing `CHANGELOG.md`, so that
open pull requests never conflict over the changelog. A fragment holds Keep a Changelog sections
(`### Added`, `### Changed`, `### Deprecated`, `### Removed`, `### Fixed`, `### Security`), each with
one or more entries (`- ...`, continued on lines indented by two spaces). The version bump
(`scripts/bump_version.py`) gathers the fragments into the version's section of `CHANGELOG.md`, the
newest first, and removes them; `## [Unreleased]` stays empty.

Usage: changelog_fragments.py check   (every fragment reads, and `## [Unreleased]` is empty)"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# The kinds of change of Keep a Changelog, in its order.
SECTIONS = ("Added", "Changed", "Deprecated", "Removed", "Fixed", "Security")
FRAGMENTS = "changelog.d"
# The folder's own explanation, which is no fragment.
README = "README.md"
NAME_RE = re.compile(r"^[a-z0-9][a-z0-9-]*\.md$")
VERSION_RE = re.compile(r"^## \[(?P<name>[^\]]+)\]")


def sections(text: str, source: str) -> dict[str, list[str]]:
    """The entries of each section of `text`, in their order; `ValueError` names what is wrong."""
    found: dict[str, list[str]] = {}
    current: list[str] | None = None
    for number, line in enumerate(text.splitlines(), start=1):
        where = f"{source}:{number}"
        if line.startswith("#"):
            kind = line[4:].strip() if line.startswith("### ") else ""
            if kind not in SECTIONS:
                raise ValueError(f"{where}: a heading is one of {', '.join('### ' + s for s in SECTIONS)}, not {line!r}")
            if kind in found:
                raise ValueError(f"{where}: `### {kind}` comes twice")
            current = found.setdefault(kind, [])
        elif line.startswith("- "):
            if current is None:
                raise ValueError(f"{where}: an entry comes under a `### ` section")
            current.append(line)
        elif line.startswith("  ") and line.strip():
            if not current:
                raise ValueError(f"{where}: an indented line continues an entry")
            current[-1] += "\n" + line
        elif line.strip():
            raise ValueError(f"{where}: an entry begins with `- `, a continuation with two spaces: {line!r}")
    for kind, entries in found.items():
        if not entries:
            raise ValueError(f"{source}: `### {kind}` has no entry")
    return found


def fragment(path: Path) -> dict[str, list[str]]:
    """The sections of the fragment at `path`; `ValueError` names what is wrong."""
    if not NAME_RE.match(path.name):
        raise ValueError(f"{path}: a fragment is named in lower case with hyphens, such as `live-values-timeout.md`")
    found = sections(path.read_text(encoding="utf-8"), str(path))
    if not found:
        raise ValueError(f"{path}: a fragment has at least one section with an entry")
    return found


def fragment_paths(root: Path) -> list[Path]:
    """The fragments under `root`, by name."""
    folder = root / FRAGMENTS
    if not folder.is_dir():
        return []
    return sorted(path for path in folder.iterdir() if path.is_file() and path.name != README)


def unreleased(changelog: str) -> tuple[int, int]:
    """The line range of `## [Unreleased]`'s body in `changelog`."""
    lines = changelog.splitlines()
    start = next((i for i, line in enumerate(lines) if line.startswith("## [Unreleased]")), None)
    if start is None:
        raise ValueError("CHANGELOG.md has no `## [Unreleased]` section")
    end = next((i for i in range(start + 1, len(lines)) if VERSION_RE.match(lines[i])), len(lines))
    return start + 1, end


def check(root: Path) -> list[str]:
    """What is wrong with the fragments under `root` and with its `CHANGELOG.md`."""
    problems = []
    for path in fragment_paths(root):
        try:
            fragment(path)
        except ValueError as error:
            problems.append(str(error))
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    try:
        start, end = unreleased(changelog)
    except ValueError as error:
        return problems + [str(error)]
    if any(line.strip() for line in changelog.splitlines()[start:end]):
        problems.append(
            f"CHANGELOG.md: `## [Unreleased]` stays empty; put the entries in a fragment under {FRAGMENTS}/ "
            "(see changelog_fragments.py)"
        )
    return problems


def gather(changelog: str, fragments: list[dict[str, list[str]]], version: str, day: str) -> str:
    """`changelog` with `## [<version>] - <day>` under an empty `## [Unreleased]`: each section's
    entries from `fragments`, given the newest first, then those still under `## [Unreleased]`."""
    start, end = unreleased(changelog)
    lines = changelog.splitlines()
    kept = sections("\n".join(lines[start:end]), "CHANGELOG.md [Unreleased]")
    body = []
    for kind in SECTIONS:
        entries = [entry for found in fragments for entry in found.get(kind, [])] + kept.get(kind, [])
        if entries:
            body += [f"### {kind}", "", *entries, ""]
    if not body:
        raise ValueError(f"nothing to release: no fragment under {FRAGMENTS}/ and no entry under `## [Unreleased]`")
    head = lines[:start]
    released = ["", f"## [{version}] - {day}", "", *body]
    rest = lines[end:]
    return "\n".join(head + released + rest) + "\n"


def main(argv: list[str]) -> int:
    if argv != ["check"]:
        print(__doc__, file=sys.stderr)
        return 2
    problems = check(Path.cwd())
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        return 1
    print(f"changelog fragments: {len(fragment_paths(Path.cwd()))}, all read; `## [Unreleased]` is empty")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
