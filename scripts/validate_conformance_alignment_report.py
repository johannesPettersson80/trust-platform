#!/usr/bin/env python3
"""Validate a generated Phase 7 conformance alignment audit at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.conformance_alignment_live import REPORT_SCHEMA_PATH
from verification.conformance_alignment_report import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from verification.conformance_alignment_validation import validate_report_files


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
        print("conformance alignment validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("conformance alignment report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
