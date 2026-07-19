#!/usr/bin/env python3
"""Run the committed Phase 6A verification-tooling fixture contract."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from verification.tooling_selftest_contract import (
    BYPASS_CONTRACT_PATH,
    load_bypass_contract,
    render_fixture_report,
    validate_bypass_contract,
)
from verification.tooling_selftest_scenarios import execute_bypass_case


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, help="write deterministic Markdown report")
    args = parser.parse_args(argv)

    contract = load_bypass_contract(BYPASS_CONTRACT_PATH)
    failures = validate_bypass_contract(contract)
    if failures:
        for failure in failures:
            print(f"tooling self-test contract failed: {failure}", file=sys.stderr)
        return 2
    results = [execute_bypass_case(row) for row in contract["cases"]]
    mismatches = [result for result in results if not result.matched]
    if mismatches:
        for result in mismatches:
            print(
                f"tooling self-test failed: {result.case_id}: expected "
                f"{result.expected_disposition}/{result.expected_signal!r}, got "
                f"{result.actual_disposition}/{result.actual_signal!r}",
                file=sys.stderr,
            )
        return 1
    report = render_fixture_report(contract, results)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(report)
    print(f"verification tooling self-tests passed: {len(results)}/{len(results)} fixtures")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
