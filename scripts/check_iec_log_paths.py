#!/usr/bin/env python3
"""Guard IEC decision/deviation log locations.

This repository uses the top-level docs/IEC_DECISIONS.md and
docs/IEC_DEVIATIONS.md files as the only authoritative tracked logs.

The guard intentionally checks tracked files only, so developers may still keep
local untracked IEC source material under docs/internal/ without tripping CI.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

CANONICAL = {
    "docs/IEC_DECISIONS.md",
    "docs/IEC_DEVIATIONS.md",
}
DISALLOWED_TRACKED = {
    "docs/internal/standards/IEC_DECISIONS.md",
    "docs/internal/standards/IEC_DEVIATIONS.md",
}
DISALLOWED_REFERENCE_PATTERNS = [
    "docs/internal/standards/IEC_DECISIONS.md",
    "docs/internal/standards/IEC_DEVIATIONS.md",
    "internal/standards/IEC_DECISIONS.md",
    "internal/standards/IEC_DEVIATIONS.md",
]
EXCLUDED_PATHS = {
    "scripts/check_iec_log_paths.py",
}


def run_git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def tracked_files(*paths: str) -> set[str]:
    result = run_git("ls-files", *paths)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"git ls-files failed: {' '.join(paths)}")
    return {line.strip() for line in result.stdout.splitlines() if line.strip()}


def grep_tracked(pattern: str) -> list[str]:
    result = run_git("grep", "-n", "-F", pattern, "--", ".")
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr.strip() or f"git grep failed for {pattern}")
    matches = []
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        path = line.split(":", 1)[0]
        if path in EXCLUDED_PATHS:
            continue
        matches.append(line)
    return matches


def main() -> int:
    errors: list[str] = []

    tracked_canonical = tracked_files(*CANONICAL)
    missing_canonical = sorted(CANONICAL - tracked_canonical)
    if missing_canonical:
        errors.append(
            "Missing canonical IEC log file(s): " + ", ".join(missing_canonical)
        )

    tracked_disallowed = tracked_files(*DISALLOWED_TRACKED)
    if tracked_disallowed:
        errors.append(
            "Tracked non-canonical IEC log file(s) found: "
            + ", ".join(sorted(tracked_disallowed))
        )

    stale_refs: list[str] = []
    for pattern in DISALLOWED_REFERENCE_PATTERNS:
        stale_refs.extend(grep_tracked(pattern))
    if stale_refs:
        errors.append(
            "Tracked files still reference non-canonical IEC log paths:\n  "
            + "\n  ".join(sorted(stale_refs))
        )

    if errors:
        print("IEC log path guard failed.", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("IEC log path guard passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
