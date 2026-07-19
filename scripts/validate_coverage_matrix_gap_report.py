#!/usr/bin/env python3
"""Validate a generated coverage-matrix gap report at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.coverage_matrix_gap_validation import validate_report_files
from verification.coverage_matrix_gaps import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown", type=Path, default=DEFAULT_MARKDOWN_PATH)
    args = parser.parse_args()
    root = args.root.resolve()
    failures = validate_report_files(
        root,
        args.json,
        args.markdown,
        root / "verification/schemas/coverage-matrix-gap-report.schema.json",
    )
    if failures:
        print("coverage-matrix gap validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("coverage-matrix gap report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
