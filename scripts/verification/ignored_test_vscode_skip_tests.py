"""Focused contracts for unsupported VS Code skip discovery."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from .ignored_test_discovery import (
    IgnoredDiscoveryBatch,
    discover_excluded_node_skip_markers,
    discover_playwright_skips,
    discover_vscode_unsupported_skip_markers,
)
from .ignored_test_live import build_live_inventory_state
from .ignored_test_models import InventoryDiagnostic
from .ignored_test_report import build_inventory_payload
from .test_catalog_common import make_fact


class IgnoredTestVscodeSkipTests(unittest.TestCase):
    def test_unsupported_skip_forms_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "editors/vscode/src/test/suite/unsupported.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text(
                'describe.skip("suite", () => {});\n'
                'suite.skip("suite alias", () => {});\n'
                'context.skip("context alias", () => {});\n'
                'xdescribe("legacy suite", () => {});\n'
                'xit("legacy test", () => {});\n'
                'test.skip(dynamicTitle, () => {});\n'
                'it.skip(makeTitle(), () => {});\n'
            )

            batch = discover_vscode_unsupported_skip_markers(root)

        self.assertEqual(batch.scanned_files, 1)
        self.assertEqual(
            batch.input_paths,
            {"editors/vscode/src/test/suite/unsupported.test.ts"},
        )
        self.assertEqual(
            [item.line for item in batch.diagnostics],
            [1, 2, 3, 4, 5, 6, 7],
        )
        self.assertEqual(
            {item.kind for item in batch.diagnostics},
            {"unsupported_vscode_skip_requires_identity_support"},
        )
        self.assertEqual(
            {item.severity for item in batch.diagnostics},
            {"error"},
        )

    def test_supported_skips_and_lexical_decoys_are_not_duplicated(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "editors/vscode/src/test/suite/supported.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text(
                '// describe.skip("comment", () => {});\n'
                '/* xit("block comment", () => {}); */\n'
                'const text = "suite.skip(\\\"string\\\", () => {})";\n'
                "const legacy = 'xdescribe(\\\"string\\\", () => {})';\n"
                'const template = `context.skip("template", () => {})`;\n'
                'const matcher = /it\\.skip\\(/;\n'
                '/*\n'
                'test.skip\n'
                '("multiline block comment", () => {});\n'
                '*/\n'
                'const multilineTemplate = `\n'
                'describe.skip\n'
                '("multiline template", () => {});\n'
                '`;\n'
                'test.skip("literal test", () => {});\n'
                'it.skip(`literal it`, () => {});\n'
                'test("runtime skip", function () { this.skip(); });\n'
            )

            batch = discover_vscode_unsupported_skip_markers(root)

        self.assertEqual(batch.scanned_files, 1)
        self.assertEqual(batch.diagnostics, [])

    def test_multiline_vscode_skip_candidates_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "editors/vscode/src/test/suite/multiline.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text(
                'test.skip\n'
                '("literal title", () => {});\n'
                'it.skip /* comment between call and arguments */\n'
                '(dynamicTitle, () => {});\n'
                'describe.skip\n'
                '("suite title", () => {});\n'
                'this.skip ();\n'
                'this.skip\n'
                '();\n'
            )

            batch = discover_vscode_unsupported_skip_markers(root)

        self.assertEqual(
            [item.line for item in batch.diagnostics],
            [1, 3, 5, 7, 8],
        )
        self.assertEqual(
            {item.severity for item in batch.diagnostics},
            {"error"},
        )

    def test_multiline_playwright_skip_candidates_fail_closed_without_lexical_decoys(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "scripts/captures/example.spec.mjs"
            source.parent.mkdir(parents=True)
            (root / "scripts/captures/package.json").write_text(
                '{"name":"trust-doc-captures"}\n'
            )
            source.write_text(
                'test.skip\n'
                '("literal title", async () => {});\n'
                'test.describe.skip /* comment */\n'
                '("suite title", () => {});\n'
                '/* test.skip\n'
                '("block comment", () => {}); */\n'
                'const text = `test.skip\n'
                '("template", () => {})`;\n'
            )

            batch = discover_playwright_skips(root)

        self.assertEqual(batch.facts, [])
        self.assertEqual(
            [(item.kind, item.line, item.severity) for item in batch.diagnostics],
            [
                ("dynamic_playwright_skip", 1, "error"),
                ("dynamic_playwright_skip", 3, "error"),
            ],
        )

    def test_multiline_excluded_node_skip_fails_closed_without_lexical_decoys(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "other/tests/multiline.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text(
                '/* test.skip\n'
                '("block comment", () => {}); */\n'
                'const text = `describe.skip\n'
                '("template", () => {})`;\n'
                'test.skip\n'
                '("silent", () => {});\n'
            )

            batch = discover_excluded_node_skip_markers(root)

        self.assertEqual(
            [(item.kind, item.line, item.severity) for item in batch.diagnostics],
            [("excluded_node_skip_requires_identity_support", 5, "error")],
        )

    def test_vscode_sentinel_count_covers_files_without_scanner_facts(self) -> None:
        scanner_fact = make_fact(
            source_kind="vscode_test",
            name="registered",
            path="editors/vscode/src/test/suite/registered.test.ts",
            line=1,
            package="trust-vscode",
            command_hint="cd editors/vscode && npm test",
            command_hint_authority="package_only",
            discovery_confidence="literal_call",
        )

        analysis = build_inventory_payload(
            scanner_facts=[scanner_fact],
            scanner_diagnostics=[],
            playwright_facts=[],
            playwright_diagnostics=[],
            vscode_scanned_files=2,
        )

        node = next(item for item in analysis.surface_summary if item["surface"] == "node")
        self.assertEqual(node["scanned_files"], 2)

    def test_live_report_aborts_on_unsupported_vscode_skip(self) -> None:
        scanner = SimpleNamespace(
            to_dict=lambda: {"scan_status": "complete"},
            provenance=SimpleNamespace(
                commit="a" * 40,
                timestamp="2026-07-10T00:00:00Z",
                platform="linux-test",
                input_paths=(),
            ),
            inferred_facts=(),
            diagnostics=(),
        )
        unsupported = IgnoredDiscoveryBatch(
            diagnostics=[
                InventoryDiagnostic(
                    "error",
                    "unsupported_vscode_skip_requires_identity_support",
                    "editors/vscode/src/test/suite/example.test.ts",
                    1,
                    "describe.skip has no modeled stable test identity",
                )
            ],
            scanned_files=1,
        )
        empty = IgnoredDiscoveryBatch()

        with (
            patch(
                "scripts.verification.ignored_test_live.scan_repository",
                return_value=scanner,
            ),
            patch(
                "scripts.verification.ignored_test_live.discover_vscode_unsupported_skip_markers",
                return_value=unsupported,
            ),
            patch(
                "scripts.verification.ignored_test_live.discover_playwright_skips",
                return_value=empty,
            ),
            patch(
                "scripts.verification.ignored_test_live.discover_excluded_rust_ignore_markers",
                return_value=empty,
            ),
            patch(
                "scripts.verification.ignored_test_live.discover_excluded_node_skip_markers",
                return_value=empty,
            ),
        ):
            with self.assertRaisesRegex(
                ValueError, "ignored-test discovery produced error diagnostics"
            ):
                build_live_inventory_state(Path.cwd())


if __name__ == "__main__":
    unittest.main()
