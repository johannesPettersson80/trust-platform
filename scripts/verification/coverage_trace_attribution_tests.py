"""Focused tests for LLVM region to production-code attribution."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.coverage_trace_attribution import (
    CodeSpan,
    attributed_code_ids,
    parse_llvm_export,
)


CODE_OUTER = "CODE_" + "A" * 64
CODE_INNER = "CODE_" + "B" * 64
CODE_ADJACENT = "CODE_" + "C" * 64


def _export(
    filename: str,
    regions: list[list[int]],
) -> dict[str, object]:
    return {
        "type": "llvm.coverage.json.export",
        "version": "2.0.1",
        "data": [
            {
                "files": [
                    {
                        "filename": filename,
                        "regions": regions,
                    }
                ]
            }
        ],
    }


class LlvmCoverageAttributionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path("/repo")
        self.path = "crates/example/src/lib.rs"
        self.source = "fn outer() {\n    let café = 1;\n    use_it(café);\n}\n".encode()
        outer_start = self.source.index(b"{")
        outer_end = len(self.source)
        inner_start = self.source.index(b"let")
        inner_end = self.source.index(b";", inner_start) + 1
        adjacent_start = self.source.index(b"use_it")
        self.spans = {
            CODE_OUTER: CodeSpan(CODE_OUTER, self.path, outer_start, outer_end),
            CODE_INNER: CodeSpan(CODE_INNER, self.path, inner_start, inner_end),
            CODE_ADJACENT: CodeSpan(
                CODE_ADJACENT,
                self.path,
                adjacent_start,
                self.source.index(b";", adjacent_start) + 1,
            ),
        }

    def test_positive_code_region_maps_every_intersecting_fact(self) -> None:
        # LLVM columns are one-based UTF-8 byte columns. This covers only `café`.
        document = _export(
            "/repo/crates/example/src/lib.rs",
            [[2, 9, 2, 14, 1, 0, 0, 0]],
        )

        regions = parse_llvm_export(
            document,
            root=self.root,
            sources={self.path: self.source},
        )

        self.assertEqual(
            tuple(sorted((CODE_INNER, CODE_OUTER))),
            attributed_code_ids(regions, self.spans),
        )

    def test_zero_count_non_code_and_adjacent_regions_do_not_map(self) -> None:
        document = _export(
            "/repo/crates/example/src/lib.rs",
            [
                [2, 5, 2, 8, 0, 0, 0, 0],
                [2, 5, 2, 8, 7, 0, 0, 1],
                [3, 5, 3, 11, 1, 0, 0, 0],
            ],
        )

        regions = parse_llvm_export(
            document,
            root=self.root,
            sources={self.path: self.source},
        )

        self.assertEqual(
            tuple(sorted((CODE_ADJACENT, CODE_OUTER))),
            attributed_code_ids(regions, self.spans),
        )

    def test_repo_relative_filename_is_supported_deterministically(self) -> None:
        document = _export(
            self.path,
            [[3, 5, 3, 11, 2, 0, 0, 0]],
        )

        regions = parse_llvm_export(
            document,
            root=self.root,
            sources={self.path: self.source},
        )

        self.assertEqual(self.path, regions[0].path)

    def test_skip_functions_file_segments_map_positive_executed_ranges(self) -> None:
        document = {
            "type": "llvm.coverage.json.export",
            "version": "3.1.0",
            "data": [
                {
                    "files": [
                        {
                            "filename": f"/repo/{self.path}",
                            "segments": [
                                [2, 5, 2, True, True, False],
                                [2, 14, 0, False, False, False],
                                [3, 5, 9, True, True, True],
                                [3, 11, 0, False, False, False],
                            ],
                        }
                    ]
                }
            ],
        }

        regions = parse_llvm_export(
            document,
            root=self.root,
            sources={self.path: self.source},
        )

        self.assertEqual(
            tuple(sorted((CODE_INNER, CODE_OUTER))),
            attributed_code_ids(regions, self.spans),
        )

    def test_malformed_file_segment_fails_closed(self) -> None:
        document = {
            "type": "llvm.coverage.json.export",
            "version": "3.1.0",
            "data": [
                {
                    "files": [
                        {
                            "filename": f"/repo/{self.path}",
                            "segments": [[2, 5, 1, 1, True, False]],
                        }
                    ]
                }
            ],
        }

        with self.assertRaisesRegex(
            ValueError, "three integers and three booleans"
        ):
            parse_llvm_export(
                document,
                root=self.root,
                sources={self.path: self.source},
            )

    def test_outside_root_and_unknown_sources_cannot_create_attribution(self) -> None:
        outside = parse_llvm_export(
            _export("/other/lib.rs", [[1, 1, 1, 2, 1, 0, 0, 0]]),
            root=self.root,
            sources={self.path: self.source},
        )
        unknown = parse_llvm_export(
            _export("crates/other/src/lib.rs", [[1, 1, 1, 2, 1, 0, 0, 0]]),
            root=self.root,
            sources={self.path: self.source},
        )

        self.assertEqual((), outside)
        self.assertEqual((), unknown)

    def test_malformed_region_and_non_utf8_boundary_fail_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "eight integers"):
            parse_llvm_export(
                _export(self.path, [[1, 1, 1]]),
                root=self.root,
                sources={self.path: self.source},
            )
        with self.assertRaisesRegex(ValueError, "UTF-8 boundary"):
            parse_llvm_export(
                _export(self.path, [[2, 13, 2, 14, 1, 0, 0, 0]]),
                root=self.root,
                sources={self.path: self.source},
            )

    def test_export_root_is_closed_and_versioned(self) -> None:
        document = _export(self.path, [])
        document["unexpected"] = True

        with self.assertRaisesRegex(ValueError, "unexpected fields"):
            parse_llvm_export(
                document,
                root=self.root,
                sources={self.path: self.source},
            )

    def test_version_three_function_regions_resolve_filename_index(self) -> None:
        document = {
            "type": "llvm.coverage.json.export",
            "version": "3.1.0",
            "data": [
                {
                    "files": [],
                    "functions": [
                        {
                            "name": "example",
                            "count": 1,
                            "filenames": [f"/repo/{self.path}"],
                            "regions": [[3, 5, 3, 11, 1, 0, 0, 0]],
                            "branches": [],
                            "mcdc_records": [],
                        }
                    ],
                    "totals": {},
                }
            ],
        }

        regions = parse_llvm_export(
            document,
            root=self.root,
            sources={self.path: self.source},
        )

        self.assertEqual(1, len(regions))
        self.assertEqual(self.path, regions[0].path)


if __name__ == "__main__":
    unittest.main()
