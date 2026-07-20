#!/usr/bin/env python3
"""Check catalog identities and reviewed redirects against a live source scan."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.test_catalog_staleness import validate_live_catalog


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    failures, records, facts = validate_live_catalog(args.root)
    if failures:
        print("test catalog staleness validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print(f"test catalog staleness validated: {records} committed records against {facts} scanner facts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
