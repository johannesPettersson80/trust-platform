"""Focused tests for deterministic Rust trace document assembly."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts.verification.code_spec_common import CodeSourceSpan
from scripts.verification.coverage_trace_document import build_rust_trace_document
from scripts.verification.coverage_trace_inputs import (
    focused_rust_trace_input_provenance,
    trace_input_digest,
)
from scripts.verification.coverage_trace_resume import profile_digest
from scripts.verification.test_catalog_json_schema import (
    validate_json_schema_instance,
)


DISCOVERY_ID = "DISC_00000000000000000001"
CODE_ID = "CODE_" + "A" * 64


class CoverageTraceDocumentTests(unittest.TestCase):
    def test_focused_input_provenance_recomputes_exact_trace_digest(self) -> None:
        resolved = [
            {
                "discovery_id": "DISC_" + "A" * 20,
                "discovery_source_kind": "rust_unit_test",
                "path": "crates/example/src/lib.rs",
                "name": "exact_case",
                "ignore_state": "not_ignored",
                "package": "example",
                "nextest_binary_id": "example::lib",
                "nextest_binary_path": "/tmp/example",
                "nextest_test_name": "tests::exact_case",
                "cargo_target_kind": "lib",
                "cargo_target_name": None,
                "no_default_features": True,
                "command": "cargo nextest run",
                "command_authority": "verified_synthesized",
            }
        ]
        code_map = {"schema_version": 1, "mappings": []}
        source_digest = "sha256:" + "1" * 64

        provenance = focused_rust_trace_input_provenance(
            source_input_digest=source_digest,
            resolved=resolved,
            code_map=code_map,
            trace_root="target/gate-artifacts/trace",
            raw_evidence_digest="sha256:" + "4" * 64,
            llvm_tool_digests={
                "llvm-profdata": "sha256:" + "5" * 64,
                "llvm-cov": "sha256:" + "6" * 64,
            },
            llvm_tool_paths={
                "llvm-profdata": "/toolchain/llvm-profdata",
                "llvm-cov": "/toolchain/llvm-cov",
            },
        )

        self.assertEqual(resolved, provenance["resolved"])
        self.assertEqual(
            trace_input_digest(source_digest, resolved, code_map),
            provenance["input_digest"],
        )

        schema = json.loads(
            (
                Path(__file__).resolve().parents[2]
                / "verification/schemas/test-coverage-trace.schema.json"
            ).read_text()
        )
        document = {
            "schema_version": 1,
            "generator": "test-coverage-trace",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "platform": "linux-x86_64",
            "scope": {
                "mode": "focused_shard",
                "requested_discovery_ids": [resolved[0]["discovery_id"]],
                "source_kinds": ["rust_unit_test"],
            },
            "input_digest": provenance["input_digest"],
            "input_provenance": provenance,
            "code_inventory_digests": {},
            "tests": [],
        }
        self.assertEqual(
            [],
            validate_json_schema_instance(document, schema),
        )

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

            span = CodeSourceSpan(
                CODE_ID, source_path, 0, len(source)
            )
            span_resolver = mock.Mock(return_value={CODE_ID: span})
            export_observer = mock.Mock()
            document = build_rust_trace_document(
                root=root,
                trace_root=trace_root,
                requested=[request],
                coverage_exports={DISCOVERY_ID: export},
                sources={source_path: source},
                spans={},
                source_commit="a" * 40,
                platform="linux-x86_64",
                input_digest="sha256:" + "1" * 64,
                code_inventory_digests={
                    "rust_observed_paths": "sha256:" + "2" * 64,
                },
                mode="focused_shard",
                span_resolver=span_resolver,
                coverage_export_observer=export_observer,
            )

        record = document["tests"][0]
        span_resolver.assert_called_once_with([source_path])
        export_observer.assert_called_once_with(export)
        self.assertEqual([CODE_ID], record["covered_code_ids"])
        self.assertEqual(12, record["duration_ms"])
        self.assertEqual("observed", record["trace_status"])
        self.assertRegex(record["profile_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(
            {"rust_observed_paths": "sha256:" + "2" * 64},
            document["code_inventory_digests"],
        )

    def test_nonzero_attempt_creates_failed_record_without_attribution(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root = root / "trace"
            attempt = trace_root / "attempts" / "attempt-key"
            profiles = attempt / "profiles"
            profiles.mkdir(parents=True)
            (profiles / "one.profraw").write_bytes(b"profile")
            manifest = {
                "discovery_id": DISCOVERY_ID,
                "test_name": "tests::negative",
                "binary_id": "example",
                "attempt_id": "run:negative",
                "status": "completed",
                "exit_code": 7,
                "duration_ms": 23,
                "profile_digest": profile_digest(profiles),
            }
            import json

            (attempt / "attempt.json").write_text(json.dumps(manifest))
            request = {
                "discovery_id": DISCOVERY_ID,
                "discovery_source_kind": "structured_text_test",
                "path": "tests/fixtures/negative/src/main.st",
                "name": "negative",
                "nextest_binary_id": "example",
                "nextest_test_name": "tests::negative",
                "command": "trust-dev test --filter negative",
                "command_authority": "verified_synthesized",
            }

            document = build_rust_trace_document(
                root=root,
                trace_root=trace_root,
                requested=[request],
                coverage_exports={DISCOVERY_ID: {"invalid": "unused"}},
                sources={},
                spans={},
                source_commit="a" * 40,
                platform="linux-x86_64",
                input_digest="sha256:" + "1" * 64,
                code_inventory_digests={
                    "rust_product": "sha256:" + "2" * 64,
                },
                mode="focused_shard",
            )

        record = document["tests"][0]
        self.assertEqual("failed", record["trace_status"])
        self.assertEqual("unavailable", record["attribution_kind"])
        self.assertEqual([], record["covered_code_ids"])
        self.assertEqual(7, record["exit_code"])
        self.assertEqual(23, record["duration_ms"])
        self.assertIn("no semantic candidate", record["limitations"][0])

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
