#!/usr/bin/env python3
"""Every example README teaches one thing, step by step (specification 25 §3.5; checklist G21).

An example's `README.md` has, in this order: its title, one or two sentences on what you do and
what it teaches, then exactly the sections `## What you learn`, `## What you need`,
`## Step by step` (with numbered steps), `## What happened`, `## Try it yourself` and
`## Reference`. A folder README that only lists the examples beneath it is an index: its title,
one or two sentences and a table of its examples. The control course (`examples/control/`)
follows its lesson template; the catalog (`examples/README.md`), the archive and the manuals of
an example's vendored libraries (`<example>/libraries/`) are not examples.

`scripts/example_readmes.json` names the indexes and the pending READMEs, which are not converted
yet. The pending list may only shrink: a listed README that passes fails here until it is taken
off the list, and so does a listed path that does not exist.

Usage: python3 scripts/check_example_readmes.py [--root DIR]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

SECTIONS = (
    "What you learn",
    "What you need",
    "Step by step",
    "What happened",
    "Try it yourself",
    "Reference",
)
LISTS = Path("scripts") / "example_readmes.json"
SECTION = re.compile(r"^## (.+?)\s*$", re.M)
FENCE = re.compile(r"^(```|~~~).*?^\1", re.M | re.S)
NUMBERED_STEP = re.compile(r"^\d+\. \S", re.M)
TABLE_SEPARATOR = re.compile(r"^\|\s*:?-{3,}", re.M)


def checked_readmes(root: Path) -> list[str]:
    """The README paths under `examples/` that are examples or indexes, relative to `root`."""
    examples = root / "examples"
    found = []
    for path in sorted(examples.rglob("README.md")):
        parts = path.relative_to(examples).parts
        if len(parts) == 1 or parts[0] in ("control", "archive") or "libraries" in parts[1:-1]:
            continue
        found.append(path.relative_to(root).as_posix())
    return found


def opening_problems(text: str) -> list[str]:
    """The title and the sentences that open every README: a `# ` title first, then text before any section."""
    lines = text.splitlines()
    if not lines or not lines[0].startswith("# "):
        return ["does not start with a '# ' title"]
    for line in lines[1:]:
        if line.startswith("## "):
            break
        if line.strip() and not line.startswith("#"):
            return []
    return ["has no sentence between its title and its first section"]


def walkthrough_problems(text: str) -> list[str]:
    """What a walkthrough README lacks against the standard of specification 25 §3.5."""
    problems = opening_problems(text)
    prose = FENCE.sub("", text)
    sections = SECTION.findall(prose)
    if tuple(sections) != SECTIONS:
        missing = [name for name in SECTIONS if name not in sections]
        extra = [name for name in sections if name not in SECTIONS]
        if missing:
            problems.append("lacks the section(s) " + ", ".join(f"'## {name}'" for name in missing))
        if extra:
            problems.append("has the section(s) " + ", ".join(f"'## {name}'" for name in extra) + ", which the standard does not have")
        if not missing and not extra:
            problems.append("has its sections out of order; the order is " + ", ".join(SECTIONS))
    if "Step by step" in sections:
        start = prose.index("## Step by step")
        following = SECTION.search(prose, start + len("## Step by step"))
        steps = prose[start : following.start() if following else len(prose)]
        if not NUMBERED_STEP.search(steps):
            problems.append("has no numbered step under '## Step by step'")
    return problems


def index_problems(text: str) -> list[str]:
    """What an index README lacks: its title, its sentences and a table of the examples beneath it."""
    problems = opening_problems(text)
    if not TABLE_SEPARATOR.search(FENCE.sub("", text)):
        problems.append("is an index without a table of its examples")
    return problems


def findings(root: Path) -> list[str]:
    """Every README that breaks the standard, and every entry of the lists that is out of date."""
    lists = json.loads((root / LISTS).read_text(encoding="utf-8"))
    pending = set(lists.get("pending", []))
    indexes = set(lists.get("indexes", []))
    readmes = checked_readmes(root)
    found = []
    for listed in sorted((pending | indexes) - set(readmes)):
        found.append(f"{LISTS.as_posix()}: lists '{listed}', which is no checked example README")
    for listed in sorted(pending & indexes):
        found.append(f"{LISTS.as_posix()}: lists '{listed}' as pending and as an index")
    for path in readmes:
        text = (root / path).read_text(encoding="utf-8")
        problems = index_problems(text) if path in indexes else walkthrough_problems(text)
        if path in pending:
            if not problems:
                found.append(f"{path}: follows the standard now; take it off the pending list in {LISTS.as_posix()}")
            continue
        found.extend(f"{path}: {problem}" for problem in problems)
    return found


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    found = findings(args.root)
    for finding in found:
        print(finding, file=sys.stderr)
    if found:
        print(f"{len(found)} example README finding(s); the standard is specification 25 §3.5", file=sys.stderr)
        return 1
    lists = json.loads((args.root / LISTS).read_text(encoding="utf-8"))
    print(
        f"check_example_readmes: every converted example README follows the standard; "
        f"{len(lists.get('pending', []))} still pending"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
