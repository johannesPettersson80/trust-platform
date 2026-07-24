"""Focused tests for exact-path Rust code-fact validation."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification.code_spec_common import sha256_digest, stable_code_id
from scripts.verification.mapping_live_code_facts import (
    rust_module_for_path,
    validated_live_code_facts,
)


class MappingLiveCodeFactsTests(unittest.TestCase):
    def test_covered_file_uses_only_single_file_extractor(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/lower/exact.rs"
            source = b"pub fn exact() {}\n"
            source_path = root / relative
            source_path.parent.mkdir(parents=True)
            source_path.write_bytes(source)
            signature = "pub fn exact()"
            qualified_symbol = "example::lower::exact::exact"
            code_id = stable_code_id(
                source_kind="rust",
                path=relative,
                qualified_symbol=qualified_symbol,
                item_kind="function",
                signature=signature,
            )
            document = {
                "schema_version": 1,
                "facts": [
                    {
                        "qualified_symbol": qualified_symbol,
                        "item_kind": "function",
                        "visibility": "pub",
                        "signature": signature,
                        "body_start": 0,
                        "body_end": len(source),
                        "line": 1,
                        "risk_signals": [],
                    }
                ],
                "diagnostics": [],
            }
            completed = subprocess.CompletedProcess(
                args=[],
                returncode=0,
                stdout=json.dumps(document),
                stderr="",
            )
            mappings = {
                code_id: {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": qualified_symbol,
                    "item_kind": "function",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": sha256_digest(source),
                    "area": "compiler_iec",
                }
            }
            trace = {
                "tests": [
                    {
                        "discovery_id": "DISC_" + "A" * 20,
                        "covered_code_ids": [code_id],
                    }
                ]
            }

            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                return_value=completed,
            ) as run:
                facts = validated_live_code_facts(root, trace, mappings)

            self.assertEqual([code_id], list(facts))
            command = run.call_args.args[0]
            self.assertEqual(
                [
                    "cargo",
                    "run",
                    "-q",
                    "-p",
                    "xtask",
                    "--",
                    "code-spec-rust",
                    "--path",
                    relative,
                    "--module",
                    "example::lower::exact",
                ],
                command,
            )
            self.assertNotIn("code-spec-rust-all", command)

    def test_live_body_mismatch_rejects_stale_code_map(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/lib.rs"
            source = b"fn exact() {}\n"
            source_path = root / relative
            source_path.parent.mkdir(parents=True)
            source_path.write_bytes(source)
            signature = "fn exact()"
            qualified_symbol = "example::exact"
            code_id = stable_code_id(
                source_kind="rust",
                path=relative,
                qualified_symbol=qualified_symbol,
                item_kind="function",
                signature=signature,
            )
            document = {
                "schema_version": 1,
                "facts": [
                    {
                        "qualified_symbol": qualified_symbol,
                        "item_kind": "function",
                        "visibility": "private",
                        "signature": signature,
                        "body_start": 0,
                        "body_end": len(source),
                        "line": 1,
                        "risk_signals": [],
                    }
                ],
                "diagnostics": [],
            }
            mappings = {
                code_id: {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": qualified_symbol,
                    "item_kind": "function",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": "sha256:" + "0" * 64,
                    "area": "compiler_iec",
                }
            }
            trace = {"tests": [{"covered_code_ids": [code_id]}]}
            completed = subprocess.CompletedProcess(
                args=[],
                returncode=0,
                stdout=json.dumps(document),
                stderr="",
            )

            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                return_value=completed,
            ), self.assertRaisesRegex(
                ValueError,
                "reviewed_body_digest differs from code map",
            ):
                validated_live_code_facts(root, trace, mappings)

    def test_module_derivation_covers_library_and_binary_sources(self) -> None:
        self.assertEqual(
            "trust_hir::types::resolve",
            rust_module_for_path("crates/trust-hir/src/types/resolve.rs"),
        )
        self.assertEqual(
            "worker::protocol",
            rust_module_for_path("crates/trust-runtime/src/bin/worker/protocol.rs"),
        )


if __name__ == "__main__":
    unittest.main()
