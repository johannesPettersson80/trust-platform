#!/usr/bin/env python3
"""Render a human-readable Markdown conformance summary."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any


def load_summary(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def render(summary: dict[str, Any]) -> str:
    totals = summary["summary"]
    runtime = summary["runtime"]
    results = summary["results"]
    category_counts = Counter(item["category"] for item in results)
    category_passed = Counter(
        item["category"] for item in results if item["status"] == "passed"
    )

    lines = [
        "# truST Conformance Summary",
        "",
        f"- profile: `{summary['profile']}`",
        f"- schema version: `{summary['version']}`",
        f"- generated_at_utc: `{summary['generated_at_utc']}`",
        f"- runtime: `{runtime.get('name', '')} {runtime.get('version', '')}`",
        f"- target: `{runtime.get('target', 'unknown')}`",
        "",
        "## Totals",
        "",
        "| total | passed | failed | errors | skipped |",
        "| ---: | ---: | ---: | ---: | ---: |",
        (
            f"| {totals['total']} | {totals['passed']} | {totals['failed']} | "
            f"{totals['errors']} | {totals['skipped']} |"
        ),
        "",
        "## Categories",
        "",
        "| category | passed | total |",
        "| --- | ---: | ---: |",
    ]
    for category in sorted(category_counts):
        lines.append(
            f"| `{category}` | {category_passed[category]} | {category_counts[category]} |"
        )

    non_passed = [item for item in results if item["status"] != "passed"]
    lines.extend(["", "## Non-Passing Cases", ""])
    if not non_passed:
        lines.append("All cases passed.")
    else:
        lines.extend(["| case | status | reason |", "| --- | --- | --- |"])
        for item in non_passed:
            reason = item.get("reason") or {}
            code = reason.get("code", "")
            lines.append(f"| `{item['case_id']}` | `{item['status']}` | `{code}` |")

    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary = load_summary(args.input)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(render(summary), encoding="utf-8")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
