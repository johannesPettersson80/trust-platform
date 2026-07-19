#!/usr/bin/env python3
"""Validate a generated existing-test refactor assessment at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.test_refactor_report import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from verification.test_refactor_validation import validate_report_files


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
        root / "verification/schemas/test-refactor-assessment-report.schema.json",
    )
    if failures:
        print("test-refactor assessment validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("test-refactor assessment report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
