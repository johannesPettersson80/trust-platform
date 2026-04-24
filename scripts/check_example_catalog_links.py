#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path


REPO_ROOT = Path.cwd()
AUDIT_PATH = REPO_ROOT / "docs/internal/testing/checklists/example-catalog-audit.md"
DOCS_EXAMPLES_ROOT = REPO_ROOT / "docs/public/examples"
DOCS_LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)|<((?:https?://)[^>]+)>")
GITHUB_REPO_LINK_RE = re.compile(
    r"^https://github\.com/johannesPettersson80/trust-platform/(tree|blob)/main/(.+)$"
)
EXAMPLE_CODE_PATH_RE = re.compile(r"`(examples/[^`]+)`")
EXAMPLE_SNIPPET_RE = re.compile(r'--8<--\s+"(examples/[^"]+)"')


def snippet_file_target(target: str) -> str:
    """Return the file part of a PyMdown snippet target.

    Snippets can select line ranges or named sections with a suffix such as
    `README.md:3` or `README.md:intro`. The catalog audit only needs to prove
    that the referenced example file exists.
    """

    return target.split(":", 1)[0]


def parse_audit_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    in_table = False
    for raw_line in AUDIT_PATH.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line.startswith("| Path | Docs category |"):
            in_table = True
            continue
        if not in_table:
            continue
        if not line.startswith("|"):
            break
        if set(line.replace("|", "").strip()) == {"-"}:
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 6:
            continue
        rows.append(
            {
                "path": cells[0].strip("`"),
                "category": cells[1].strip("`"),
                "audience": cells[2].strip("`"),
                "hardware": cells[3].strip("`"),
                "decision": cells[4].strip("`"),
                "why": cells[5],
            }
        )
    return rows


def repo_target_from_link(target: str) -> Path | None:
    match = GITHUB_REPO_LINK_RE.match(target.strip())
    if not match:
        return None
    return REPO_ROOT / match.group(2)


def readme_for_example(example_path: Path) -> Path | None:
    if example_path.is_file():
        return example_path if example_path.name == "README.md" else None
    readme = example_path / "README.md"
    return readme if readme.exists() else None


def main() -> int:
    failures: list[str] = []
    audit_available = AUDIT_PATH.exists()

    if not DOCS_EXAMPLES_ROOT.exists():
        print(f"missing docs examples root: {DOCS_EXAMPLES_ROOT}", file=sys.stderr)
        return 1

    for page in sorted(DOCS_EXAMPLES_ROOT.glob("*.md")):
        if page.name in {"index.md", "learning-paths.md"}:
            continue
        raw_links = []
        for markdown_link, autolink in DOCS_LINK_RE.findall(page.read_text(encoding="utf-8")):
            raw_links.append(markdown_link or autolink)
        repo_links = [repo_target_from_link(link) for link in raw_links]
        repo_links = [path for path in repo_links if path is not None]
        code_path_links = [
            REPO_ROOT / match
            for match in EXAMPLE_CODE_PATH_RE.findall(page.read_text(encoding="utf-8"))
        ]
        repo_links.extend(code_path_links)
        snippet_links = [
            REPO_ROOT / snippet_file_target(match)
            for match in EXAMPLE_SNIPPET_RE.findall(page.read_text(encoding="utf-8"))
        ]
        repo_links.extend(snippet_links)
        if not repo_links:
            failures.append(f"{page}: no runnable example links found")
            continue
        for target in repo_links:
            if not target.exists():
                failures.append(f"{page}: missing repo example target {target.relative_to(REPO_ROOT)}")

    if audit_available:
        for row in parse_audit_rows():
            decision = row["decision"]
            if decision not in {"keep", "tweak", "merge"}:
                continue
            example_path = REPO_ROOT / row["path"]
            if not example_path.exists():
                failures.append(f"audit row missing example path: {row['path']}")
                continue
            readme = readme_for_example(example_path)
            if readme is None:
                failures.append(f"{row['path']}: missing README.md for kept public example")
                continue
            docs_category_path = f"docs/public/examples/{row['category']}.md"
            docs_page = REPO_ROOT / docs_category_path
            if not docs_page.exists():
                failures.append(f"{row['path']}: missing docs category page {docs_category_path}")
                continue
            if docs_category_path not in readme.read_text(encoding="utf-8", errors="ignore"):
                failures.append(f"{readme.relative_to(REPO_ROOT)}: missing backlink to {docs_category_path}")
    else:
        print(
            f"example audit file not present; skipping internal audit checks: {AUDIT_PATH}",
            file=sys.stderr,
        )

    if failures:
        print("example catalog link audit failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("example catalog link audit passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
