"""Focused gate/fuzz join tests for the Phase 17 tooling denominator."""

from __future__ import annotations

import unittest

from scripts.verification.code_spec_common import CodeFact
from scripts.verification.code_spec_tooling_join import join_tooling_inventories


def fact(
    code_id: str,
    path: str,
    *,
    owner: str = "verification",
    area: str = "verification",
) -> CodeFact:
    return CodeFact(
        code_id=code_id,
        source_kind="python" if path.endswith(".py") else "rust",
        path=path,
        qualified_symbol=path,
        item_kind="python_entrypoint" if path.endswith(".py") else "rust_function",
        signature="entrypoint",
        signature_digest="sha256:" + "1" * 64,
        body_digest="sha256:" + "2" * 64,
        owner=owner,
        area=area,
    )


class ToolingInventoryJoinTests(unittest.TestCase):
    def test_gate_and_cargo_fuzz_paths_bind_existing_ids_without_duplicates(self) -> None:
        gate = fact("CODE_" + "A" * 64, "scripts/check_gate.py")
        fuzz = fact("CODE_" + "B" * 64, "fuzz/fuzz_targets/syntax_parse.rs")

        result = join_tooling_inventories(
            facts={gate.code_id: gate, fuzz.code_id: fuzz},
            source_paths=(gate.path, fuzz.path),
            gate_records={
                "GATE_CHECK": {
                    "source_kind": "gate_script",
                    "path": gate.path,
                }
            },
            fuzz_targets=[
                {
                    "id": "FUZZ_TARGET_SYNTAX_PARSE",
                    "target_kind": "cargo_fuzz",
                    "path": fuzz.path,
                    "surface_associations": [
                        {"surface_id": "st_lexer_parser", "strength": "direct"}
                    ],
                }
            ],
            fuzz_surfaces=[
                {"id": "st_lexer_parser", "area": "verification"}
            ],
            area_records=[
                {"id": "verification", "owner": "verification"}
            ],
        )

        self.assertEqual((), result.failures)
        self.assertEqual(("gate:GATE_CHECK",), result.bindings[gate.code_id])
        self.assertEqual(("fuzz:FUZZ_TARGET_SYNTAX_PARSE",), result.bindings[fuzz.code_id])

    def test_unregistered_fuzz_source_and_gate_without_fact_fail_closed(self) -> None:
        fuzz = fact("CODE_" + "B" * 64, "fuzz/fuzz_targets/unregistered.rs")

        result = join_tooling_inventories(
            facts={fuzz.code_id: fuzz},
            source_paths=(fuzz.path, "scripts/missing_gate.py"),
            gate_records={
                "GATE_MISSING": {
                    "source_kind": "gate_script",
                    "path": "scripts/missing_gate.py",
                }
            },
            fuzz_targets=[],
            fuzz_surfaces=[],
            area_records=[],
        )

        self.assertTrue(any("unregistered cargo-fuzz source" in item for item in result.failures))
        self.assertTrue(any("gate inventory path has no tooling fact" in item for item in result.failures))

    def test_cargo_fuzz_fact_must_match_authoritative_surface_area_and_owner(self) -> None:
        fuzz = fact(
            "CODE_" + "C" * 64,
            "fuzz/fuzz_targets/lsp_incremental.rs",
            owner="language",
            area="compiler_iec",
        )

        result = join_tooling_inventories(
            facts={fuzz.code_id: fuzz},
            source_paths=(fuzz.path,),
            gate_records={},
            fuzz_targets=[
                {
                    "id": "FUZZ_TARGET_LSP_INCREMENTAL",
                    "target_kind": "cargo_fuzz",
                    "path": fuzz.path,
                    "surface_associations": [
                        {"surface_id": "lsp_incremental_edits", "strength": "direct"}
                    ],
                }
            ],
            fuzz_surfaces=[
                {"id": "lsp_incremental_edits", "area": "editor_safety"}
            ],
            area_records=[
                {"id": "compiler_iec", "owner": "language"},
                {"id": "editor_safety", "owner": "editor"},
            ],
        )

        self.assertTrue(
            any("authoritative area editor_safety" in item for item in result.failures)
        )
        self.assertTrue(
            any("authoritative owner editor" in item for item in result.failures)
        )


if __name__ == "__main__":
    unittest.main()
