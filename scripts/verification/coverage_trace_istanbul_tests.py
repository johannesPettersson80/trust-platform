"""Focused tests for Istanbul-to-code-fact attribution."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.verification.code_spec_common import CodeSourceSpan
from scripts.verification.coverage_trace_attribution import attributed_code_ids
from scripts.verification.coverage_trace_istanbul import (
    merge_istanbul_coverage,
    parse_istanbul_coverage,
)


class CoverageTraceIstanbulTests(unittest.TestCase):
    def test_covered_original_typescript_ranges_attribute_exact_facts(self) -> None:
        source = "export function café() {\n  return 1;\n}\n".encode()
        with TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "editors/vscode/src/live.ts"
            path.parent.mkdir(parents=True)
            path.write_bytes(source)
            document = {
                str(path): {
                    "path": str(path),
                    "statementMap": {
                        "0": {
                            "start": {"line": 2, "column": 2},
                            "end": {"line": 2, "column": 11},
                        }
                    },
                    "s": {"0": 1},
                    "fnMap": {
                        "0": {
                            "name": "café",
                            "decl": {
                                "start": {"line": 1, "column": 16},
                                "end": {"line": 1, "column": 20},
                            },
                            "loc": {
                                "start": {"line": 1, "column": 7},
                                "end": {"line": 3, "column": 1},
                            },
                        }
                    },
                    "f": {"0": 1},
                    "branchMap": {},
                    "b": {},
                }
            }

            regions = parse_istanbul_coverage(
                document,
                root=root,
                sources={"editors/vscode/src/live.ts": source},
            )

        spans = {
            "CODE_LIVE": CodeSourceSpan(
                code_id="CODE_LIVE",
                path="editors/vscode/src/live.ts",
                body_start=0,
                body_end=len(source),
            )
        }
        self.assertEqual(("CODE_LIVE",), attributed_code_ids(regions, spans))
        self.assertEqual(2, len(regions))

    def test_zero_counts_and_sources_outside_inventory_create_no_attribution(self) -> None:
        source = b"export function live() { return 1; }\n"
        document = {
            "/outside/generated.js": {
                "path": "/outside/generated.js",
                "statementMap": {},
                "s": {},
                "fnMap": {},
                "f": {},
            },
            "editors/vscode/src/live.ts": {
                "path": "editors/vscode/src/live.ts",
                "statementMap": {
                    "0": {
                        "start": {"line": 1, "column": 0},
                        "end": {"line": 1, "column": 36},
                    }
                },
                "s": {"0": 0},
                "fnMap": {},
                "f": {},
            },
        }

        regions = parse_istanbul_coverage(
            document,
            root=Path("/repo"),
            sources={"editors/vscode/src/live.ts": source},
        )

        self.assertEqual((), regions)

    def test_mismatched_counter_identity_is_rejected(self) -> None:
        document = {
            "editors/vscode/src/live.ts": {
                "statementMap": {
                    "0": {
                        "start": {"line": 1, "column": 0},
                        "end": {"line": 1, "column": 1},
                    }
                },
                "s": {"1": 1},
                "fnMap": {},
                "f": {},
            }
        }

        with self.assertRaisesRegex(ValueError, "identities differ"):
            parse_istanbul_coverage(
                document,
                root=Path("/repo"),
                sources={"editors/vscode/src/live.ts": b"x\n"},
            )

    def test_coverage_profiles_merge_only_when_geometry_is_identical(self) -> None:
        base = {
            "editors/vscode/src/live.ts": {
                "path": "editors/vscode/src/live.ts",
                "statementMap": {},
                "s": {},
                "fnMap": {"0": {"loc": {"start": {}, "end": {}}}},
                "f": {"0": 1},
                "branchMap": {},
                "b": {},
            }
        }
        second = {
            "editors/vscode/src/live.ts": {
                **base["editors/vscode/src/live.ts"],
                "f": {"0": 2},
            }
        }

        merged = merge_istanbul_coverage([base, second])

        self.assertEqual(
            {"0": 3},
            merged["editors/vscode/src/live.ts"]["f"],
        )


if __name__ == "__main__":
    unittest.main()
