"""Focused tests for deterministic Istanbul trace assembly."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.code_spec_common import CodeSourceSpan
from scripts.verification.coverage_trace_istanbul_document import (
    build_istanbul_trace_document,
)


DISCOVERY_ID = "DISC_" + "A" * 20
CODE_ID = "CODE_" + "B" * 64


class CoverageTraceIstanbulDocumentTests(unittest.TestCase):
    def test_successful_observation_builds_an_attributed_trace(self) -> None:
        source = b"export function live() { return 1; }\n"
        document = build_istanbul_trace_document(
            root=Path("/repo"),
            requested=[
                {
                    "discovery_id": DISCOVERY_ID,
                    "discovery_source_kind": "vscode_test",
                    "path": "editors/vscode/src/test/suite/live.test.ts",
                    "name": "uses live",
                }
            ],
            observations={
                DISCOVERY_ID: {
                    "attempt_id": "attempt-1",
                    "exit_code": 0,
                    "duration_ms": 9,
                    "command": "generated focused VS Code runner",
                }
            },
            coverage_documents={
                DISCOVERY_ID: {
                    "editors/vscode/src/live.ts": {
                        "statementMap": {
                            "0": {
                                "start": {"line": 1, "column": 0},
                                "end": {"line": 1, "column": 36},
                            }
                        },
                        "s": {"0": 1},
                        "fnMap": {},
                        "f": {},
                    }
                }
            },
            sources={"editors/vscode/src/live.ts": source},
            spans={
                CODE_ID: CodeSourceSpan(
                    code_id=CODE_ID,
                    path="editors/vscode/src/live.ts",
                    body_start=0,
                    body_end=len(source),
                )
            },
            source_commit="a" * 40,
            platform="linux-x86_64",
            input_digest="sha256:" + "1" * 64,
            code_inventory_digest="sha256:" + "2" * 64,
            mode="focused_shard",
        )

        self.assertEqual([CODE_ID], document["tests"][0]["covered_code_ids"])
        self.assertEqual(
            "istanbul_source_coverage",
            document["tests"][0]["attribution_kind"],
        )
        self.assertEqual(
            {"typescript_product": "sha256:" + "2" * 64},
            document["code_inventory_digests"],
        )

    def test_observation_set_must_exactly_match_requests(self) -> None:
        with self.assertRaisesRegex(ValueError, "observation set"):
            build_istanbul_trace_document(
                root=Path("/repo"),
                requested=[
                    {
                        "discovery_id": DISCOVERY_ID,
                        "discovery_source_kind": "vscode_test",
                        "path": "test.ts",
                        "name": "test",
                    }
                ],
                observations={},
                coverage_documents={},
                sources={},
                spans={},
                source_commit="a" * 40,
                platform="linux",
                input_digest="sha256:" + "1" * 64,
                code_inventory_digest="sha256:" + "2" * 64,
                mode="focused_shard",
            )


if __name__ == "__main__":
    unittest.main()
