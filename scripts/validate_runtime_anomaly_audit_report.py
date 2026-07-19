#!/usr/bin/env python3
"""Validate a generated Phase 8 runtime-anomaly audit at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.metadata_validator.constants import ROOT
from verification.runtime_anomaly_live import REPORT_SCHEMA_PATH
from verification.runtime_anomaly_validation import validate_report_files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args()
    failures = validate_report_files(
        args.root,
        args.json,
        args.markdown,
        Path(REPORT_SCHEMA_PATH),
    )
    if failures:
        for failure in failures:
            print(f"runtime-anomaly report validation failed: {failure}")
        return 2
    print("runtime-anomaly report validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
