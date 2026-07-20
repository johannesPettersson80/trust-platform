#!/usr/bin/env python3
"""Validate a generated ignored-test inventory report at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.ignored_test_live import REPORT_SCHEMA_PATH
from verification.ignored_test_models import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from verification.ignored_test_validation import validate_report_files


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
        root / REPORT_SCHEMA_PATH,
    )
    if failures:
        print("ignored-test inventory validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("ignored-test inventory report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
