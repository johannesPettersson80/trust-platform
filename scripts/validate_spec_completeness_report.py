#!/usr/bin/env python3
"""Validate a generated specification-completeness report at rest."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.spec_completeness_cli import SCHEMA_PATH
from verification.spec_completeness_live import validate_report_files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--json",
        type=Path,
        default=Path("target/gate-artifacts/verification/spec-completeness.json"),
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        default=Path("target/gate-artifacts/verification/spec-completeness.md"),
    )
    parser.add_argument("--schema", type=Path, default=SCHEMA_PATH)
    args = parser.parse_args()
    failures = validate_report_files(
        args.root,
        args.json,
        args.markdown,
        args.schema,
    )
    if failures:
        print("specification-completeness report validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("specification-completeness report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
