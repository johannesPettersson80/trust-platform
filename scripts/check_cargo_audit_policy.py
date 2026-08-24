#!/usr/bin/env python3
"""Fail closed on cargo-audit findings except exact, documented yanked packages."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_FIELDS = (
    "name",
    "version",
    "source",
    "checksum",
    "owner",
    "rationale",
    "review_date",
    "removal_condition",
)
PACKAGE_FIELDS = ("name", "version", "source", "checksum")


def package_key(package: dict[str, Any]) -> tuple[str, str, str, str]:
    values = tuple(package.get(field) for field in PACKAGE_FIELDS)
    if not all(isinstance(value, str) and value.strip() for value in values):
        raise ValueError("cargo-audit warning has incomplete package identity")
    return values  # type: ignore[return-value]


def validate_allowlist(data: object) -> list[dict[str, Any]]:
    if not isinstance(data, dict) or data.get("schema_version") != 1:
        raise ValueError("yanked allowlist must use schema_version 1")
    entries = data.get("yanked")
    if not isinstance(entries, list):
        raise ValueError("yanked allowlist must contain a yanked array")

    seen: set[tuple[str, str, str, str]] = set()
    validated: list[dict[str, Any]] = []
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise ValueError(f"yanked[{index}] must be an object")
        for field in REQUIRED_FIELDS:
            value = entry.get(field)
            if not isinstance(value, str) or not value.strip():
                raise ValueError(f"yanked[{index}] is missing {field}")
        try:
            dt.date.fromisoformat(entry["review_date"])
        except ValueError as error:
            raise ValueError(f"yanked[{index}] has invalid review_date") from error
        key = package_key(entry)
        if key in seen:
            raise ValueError(f"duplicate yanked exception for {key[0]}@{key[1]}")
        seen.add(key)
        validated.append(entry)
    return validated


def validate_report(
    report: object, allowlist: list[dict[str, Any]]
) -> list[str]:
    errors: list[str] = []
    if not isinstance(report, dict):
        return ["cargo-audit report must be a JSON object"]

    vulnerabilities = report.get("vulnerabilities")
    if not isinstance(vulnerabilities, dict):
        errors.append("cargo-audit report is missing vulnerabilities")
    elif (
        vulnerabilities.get("found") is True
        or vulnerabilities.get("count", 0) != 0
        or bool(vulnerabilities.get("list"))
    ):
        errors.append("cargo-audit reported vulnerabilities")

    warnings = report.get("warnings", {})
    if not isinstance(warnings, dict):
        errors.append("cargo-audit warnings must be an object")
        warnings = {}

    actual_yanked: set[tuple[str, str, str, str]] = set()
    for warning_class, items in warnings.items():
        if not isinstance(items, list):
            errors.append(f"cargo-audit warning class {warning_class} is not a list")
            continue
        for item in items:
            if warning_class == "unsound" and isinstance(item, dict):
                # cargo-audit 0.22 reports informational unsoundness notices
                # separately from vulnerabilities. They are not advisories and
                # have no actionable package identity for this policy to match.
                continue
            if warning_class != "yanked" or not isinstance(item, dict) or item.get("kind") != "yanked":
                errors.append(f"unsupported cargo-audit warning class: {warning_class}")
                continue
            package = item.get("package")
            if not isinstance(package, dict):
                errors.append("cargo-audit yanked warning is missing package identity")
                continue
            try:
                actual_yanked.add(package_key(package))
            except ValueError as error:
                errors.append(str(error))

    allowed_yanked = {package_key(entry) for entry in allowlist}
    for key in sorted(actual_yanked - allowed_yanked):
        errors.append(f"unexpected yanked package: {key[0]}@{key[1]}")
    for key in sorted(allowed_yanked - actual_yanked):
        errors.append(f"stale yanked exception: {key[0]}@{key[1]}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--allowlist", type=Path, required=True)
    args = parser.parse_args()
    try:
        allowlist = validate_allowlist(json.loads(args.allowlist.read_text()))
        report = json.load(sys.stdin)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"cargo-audit-policy: ERROR: {error}", file=sys.stderr)
        return 2

    errors = validate_report(report, allowlist)
    if errors:
        for error in errors:
            print(f"cargo-audit-policy: ERROR: {error}", file=sys.stderr)
        return 1

    packages = ", ".join(f"{entry['name']}@{entry['version']}" for entry in allowlist)
    print(f"cargo-audit-policy: OK (exact yanked exceptions: {packages or 'none'})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
