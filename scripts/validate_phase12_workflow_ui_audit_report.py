#!/usr/bin/env python3
"""Validate the generated Phase 12 workflow and UI audit at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.phase12_audit import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from verification.phase12_audit_live import SCHEMA_PATH
from verification.phase12_audit_validation import validate_files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown", type=Path, default=DEFAULT_MARKDOWN_PATH)
    args = parser.parse_args()
    root = args.root.resolve()
    failures = validate_files(root, args.json, args.markdown, root / SCHEMA_PATH)
    if failures:
        print("Phase 12 workflow/UI audit validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("Phase 12 workflow/UI audit report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
