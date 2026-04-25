#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path


REPO_ROOT = Path.cwd()
DOCS_ROOT = REPO_ROOT / "docs/public"
MKDOCS_YML = REPO_ROOT / "mkdocs.yml"

LINK_RE = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")
SNIPPET_RE = re.compile(r'--8<--\s+"([^"]+)"')
NAV_MD_RE = re.compile(r":\s*([^#\n]+?\.md)\s*$")
LIST_RE = re.compile(r"^([-*+]|\d+\.)\s+")
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


def main() -> int:
    failures: list[str] = []
    check_public_links_in_nav(failures)
    check_snippet_h1_collisions(failures)
    check_blank_line_before_colon_lists(failures)
    check_public_filler_phrases(failures)

    if failures:
        print("public docs IA check failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("public docs IA check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
