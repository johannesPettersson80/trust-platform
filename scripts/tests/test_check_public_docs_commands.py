"""The documentation names only commands and settings the extension has (checklist G16). Each expected
finding follows from the page and the package each test writes."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_public_docs_commands as check  # noqa: E402

PACKAGE = {
    "contributes": {
        "commands": [
            {"command": "trust-lsp.newProject", "category": "truST", "title": "Create Project"},
            {"command": "trust-lsp.networkCanvas.open", "category": "truST", "title": "Open Devices & Connections"},
            {"command": "trust-lsp.hmiBuilder.connectSimulator", "title": "truST: Connect Named HMI Simulator Journey"},
        ],
        "configuration": [{"properties": {"trust.runtime.mode": {"type": "string"}}}],
    }
}


def root_with(pages: dict[str, str]) -> tuple[tempfile.TemporaryDirectory, Path]:
    holder = tempfile.TemporaryDirectory()
    root = Path(holder.name)
    (root / "editors" / "vscode").mkdir(parents=True)
    (root / "editors" / "vscode" / "package.json").write_text(json.dumps(PACKAGE))
    for name, text in pages.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return holder, root


class DocsCommandNamesTests(unittest.TestCase):
    def test_a_named_command_must_be_a_palette_title(self) -> None:
        # Catches a page left behind by a renamed command: "truST: Devices & Connections" stayed in two
        # pages after the command became "Open Devices & Connections" (30 October 2026).
        holder, root = root_with(
            {
                "README.md": "Run **truST: Create Project** first.\n",
                "docs/public/a.md": "Text.\n\n1. Open **truST: Devices & Connections**.\n",
                "examples/x/README.md": "run **truST: Connect Named HMI Simulator Journey**\n",
            }
        )
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "docs/public/a.md:3: names the command 'truST: Devices & Connections', "
                    "which the extension does not have"
                ],
            )

    def test_a_title_broken_over_two_lines_is_one_name(self) -> None:
        holder, root = root_with({"README.md": "run **truST: Open Devices &\nConnections** now\n"})
        with holder:
            self.assertEqual(check.findings(root), [])

    def test_other_text_is_not_a_command_and_settings_are_checked_by_key(self) -> None:
        # The status bar's words in backticks are not a command; a setting key is checked against the
        # package's configuration.
        holder, root = root_with(
            {
                "README.md": "The status bar shows `truST: Simulator running`.\n",
                "docs/public/b.md": "Set `trust.runtime.mode` and `trust.runtime.missing`.\n",
            }
        )
        with holder:
            self.assertEqual(
                check.findings(root),
                ["docs/public/b.md:1: names the setting 'trust.runtime.missing', which the extension does not have"],
            )

    def test_a_palette_step_names_an_existing_command(self) -> None:
        # Catches a palette step typing a title the extension no longer has, in backticks or as plain
        # text in a code block: `Structured Text: Open Runtime Panel` sent readers to nothing
        # (30 October 2026).
        holder, root = root_with(
            {
                "examples/a/README.md": "1. `Ctrl/Cmd+Shift+P` -> `truST: Create Project`.\n"
                "2. `Ctrl/Cmd+Shift+P` -> `truST: Open Runtime Panel`\n",
                "examples/b/README.md": "```\nCtrl/Cmd+Shift+P → truST: Open Devices & Connections\n```\n",
            }
        )
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "examples/a/README.md:2: names the command 'truST: Open Runtime Panel', "
                    "which the extension does not have",
                    "examples/a/README.md:2: names the Runtime Panel, which the extension retired; "
                    "it is Live Values and the truST view",
                ],
            )

    def test_retired_names_fail_wherever_a_page_uses_them(self) -> None:
        # Catches the retired command category and the Runtime Panel and pane, in any case; the page
        # file `debugging-and-runtime-panel.md` keeps its name, so a link to it is no finding.
        holder, root = root_with(
            {
                "docs/public/c.md": "Use `Structured Text: Compile`, then the runtime panel.\n"
                "See [Run](operate/debugging-and-runtime-panel.md) in the Runtime pane.\n",
            }
        )
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "docs/public/c.md:1: names 'Structured Text: Compile', a command category the extension "
                    "retired; its commands are 'truST:' ones",
                    "docs/public/c.md:1: names the runtime panel, which the extension retired; it is Live Values "
                    "and the truST view",
                    "docs/public/c.md:2: names the Runtime pane, which the extension retired; it is Live Values "
                    "and the truST view",
                ],
            )

    def test_the_repository_names_only_existing_commands_and_settings(self) -> None:
        self.assertEqual(check.findings(ROOT), [])


if __name__ == "__main__":
    unittest.main()
