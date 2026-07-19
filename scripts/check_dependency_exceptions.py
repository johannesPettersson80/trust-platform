#!/usr/bin/env python3
"""Validate cargo-deny advisory exceptions against the 90-day policy."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from datetime import date
from pathlib import Path

try:
    from scripts.release_evidence_contract import (
        DependencyException,
        ReleaseEvidenceError,
        validate_dependency_exception,
    )
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    from release_evidence_contract import (  # type: ignore[no-redef]
        DependencyException,
        ReleaseEvidenceError,
        validate_dependency_exception,
    )


REASON_FIELDS = ("owner", "rationale", "review", "removal", "expires")


def parse_reason(advisory_id: str, reason: str) -> DependencyException:
    fields: dict[str, str] = {}
    for part in reason.split(";"):
        key, separator, value = part.strip().partition("=")
        if separator:
            fields[key.strip()] = value.strip()
    missing = [field for field in REASON_FIELDS if not fields.get(field)]
    if missing:
        raise ReleaseEvidenceError(
            f"dependency exception {advisory_id} missing fields: {', '.join(missing)}"
        )
    if not re.fullmatch(r"RUSTSEC-\d{4}-\d{4}", advisory_id):
        raise ReleaseEvidenceError(f"invalid advisory id {advisory_id!r}")
    try:
        reviewed = date.fromisoformat(fields["review"])
        expires = date.fromisoformat(fields["expires"])
    except ValueError as exc:
        raise ReleaseEvidenceError(
            f"dependency exception {advisory_id} has invalid date: {exc}"
        ) from exc
    return DependencyException(
        advisory_id=advisory_id,
        owner=fields["owner"],
        rationale=fields["rationale"],
        removal=fields["removal"],
        reviewed=reviewed,
        expires=expires,
    )


def validate_file(path: Path, *, today: date) -> list[DependencyException]:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    entries = data.get("advisories", {}).get("ignore", [])
    if not isinstance(entries, list):
        raise ReleaseEvidenceError("[advisories].ignore must be an array")
    parsed: list[DependencyException] = []
    seen: set[str] = set()
    for row in entries:
        if not isinstance(row, dict) or set(row) != {"id", "reason"}:
            raise ReleaseEvidenceError(
                "each advisory exception must contain exactly id and reason"
            )
        advisory_id = row.get("id")
        reason = row.get("reason")
        if not isinstance(advisory_id, str) or not isinstance(reason, str):
            raise ReleaseEvidenceError("advisory exception id and reason must be strings")
        if advisory_id in seen:
            raise ReleaseEvidenceError(f"duplicate dependency exception {advisory_id}")
        seen.add(advisory_id)
        exception = parse_reason(advisory_id, reason)
        validate_dependency_exception(exception)
        if exception.expires is not None and exception.expires < today:
            raise ReleaseEvidenceError(
                f"dependency exception {advisory_id} expired on {exception.expires}"
            )
        parsed.append(exception)
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--deny", type=Path, default=Path("deny.toml"))
    parser.add_argument("--today", type=date.fromisoformat, default=date.today())
    args = parser.parse_args()
    try:
        rows = validate_file(args.deny, today=args.today)
    except (OSError, tomllib.TOMLDecodeError, ReleaseEvidenceError) as exc:
        print(f"dependency-exception-check: FAIL: {exc}", file=sys.stderr)
        return 1
    print(f"dependency-exception-check: PASS ({len(rows)} owned exceptions)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
