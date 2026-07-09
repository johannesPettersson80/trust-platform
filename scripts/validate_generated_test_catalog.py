#!/usr/bin/env python3
"""Validate the generated existing-test catalog and its Markdown summary at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.test_catalog_models import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from verification.test_catalog_validation import validate_report_files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown", type=Path, default=DEFAULT_MARKDOWN_PATH)
    args = parser.parse_args()
    failures = validate_report_files(
        args.root,
        args.json,
        args.markdown,
        args.root / "verification/schemas/generated-test-catalog.schema.json",
    )
    if failures:
        print("generated test catalog validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("generated test catalog validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
