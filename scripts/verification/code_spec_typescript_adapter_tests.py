"""Focused denominator tests for the Phase 17 TS/JS adapter."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification import code_spec_typescript as typescript_adapter
from scripts.verification.code_spec_typescript import (
    PRODUCT_ROOTS,
    discover_product_sources,
    invoke_typescript_extractor,
)


PACKAGING_INPUTS = getattr(typescript_adapter, "PACKAGING_INPUTS", ())


class TypeScriptAdapterTests(unittest.TestCase):
    def test_exact_product_roots_are_nonzero_and_test_geometry_is_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, product_root in enumerate(PRODUCT_ROOTS):
                directory = root / product_root
                directory.mkdir(parents=True)
                extension = "ts" if index != 1 else "js"
                (directory / f"live.{extension}").write_text("export function live() {}\n")
                (directory / f"ignored.test.{extension}").write_text("test('ignored', () => {});\n")
                (directory / "test").mkdir()
                (directory / "test" / f"helper.{extension}").write_text("export function helper() {}\n")
                (directory / "types.d.ts").write_text("export interface Shape {}\n")
            for packaging_input in PACKAGING_INPUTS:
                path = root / packaging_input
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("export default { build: { target: 'es2020' } };\n")

            sources = discover_product_sources(root)

        self.assertEqual(5, len(sources))
        self.assertEqual(
            [
                "crates/trust-runtime/src/web/ui/live.js",
                "crates/trust-runtime/web/ide-frontend/src/live.ts",
                "crates/trust-runtime/web/ide-frontend/vite.config.ts",
                "crates/trust-runtime/web/ide-frontend/vite.monaco.config.ts",
                "editors/vscode/src/live.ts",
            ],
            [source["path"] for source in sources],
        )
        self.assertEqual({"javascript", "typescript"}, {source["source_kind"] for source in sources})

    def test_adapter_builds_stable_exact_byte_facts_for_all_product_surfaces(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, product_root in enumerate(PRODUCT_ROOTS):
                directory = root / product_root
                directory.mkdir(parents=True)
                extension = "js" if index == 1 else "ts"
                (directory / f"live.{extension}").write_text(
                    "export function live(value) { return 'café' + value; }\n"
                )
            for packaging_input in PACKAGING_INPUTS:
                path = root / packaging_input
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("export default { build: { target: 'es2020' } };\n")
            matrix = {
                "areas": [{"id": "editor_safety", "owner": "editor"}],
                "code_areas": [
                    {
                        "id": "fixture",
                        "match_kind": "path",
                        "area_ids": ["editor_safety"],
                        "path_globs": ["**"],
                        "required_test_classes": [],
                        "suite_tiers": ["pr"],
                        "conditional_suite_tiers": [],
                    }
                ],
            }

            inventory = invoke_typescript_extractor(root, matrix)

        self.assertEqual(5, inventory.source_count)
        self.assertEqual(
            tuple(sorted({fact.path for fact in inventory.facts.values()})),
            inventory.source_paths,
        )
        self.assertEqual(5, len(inventory.facts))
        self.assertEqual(set(inventory.facts), set(inventory.spans))
        self.assertTrue(
            all(
                span.body_start < span.body_end
                for span in inventory.spans.values()
            )
        )
        self.assertEqual((), inventory.unmapped_code_ids)
        self.assertTrue(all(fact.area == "editor_safety" for fact in inventory.facts.values()))
        self.assertTrue(
            all(fact.area_candidates == ("editor_safety",) for fact in inventory.facts.values())
        )
        self.assertEqual(
            {"javascript_function", "typescript_asset", "typescript_function"},
            {fact.item_kind for fact in inventory.facts.values()},
        )


if __name__ == "__main__":
    unittest.main()
