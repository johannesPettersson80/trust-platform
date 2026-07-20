#!/usr/bin/env python3
"""Validate reviewed test-refactor proposals and catalog redirects."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.test_refactor_contract import validate_repository_test_refactors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    failures, proposals, redirects, tests, facts = validate_repository_test_refactors(args.root)
    if failures:
        print("test-refactor proposal validation failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print(
        "test-refactor proposals validated: "
        f"{proposals} proposals, {redirects} redirects, "
        f"{tests} catalog records, {facts} scanner facts"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
