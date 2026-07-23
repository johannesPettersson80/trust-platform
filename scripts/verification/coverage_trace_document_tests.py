"""Focused tests for deterministic Rust trace document assembly."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.code_spec_common import CodeSourceSpan
from scripts.verification.coverage_trace_document import build_rust_trace_document


DISCOVERY_ID = "DISC_00000000000000000001"
CODE_ID = "CODE_" + "A" * 64


class CoverageTraceDocumentTests(unittest.TestCase):
    def test_completed_attempt_and_export_create_exact_trace_record(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root = root / "trace"
            attempt = trace_root / "attempts" / "attempt-key"
            profiles = attempt / "profiles"
            profiles.mkdir(parents=True)
            (profiles / "one.profraw").write_bytes(b"profile")
            source_path = "crates/example/src/lib.rs"
            source = b"fn behavior() { execute(); }\n"
            governed = root / source_path
            governed.parent.mkdir(parents=True)
            governed.write_bytes(source)
            manifest = {
                "discovery_id": DISCOVERY_ID,
                "test_name": "tests::observes",
                "binary_id": "example",
                "attempt_id": "run:test",
                "attempt_key": "attempt-key",
                "profile_pattern": str(profiles / "%m-%p.profraw"),
                "test_executable": "/target/example",
                "test_arguments": ["--exact", "tests::observes"],
                "status": "completed",
                "exit_code": 0,
                "duration_ms": 12,
            }
            import json

            (attempt / "attempt.json").write_text(json.dumps(manifest))
            request = {
                "discovery_id": DISCOVERY_ID,
                "discovery_source_kind": "rust_unit_test",
                "path": source_path,
                "name": "observes",
                "ignore_state": "not_ignored",
                "package": "example",
                "nextest_binary_id": "example",
                "nextest_test_name": "tests::observes",
                "command": "cargo nextest run exact",
                "command_authority": "verified_synthesized",
            }
            export = {
                "type": "llvm.coverage.json.export",
                "version": "3.1.0",
                "data": [
                    {
                        "files": [],
                        "functions": [
                            {
                                "name": "behavior",
                                "count": 1,
                                "filenames": [str(governed)],
                                "regions": [[1, 1, 1, 28, 1, 0, 0, 0]],
                                "branches": [],
                                "mcdc_records": [],
                            }
                        ],
                        "totals": {},
                    }
                ],
            }

            document = build_rust_trace_document(
                root=root,
                trace_root=trace_root,
                requested=[request],
                coverage_exports={DISCOVERY_ID: export},
                sources={source_path: source},
                spans={
                    CODE_ID: CodeSourceSpan(
                        CODE_ID, source_path, 0, len(source)
                    )
                },
                source_commit="a" * 40,
                platform="linux-x86_64",
                input_digest="sha256:" + "1" * 64,
                code_inventory_digests={
                    "rust_product": "sha256:" + "2" * 64,
                },
                mode="focused_shard",
            )

        record = document["tests"][0]
        self.assertEqual([CODE_ID], record["covered_code_ids"])
        self.assertEqual(12, record["duration_ms"])
        self.assertEqual("observed", record["trace_status"])
        self.assertRegex(record["profile_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(
            {"rust_product": "sha256:" + "2" * 64},
            document["code_inventory_digests"],
        )

    def test_missing_duplicate_or_failed_attempt_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root = root / "trace"
            trace_root.mkdir()
            request = {
                "discovery_id": DISCOVERY_ID,
                "discovery_source_kind": "rust_unit_test",
                "path": "crates/example/src/lib.rs",
                "name": "observes",
                "ignore_state": "not_ignored",
                "package": "example",
                "nextest_binary_id": "example",
                "nextest_test_name": "tests::observes",
                "command": "cargo nextest run exact",
                "command_authority": "verified_synthesized",
            }
            with self.assertRaisesRegex(ValueError, "attempt set does not match"):
                build_rust_trace_document(
                    root=root,
                    trace_root=trace_root,
                    requested=[request],
                    coverage_exports={},
                    sources={},
                    spans={},
                    source_commit="a" * 40,
                    platform="linux",
                    input_digest="sha256:" + "1" * 64,
                    code_inventory_digests={
                        "rust_product": "sha256:" + "2" * 64,
                    },
                    mode="focused_shard",
                )


if __name__ == "__main__":
    unittest.main()
