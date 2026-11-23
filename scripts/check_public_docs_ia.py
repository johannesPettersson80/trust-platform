#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path


REPO_ROOT = Path.cwd()
DOCS_ROOT = REPO_ROOT / "docs/public"
MKDOCS_YML = REPO_ROOT / "mkdocs.yml"

# A link's text holds only balanced brackets, as in CommonMark: a lone bracket such as the
# shortcut Ctrl+[ starts no link.
LINK_RE = re.compile(r"(?<!!)\[(?:[^\[\]]|\[[^\[\]]*\])+\]\(([^)]+)\)")
SNIPPET_RE = re.compile(r'--8<--\s+"([^"]+)"')
NAV_MD_RE = re.compile(r":\s*([^#\n]+?\.md)\s*$")
LIST_RE = re.compile(r"^([-*+]|\d+\.)\s+")
# A top-level numbered list item, a step: its number, and the spaces before its text.
TOP_STEP_RE = re.compile(r"^(\d+\.)( +)\S")
# A line that starts a block of its own inside a list item: a fence, a table row or a nested item.
BLOCK_START_RE = re.compile(r"^ *(```|~~~|\||(\d+\.|[-*+]) )")
# The indentation that Python-Markdown, which builds the site, nests a list item's blocks at.
LIST_BLOCK_INDENT = 4
FILLER_PATTERNS = [
    (
        re.compile(r"\buse this (page|guide)\s+(for|when|before|as)\b", re.I),
        "avoid generic 'use this page/guide' routing prose",
    ),
    (re.compile(r"\buse it when\b", re.I), "avoid generic 'use it when' prose"),
    (re.compile(r"\buse this when\b", re.I), "avoid generic 'use this when' prose"),
    (re.compile(r"^#+\s*use this\b", re.I), "avoid 'Use this' section headings"),
    (re.compile(r"\bafter reading\b", re.I), "avoid boilerplate outcome prose"),
    (
        re.compile(r"\byou should (leave|be able|know)\b", re.I),
        "avoid boilerplate outcome prose",
    ),
    (re.compile(r"^#+\s*why it matters\b", re.I), "use concrete section headings"),
    (re.compile(r"^#+\s*start here if\b", re.I), "use concrete section headings"),
    (
        re.compile(r"\bthe guide below teaches\b", re.I),
        "avoid wrapper-page filler",
    ),
    (
        re.compile(r"\bbudget\s+\d+\s*-?\s*\d*\s+minutes\b", re.I),
        "avoid generic time-budget filler",
    ),
    (re.compile(r"\bthis public entry\b", re.I), "use direct reference prose"),
    (re.compile(r"\bthis page points\b", re.I), "use direct reference prose"),
    (re.compile(r"\bthis page documents\b", re.I), "use direct reference prose"),
    (re.compile(r"\bthis page renders\b", re.I), "use direct reference prose"),
]
EXTERNAL_SYSTEM_PROTOCOLS = {
    "Modbus TCP": ("Modbus TCP",),
    "MQTT": ("MQTT",),
    "OPC UA": ("OPC UA",),
    "Beckhoff ADS": ("Beckhoff ADS", "ADS"),
}
PROTOCOL_OVERVIEW_PAGES = [
    "concepts/communication-planes.md",
    "connect/protocol-matrix.md",
    "connect/add-a-device.md",
    "connect/external-systems/index.md",
]


def nav_paths() -> set[str]:
    paths: set[str] = set()
    in_nav = False
    for raw_line in MKDOCS_YML.read_text(encoding="utf-8").splitlines():
        if raw_line.strip() == "nav:":
            in_nav = True
            continue
        if not in_nav:
            continue
        match = NAV_MD_RE.search(raw_line)
        if not match:
            continue
        target = match.group(1).strip().strip('"').strip("'")
        paths.add(target)
    return paths


def not_in_nav_paths() -> set[str]:
    patterns: list[str] = []
    in_block = False
    for raw_line in MKDOCS_YML.read_text(encoding="utf-8").splitlines():
        if raw_line.strip() == "not_in_nav: |":
            in_block = True
            continue
        if not in_block:
            continue
        if raw_line and not raw_line.startswith((" ", "\t")):
            break
        stripped = raw_line.strip()
        if stripped:
            patterns.append(stripped)

    hidden: set[str] = set()
    for pattern in patterns:
        matches = DOCS_ROOT.glob(pattern)
        hidden.update(path.relative_to(DOCS_ROOT).as_posix() for path in matches)
    return hidden


