#!/usr/bin/env python3
"""Validate a generated test-class completeness report at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.test_class_completeness import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from verification.test_class_completeness_validation import validate_report_files


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
        root / "verification/schemas/test-class-completeness-report.schema.json",
    )
    if failures:
        print("test-class completeness validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("test-class completeness report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
