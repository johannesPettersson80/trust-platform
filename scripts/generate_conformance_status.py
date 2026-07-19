#!/usr/bin/env python3
"""Generate release conformance status from an executed suite summary."""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Mapping


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


def _gap_titles(text: str) -> list[str]:
    return [line[2:].strip() for line in text.splitlines() if line.startswith("- ")]


def build_conformance_status(
    *,
    summary: Mapping[str, Any],
    known_gaps: str,
    commit: str,
    toolchain: str,
    timestamp: str,
) -> dict:
    if not COMMIT_RE.fullmatch(commit):
        raise ValueError("commit must be a clean full Git SHA")
    parsed_time = datetime.fromisoformat(timestamp)
    if parsed_time.tzinfo is None:
        raise ValueError("timestamp must carry a timezone")
    if not toolchain.strip():
        raise ValueError("toolchain is required")
    counts = summary.get("summary")
    results = summary.get("results")
    if not isinstance(counts, Mapping) or not isinstance(results, list):
        raise ValueError("input is not a conformance summary")
    required_counts = ("total", "passed", "failed", "errors", "skipped")
    if any(isinstance(counts.get(key), bool) or not isinstance(counts.get(key), int) for key in required_counts):
        raise ValueError("conformance summary counts must be integers")
    if counts["total"] != len(results):
        raise ValueError("conformance total does not match result count")
    if counts["total"] != sum(counts[key] for key in required_counts[1:]):
        raise ValueError("conformance outcome counts do not sum to total")
    gaps = _gap_titles(known_gaps)
    if not gaps:
        raise ValueError("known-gap source contains no current gap rows")
    return {
        "schema_version": 1,
        "commit": commit,
        "toolchain": toolchain,
        "timestamp": timestamp,
        "runtime": summary.get("runtime"),
        "executed": counts["total"],
        "passed": counts["passed"],
        "failed": counts["failed"],
        "errors": counts["errors"],
        "skipped": counts["skipped"],
        "known_gaps": gaps,
        "result_source": "conformance-summary-v2",
    }


def render_markdown(payload: Mapping[str, Any]) -> str:
    lines = [
        "# Release Conformance Status",
        "",
        f"- Commit: `{payload['commit']}`",
        f"- Toolchain: `{payload['toolchain']}`",
        f"- Timestamp: `{payload['timestamp']}`",
        f"- Executed: {payload['executed']}",
        f"- Passed: {payload['passed']}",
        f"- Failed: {payload['failed']}",
        f"- Errors: {payload['errors']}",
        f"- Skipped: {payload['skipped']}",
        "",
        "## Known Gaps",
        "",
    ]
    lines.extend(f"- {gap}" for gap in payload["known_gaps"])
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--summary", type=Path, required=True)
    parser.add_argument("--known-gaps", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--toolchain", required=True)
    parser.add_argument("--timestamp", required=True)
    parser.add_argument("--json-out", type=Path, required=True)
    parser.add_argument("--markdown-out", type=Path, required=True)
    args = parser.parse_args()
    try:
        payload = build_conformance_status(
            summary=json.loads(args.summary.read_text(encoding="utf-8")),
            known_gaps=args.known_gaps.read_text(encoding="utf-8"),
            commit=args.commit,
            toolchain=args.toolchain,
            timestamp=args.timestamp,
        )
        args.json_out.write_text(
            json.dumps(payload, sort_keys=True, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        args.markdown_out.write_text(render_markdown(payload), encoding="utf-8")
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"conformance-status: FAIL: {exc}", file=sys.stderr)
        return 1
    print(f"conformance-status: wrote {payload['executed']} executed cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