def normalize_public_target(source: Path, raw_target: str) -> str | None:
    target = raw_target.strip().strip("<>")
    if not target or target.startswith("#"):
        return None
    if "://" in target or target.startswith("mailto:"):
        return None
    target = target.split("#", 1)[0].strip()
    if not target:
        return None
    path = (source.parent / target).resolve()
    if path.is_dir():
        path = path / "index.md"
    try:
        return path.relative_to(DOCS_ROOT.resolve()).as_posix()
    except ValueError:
        return None


def check_public_links_in_nav(failures: list[str]) -> None:
    nav = nav_paths()
    hidden = not_in_nav_paths()
    for source in sorted(DOCS_ROOT.rglob("*.md")):
        text = source.read_text(encoding="utf-8")
        for raw_target in LINK_RE.findall(text):
            normalized = normalize_public_target(source, raw_target)
            if normalized is None:
                continue
            if normalized not in nav and normalized not in hidden:
                failures.append(
                    f"{source}: linked public target is missing from nav: {raw_target}"
                )


def split_snippet_target(target: str) -> tuple[Path, str | None]:
    prefix, sep, basename = target.rpartition("/")
    name, selector_sep, selector = basename.partition(":")
    path_text = f"{prefix}{sep}{name}" if sep else name
    return REPO_ROOT / path_text, selector if selector_sep else None


def first_nonempty_line(lines: list[str]) -> tuple[int, str] | None:
    for index, line in enumerate(lines, start=1):
        if line.strip():
            return index, line.strip()
    return None


def selector_includes_line_one(selector: str | None) -> bool:
    if selector is None:
        return True
    for part in selector.split(","):
        start = part.split(":", 1)[0]
        if not start:
            return True
        if start.lstrip("-").isdigit() and int(start) <= 1:
            return True
        if not start.lstrip("-").isdigit():
            return False
    return False


def check_snippet_h1_collisions(failures: list[str]) -> None:
    for source in sorted(DOCS_ROOT.rglob("*.md")):
        text = source.read_text(encoding="utf-8")
        for match in SNIPPET_RE.finditer(text):
            target_text = match.group(1)
            target, selector = split_snippet_target(target_text)
            if not target.exists() or not target.is_file():
                continue
            first = first_nonempty_line(
                target.read_text(encoding="utf-8", errors="ignore").splitlines()
            )
            if first is None:
                continue
            first_line, content = first
            if content.startswith("# ") and selector_includes_line_one(selector):
                line_no = text[: match.start()].count("\n") + 1
                failures.append(
                    f"{source}:{line_no}: snippet includes H1 from {target_text}; "
                    f"start after line {first_line}"
                )


def check_blank_line_before_colon_lists(failures: list[str]) -> None:
    roots = [DOCS_ROOT, REPO_ROOT / "docs/guides"]
    for root in roots:
        for source in sorted(root.rglob("*.md")):
            lines = source.read_text(encoding="utf-8").splitlines()
            in_fence = False
            for index, line in enumerate(lines):
                stripped = line.strip()
                if stripped.startswith("```") or stripped.startswith("~~~"):
                    in_fence = not in_fence
                    continue
                if in_fence or index == 0:
                    continue
                previous = lines[index - 1].rstrip()
                if (
                    LIST_RE.match(line)
                    and previous.endswith(":")
                    and previous.strip()
                    and not previous.lstrip().startswith(("#", "|", "- ", "* "))
                ):
                    failures.append(
                        f"{source}:{index + 1}: list after colon needs a blank line"
                    )


