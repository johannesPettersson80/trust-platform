#!/usr/bin/env python3
"""The VS Code commands and settings the documentation names exist (checklist G16).

A page names a command by its Command Palette title in bold, `**truST: Create Project**`, or as
the title a palette step types, `` `Ctrl/Cmd+Shift+P` -> `truST: Compile` ``, and a setting by
its key in backticks, `` `trust.runtime.mode` ``. Every such name in the README, the Marketplace
page (`editors/vscode/README.md`), the pages of `docs/public` and the examples' READMEs must be a
command title or a setting key that `editors/vscode/package.json` contributes, so a renamed
command or setting fails here instead of leaving the page behind. Other text, such as the status bar's `truST: Simulator running` in
backticks, is not a command and is not checked.

Names the extension retired fail wherever a page uses them: the command category `Structured
Text:` (its commands are `truST:` ones now) in bold or backticks, and the Runtime Panel and the
Runtime pane, which Live Values and the truST view replaced (30 October 2026: 15 pages still sent
readers to them).

Usage: python3 scripts/check_public_docs_commands.py [--root DIR]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

CATEGORIES = r"(?:truST HMI|truST|Control Studio|Structured Text)"
COMMAND = re.compile(rf"\*\*({CATEGORIES}: [^*]+?)\*\*", re.S)
# A palette step: the key, an arrow, and the title it types, with or without backticks.
PALETTE = re.compile(rf"(?:Ctrl|Cmd)(?:/(?:Cmd|Ctrl))?\+Shift\+P`?\s*(?:->|→)\s*`?({CATEGORIES}: [^`\n]+?)`?\.?\s*$", re.M)
SETTING = re.compile(r"`(trust\.[A-Za-z0-9_.]+)`")
RETIRED_CATEGORY = re.compile(r"`(Structured Text: [^`\n]*)`")
RETIRED_SURFACES = re.compile(r"\b(runtime panel|runtime pane)\b", re.I)


def palette_titles(package: dict) -> set[str]:
    """Every command's Command Palette title: its category, a colon and its title."""
    titles = set()
    for command in package.get("contributes", {}).get("commands", []):
        title = command.get("title", "")
        category = command.get("category")
        titles.add(f"{category}: {title}" if category else title)
    return titles


def setting_keys(package: dict) -> set[str]:
    configuration = package.get("contributes", {}).get("configuration", [])
    blocks = configuration if isinstance(configuration, list) else [configuration]
    return {key for block in blocks for key in block.get("properties", {})}


def pages(root: Path) -> list[Path]:
    found = [root / "README.md", root / "editors" / "vscode" / "README.md"]
    found += sorted((root / "docs" / "public").rglob("*.md"))
    found += sorted(
        path
        for path in (root / "examples").rglob("README*.md")
        if "node_modules" not in path.parts
    )
    return [path for path in found if path.is_file()]


def findings(root: Path) -> list[str]:
    package = json.loads((root / "editors" / "vscode" / "package.json").read_text(encoding="utf-8"))
    titles = palette_titles(package)
    settings = setting_keys(package)
    found = []
    for page in pages(root):
        text = page.read_text(encoding="utf-8", errors="replace")
        shown = page.relative_to(root)
        named = [*COMMAND.finditer(text), *PALETTE.finditer(text)]
        for match in sorted(named, key=lambda match: match.start()):
            name = " ".join(match.group(1).split())
            if name not in titles:
                line = text.count("\n", 0, match.start()) + 1
                found.append(f"{shown}:{line}: names the command '{name}', which the extension does not have")
        for match in RETIRED_CATEGORY.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            found.append(
                f"{shown}:{line}: names '{match.group(1)}', a command category the extension retired; "
                "its commands are 'truST:' ones"
            )
        for match in RETIRED_SURFACES.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            found.append(
                f"{shown}:{line}: names the {match.group(1)}, which the extension retired; "
                "it is Live Values and the truST view"
            )
        for match in SETTING.finditer(text):
            key = match.group(1)
            if key not in settings:
                line = text.count("\n", 0, match.start()) + 1
                found.append(f"{shown}:{line}: names the setting '{key}', which the extension does not have")
    return found


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    found = findings(args.root)
    for finding in found:
        print(finding, file=sys.stderr)
    if found:
        print(f"{len(found)} command or setting name(s) in the documentation do not exist", file=sys.stderr)
        return 1
    print("check_public_docs_commands: every named command and setting exists")
    return 0


if __name__ == "__main__":
    sys.exit(main())
