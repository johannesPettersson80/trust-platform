"""Focused tests for the complete Phase 17 metadata join."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification import code_spec_live
from scripts.verification.code_spec_common import CodeFact
from scripts.verification.code_spec_live import (
    CodeSpecLiveError,
    load_code_spec_metadata,
    merge_fact_sets,
)


class CodeSpecLiveTests(unittest.TestCase):
    def test_fact_sets_merge_exactly_and_reject_cross_scanner_collisions(self) -> None:
        rust = CodeFact(
            code_id="CODE_RUST",
            source_kind="rust",
            path="crates/example/src/lib.rs",
            qualified_symbol="example::run",
            item_kind="rust_function",
            signature="fn run()",
            signature_digest="sha256:" + "1" * 64,
            body_digest="sha256:" + "2" * 64,
            owner="example",
            area="runtime_safety",
        )
        browser = CodeFact(
            code_id="CODE_BROWSER",
            source_kind="javascript",
            path="crates/example/ui.js",
            qualified_symbol="example::render",
            item_kind="javascript_function",
            signature="function render()",
            signature_digest="sha256:" + "3" * 64,
            body_digest="sha256:" + "4" * 64,
            owner="example",
            area="hmi_ui",
        )

        self.assertEqual(
            ["CODE_BROWSER", "CODE_RUST"],
            list(merge_fact_sets({rust.code_id: rust}, {browser.code_id: browser})),
        )
        with self.assertRaisesRegex(CodeSpecLiveError, "duplicate live code identity CODE_RUST"):
            merge_fact_sets({rust.code_id: rust}, {rust.code_id: rust})

    def test_global_area_bindings_are_checked_after_language_fact_sets_merge(self) -> None:
        validator = getattr(code_spec_live, "validate_global_area_bindings", None)
        self.assertIsNotNone(validator, "live join must own global area-binding validation")
        browser = CodeFact(
            code_id="CODE_BROWSER",
            source_kind="javascript",
            path="crates/example/ui.js",
            qualified_symbol="example::render",
            item_kind="javascript_function",
            signature="function render()",
            signature_digest="sha256:" + "3" * 64,
            body_digest="sha256:" + "4" * 64,
            owner="example",
            area="hmi_ui",
        )

        self.assertEqual([], validator({"CODE_BROWSER": "hmi_ui"}, {browser.code_id: browser}))
        self.assertEqual(
            ["CODE_INVENTED: area binding names invented code identity"],
            validator({"CODE_INVENTED": "hmi_ui"}, {browser.code_id: browser}),
        )

    def test_loads_exact_id_indexes_and_area_bindings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "verification/invariants/runtime_safety").mkdir(parents=True)
            (root / "verification/code-spec-map.toml").write_text(
                'schema_version = 1\n[[mappings]]\ncode_id = "CODE_A"\narea = "runtime_safety"\n'
            )
            (root / "verification/code-spec-debt-baseline.toml").write_text(
                'schema_version = 1\ncreated_from_commit = "' + "c" * 40 + '"\n'
                'captured_on = "2026-07-21"\nenforcement_mode = "changed"\n'
                'append_policy = "delete_only"\n'
            )
            (root / "verification/code-spec-risk-acknowledgements.toml").write_text(
                'schema_version = 1\n[[acknowledgements]]\n'
                'id = "RISK_ACK_' + "A" * 64 + '"\n'
                'code_id = "CODE_A"\n',
                encoding="utf-8",
            )
            (root / "verification/spec-sources.toml").write_text(
                '[[spec_sources]]\nid = "SPEC_A"\nsource_status = "active"\n'
            )
            (root / "verification/spec-gaps.toml").write_text(
                '[[spec_gaps]]\nid = "GAP_A"\nresolution_status = "open"\n'
            )
            (root / "verification/test-catalog.toml").write_text(
                '[[tests]]\nid = "TEST_A"\nstatus = "mapped"\n'
            )
            (root / "verification/invariants/runtime_safety/INV_A.toml").write_text(
                'id = "INV_A"\narea = "runtime_safety"\n'
            )

            metadata = load_code_spec_metadata(root)

        self.assertEqual({"CODE_A": "runtime_safety"}, metadata.area_bindings)
        self.assertEqual(["SPEC_A"], list(metadata.spec_sources))
        self.assertEqual(["GAP_A"], list(metadata.spec_gaps))
        self.assertEqual(["INV_A"], list(metadata.invariants))
        self.assertEqual(["TEST_A"], list(metadata.tests))
        self.assertEqual(
            ["RISK_ACK_" + "A" * 64],
            list(getattr(metadata, "risk_acknowledgements", {})),
        )

    def test_duplicate_or_missing_ids_and_wrong_roots_are_blocking(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "verification/invariants/runtime_safety").mkdir(parents=True)
            (root / "verification/code-spec-map.toml").write_text(
                'schema_version = 1\n[[mappings]]\ncode_id = "CODE_A"\narea = "runtime_safety"\n'
                '[[mappings]]\ncode_id = "CODE_A"\narea = "runtime_safety"\n'
            )
            (root / "verification/code-spec-debt-baseline.toml").write_text(
                'schema_version = 1\ncreated_from_commit = "' + "c" * 40 + '"\n'
                'captured_on = "2026-07-21"\nenforcement_mode = "changed"\n'
                'append_policy = "delete_only"\n'
            )
            (root / "verification/spec-sources.toml").write_text("spec_sources = []\n")
            (root / "verification/spec-gaps.toml").write_text("spec_gaps = []\n")
            (root / "verification/test-catalog.toml").write_text("tests = []\n")
            with self.assertRaisesRegex(CodeSpecLiveError, "duplicate code_id CODE_A"):
                load_code_spec_metadata(root)


if __name__ == "__main__":
    unittest.main()