def short_list_blocks(lines: list[str]) -> list[int]:
    """The lines, from 1, that start a block inside a top-level numbered item, a step, indented by
    less than the 4 spaces Python-Markdown needs: the site ends the list there, so the step loses
    its block and the steps after it are numbered from 1 again or run into one paragraph. GitHub
    nests a block under the step's text, 3 spaces after "1. ", and a block indented less than the
    text, such as a bullet 3 spaces under "10. ", breaks the step there too. A block starts after a
    blank line, and at a fence, a table row or a nested item; a line that continues the step's
    paragraph is no block. (A bullet's block indented by 2 is only drawn flatter, and released
    changelog sections may not change.)"""
    found: list[int] = []
    content: int | None = None
    fence = False
    # Inside a fence that a step holds: its lines, and its closing fence, start no block.
    step_fence = False
    for number, line in enumerate(lines, start=1):
        if line.startswith(("```", "~~~")):
            fence = not fence
            content = None
            step_fence = False
            continue
        if fence or not line.strip():
            continue
        indent = len(line) - len(line.lstrip(" "))
        if indent == 0:
            item = TOP_STEP_RE.match(line)
            content = len(item.group(1)) + len(item.group(2)) if item else None
            step_fence = False
            continue
        is_fence = line.lstrip(" ").startswith(("```", "~~~"))
        if step_fence:
            step_fence = not is_fence
            continue
        step_fence = is_fence
        if content is None or indent >= LIST_BLOCK_INDENT:
            continue
        if not lines[number - 2].strip() or BLOCK_START_RE.match(line):
            found.append(number)
    return found


def steps_joined_to_a_block(lines: list[str]) -> list[int]:
    """The lines, from 1, of a step written right under a step of several blocks, with no blank line
    between them. Python-Markdown splits a list into blocks at blank lines and reads such a step as
    part of the previous step's last block, such as its closing fence: the step runs into that block
    instead of starting an item of its own. GitHub starts the item either way."""
    found: list[int] = []
    fence = False
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.startswith(("```", "~~~")):
            fence = not fence
            index += 1
            continue
        if fence or not TOP_STEP_RE.match(line):
            index += 1
            continue
        end = index + 1
        while end < len(lines) and (not lines[end].strip() or lines[end].startswith(" ")):
            end += 1
        last = end - 1
        while last > index and not lines[last].strip():
            last -= 1
        several_blocks = any(not lines[k].strip() for k in range(index + 1, last))
        if (
            several_blocks
            and end < len(lines)
            and TOP_STEP_RE.match(lines[end])
            and lines[end - 1].strip()
        ):
            found.append(end + 1)
        index = end
    return found


def check_list_block_indentation(failures: list[str]) -> None:
    sources = set(DOCS_ROOT.rglob("*.md"))
    for page in sorted(DOCS_ROOT.rglob("*.md")):
        for match in SNIPPET_RE.finditer(page.read_text(encoding="utf-8")):
            target, _ = split_snippet_target(match.group(1))
            if target.suffix == ".md" and target.is_file():
                sources.add(target)
    for source in sorted(sources):
        lines = source.read_text(encoding="utf-8").splitlines()
        for number in short_list_blocks(lines):
            failures.append(
                f"{source}:{number}: a list item's block is indented less than "
                f"{LIST_BLOCK_INDENT} spaces, and the site's Markdown ends the list there; "
                f"indent the item's blocks by {LIST_BLOCK_INDENT}"
            )
        for number in steps_joined_to_a_block(lines):
            failures.append(
                f"{source}:{number}: a step follows a step of several blocks without a blank "
                f"line, and the site's Markdown runs it into that step's last block; add the "
                f"blank line"
            )


def check_public_filler_phrases(failures: list[str]) -> None:
    for source in sorted(DOCS_ROOT.rglob("*.md")):
        lines = source.read_text(encoding="utf-8").splitlines()
        in_fence = False
        for index, line in enumerate(lines, start=1):
            stripped = line.strip()
            if stripped.startswith("```") or stripped.startswith("~~~"):
                in_fence = not in_fence
                continue
            if in_fence:
                continue
            for pattern, message in FILLER_PATTERNS:
                if pattern.search(line):
                    failures.append(f"{source}:{index}: {message}: {stripped}")


def check_external_protocol_overview_sync(failures: list[str]) -> None:
    for relative_path in PROTOCOL_OVERVIEW_PAGES:
        source = DOCS_ROOT / relative_path
        if not source.exists():
            failures.append(f"{source}: required protocol overview page is missing")
            continue
        text = source.read_text(encoding="utf-8")
        for protocol, accepted_labels in EXTERNAL_SYSTEM_PROTOCOLS.items():
            if not any(label in text for label in accepted_labels):
                failures.append(
                    f"{source}: missing external-system protocol listing for {protocol}"
                )


def main() -> int:
    failures: list[str] = []
    check_public_links_in_nav(failures)
    check_snippet_h1_collisions(failures)
    check_blank_line_before_colon_lists(failures)
    check_list_block_indentation(failures)
    check_public_filler_phrases(failures)
    check_external_protocol_overview_sync(failures)

    if failures:
        print("public docs IA check failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("public docs IA check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
