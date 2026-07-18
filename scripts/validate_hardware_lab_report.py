#!/usr/bin/env python3
"""Validate a committed Phase 11 hardware-lab report pair."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.verification.hardware_lab import REPORT_SCHEMA_PATH
from scripts.verification.hardware_lab_validation import validate_files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    failures = validate_files(root, args.json, args.markdown, root / REPORT_SCHEMA_PATH)
    if failures:
        for failure in failures:
            print(f"Hardware-lab report: FAIL: {failure}")
        return 1
    print("Hardware-lab report: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
