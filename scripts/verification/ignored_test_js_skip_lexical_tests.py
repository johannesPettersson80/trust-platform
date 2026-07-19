"""Regression contracts for fail-closed JavaScript skip discovery."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from .ignored_test_discovery import (
    discover_excluded_node_skip_markers,
    discover_playwright_skips,
    discover_vscode_unsupported_skip_markers,
)


class IgnoredTestJsSkipLexicalTests(unittest.TestCase):
    def test_midline_playwright_literal_is_discovered(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "scripts/captures/midline.spec.mjs"
            source.parent.mkdir(parents=True)
            (source.parent / "package.json").write_text(
                '{"name":"trust-doc-captures"}\n'
            )
            source.write_text(
                'const probe = 1; test.skip("mid-line probe", async () => {});\n'
            )

            batch = discover_playwright_skips(root)

        self.assertEqual(batch.diagnostics, [])
        self.assertEqual(
            [(fact.name, fact.line, fact.ignore_mechanism) for fact in batch.facts],
            [("mid-line probe", 1, "playwright_literal_skip")],
        )

    def test_split_member_skip_forms_fail_closed_across_js_surfaces(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            playwright = root / "scripts/captures/split.spec.mjs"
            playwright.parent.mkdir(parents=True)
            (playwright.parent / "package.json").write_text(
                '{"name":"trust-doc-captures"}\n'
            )
            playwright.write_text('test\n.skip("playwright", async () => {});\n')

            vscode = root / "editors/vscode/src/test/suite/split.test.ts"
            vscode.parent.mkdir(parents=True)
            vscode.write_text('it\n.skip("vscode", () => {});\n')

            excluded = root / "other/tests/split.test.ts"
            excluded.parent.mkdir(parents=True)
            excluded.write_text(
                'test\n.skip("node test", () => {});\n'
                'it\n.skip("node it", () => {});\n'
            )

            playwright_batch = discover_playwright_skips(root)
            vscode_batch = discover_vscode_unsupported_skip_markers(root)
            excluded_batch = discover_excluded_node_skip_markers(root)

        self.assertEqual(playwright_batch.facts, [])
        self.assertEqual(
            [(item.kind, item.line) for item in playwright_batch.diagnostics],
            [("dynamic_playwright_skip", 1)],
        )
        self.assertEqual(
            [(item.kind, item.line) for item in vscode_batch.diagnostics],
            [("unsupported_vscode_skip_requires_identity_support", 1)],
        )
        self.assertEqual(
            [(item.kind, item.line) for item in excluded_batch.diagnostics],
            [
                ("excluded_node_skip_requires_identity_support", 1),
                ("excluded_node_skip_requires_identity_support", 3),
            ],
        )

    def test_excluded_node_named_skip_vocabulary_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "other/tests/named.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text(
                'xit("legacy test", () => {});\n'
                'xdescribe("legacy suite", () => {});\n'
                'suite.skip("suite alias", () => {});\n'
                'context.skip("context alias", () => {});\n'
            )

            batch = discover_excluded_node_skip_markers(root)

        self.assertEqual(
            [(item.kind, item.line) for item in batch.diagnostics],
            [
                ("excluded_node_skip_requires_identity_support", 1),
                ("excluded_node_skip_requires_identity_support", 2),
                ("excluded_node_skip_requires_identity_support", 3),
                ("excluded_node_skip_requires_identity_support", 4),
            ],
        )

    def test_comments_and_literals_do_not_create_skip_observations(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            playwright = root / "scripts/captures/decoys.spec.mjs"
            playwright.parent.mkdir(parents=True)
            (playwright.parent / "package.json").write_text(
                '{"name":"trust-doc-captures"}\n'
            )
            playwright.write_text(_decoy_source())

            vscode = root / "editors/vscode/src/test/suite/decoys.test.ts"
            vscode.parent.mkdir(parents=True)
            vscode.write_text(_decoy_source())

            excluded = root / "other/tests/decoys.test.ts"
            excluded.parent.mkdir(parents=True)
            excluded.write_text(_decoy_source())

            playwright_batch = discover_playwright_skips(root)
            vscode_batch = discover_vscode_unsupported_skip_markers(root)
            excluded_batch = discover_excluded_node_skip_markers(root)

        self.assertEqual(playwright_batch.facts, [])
        self.assertEqual(playwright_batch.diagnostics, [])
        self.assertEqual(vscode_batch.diagnostics, [])
        self.assertEqual(excluded_batch.diagnostics, [])


def _decoy_source() -> str:
    return (
        '// const value = 1; test.skip("comment", () => {});\n'
        '/* test\n.skip("block comment", () => {}); */\n'
        'const text = "xit(\\\"string\\\", () => {})";\n'
        "const other = 'suite.skip(\\\"string\\\", () => {})';\n"
        'const template = `context\n.skip("template", () => {})`;\n'
        'const matcher = /xdescribe|it\\s*\\.\\s*skip\\s*\\(/;\n'
    )


if __name__ == "__main__":
    unittest.main()
