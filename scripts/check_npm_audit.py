#!/usr/bin/env python3
"""The VS Code lockfile passes `npm audit` under the owned-exception policy (specification 24, "Dependency
Exceptions").

Every advisory that `npm audit --json` reports for `editors/vscode` must have an owned exception in
`editors/vscode/audit-exceptions.toml`, which keeps deny.toml's fields and 90-day limit with GitHub advisory ids. An
advisory without one fails, and so does a missing, expired or overlong exception or an answer of npm that is not its
audit report. An exception whose advisory npm no longer reports is named, so it can be taken off the list.

Usage: python3 scripts/check_npm_audit.py [--root DIR] [--today YYYY-MM-DD]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from datetime import date
from pathlib import Path

try:
    from scripts.check_dependency_exceptions import GHSA_ID, validate_file
    from scripts.release_evidence_contract import ReleaseEvidenceError
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    from check_dependency_exceptions import GHSA_ID, validate_file  # type: ignore[no-redef]
    from release_evidence_contract import ReleaseEvidenceError  # type: ignore[no-redef]

EXCEPTIONS = Path("editors/vscode/audit-exceptions.toml")


def npm_program() -> str:
    return "npm.cmd" if sys.platform == "win32" else "npm"


def run_audit(root: Path) -> dict:
    """npm's audit report for the lockfile. npm exits non-zero when it reports advisories, so the report decides."""
    result = subprocess.run([npm_program(), "audit", "--json"], cwd=root / "editors/vscode", capture_output=True,
                            text=True)
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise ReleaseEvidenceError(f"npm audit did not answer its JSON report: {exc}") from exc
    if not isinstance(report, dict) or "error" in report or not isinstance(report.get("vulnerabilities"), dict):
        raise ReleaseEvidenceError(f"npm audit did not answer its report: {str(report)[:300]}")
    return report


def advisories(report: dict) -> dict[str, set[str]]:
    """Each advisory of the report by its GitHub id, with the packages it is reported for."""
    found: dict[str, set[str]] = {}
    for name, entry in report["vulnerabilities"].items():
        for via in entry.get("via") or []:
            if isinstance(via, dict):
                advisory = str(via.get("url", "")).rstrip("/").rsplit("/", 1)[-1]
                if not advisory.startswith("GHSA-"):
                    raise ReleaseEvidenceError(f"npm audit names an advisory of {name} without a GitHub id: {via}")
                found.setdefault(advisory, set()).add(str(via.get("name") or name))
    return found


def findings(report: dict, root: Path, today: date) -> tuple[list[str], list[str], int]:
    """The advisories without an owned exception, the exceptions whose advisory is no longer reported, and the number
    of owned exceptions. An exception that breaks the policy raises."""
    owned = validate_file(root / EXCEPTIONS, today=today, id_pattern=GHSA_ID)
    reported = advisories(report)
    excepted = {row.advisory_id for row in owned}
    unowned = [
        f"{advisory} ({', '.join(sorted(packages))}) has no owned exception in {EXCEPTIONS.as_posix()}"
        for advisory, packages in sorted(reported.items())
        if advisory not in excepted
    ]
    stale = [
        f"{row.advisory_id}: npm audit no longer reports it; take its exception off {EXCEPTIONS.as_posix()}"
        for row in owned
        if row.advisory_id not in reported
    ]
    return unowned, stale, len(owned)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--today", type=date.fromisoformat, default=date.today())
    args = parser.parse_args(argv)
    try:
        unowned, stale, owned = findings(run_audit(args.root), args.root, args.today)
    except (OSError, tomllib.TOMLDecodeError, ReleaseEvidenceError) as exc:
        print(f"check_npm_audit: FAIL: {exc}", file=sys.stderr)
        return 1
    for line in stale:
        print(f"check_npm_audit: note: {line}")
    for line in unowned:
        print(f"check_npm_audit: {line}", file=sys.stderr)
    if unowned:
        print(f"check_npm_audit: FAIL: {len(unowned)} advisory(ies) without an owned exception", file=sys.stderr)
        return 1
    print(f"check_npm_audit: PASS (every advisory owned; {owned} owned exception(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
