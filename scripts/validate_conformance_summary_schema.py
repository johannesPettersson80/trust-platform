#!/usr/bin/env python3
"""Validate truST conformance summary schemas and generated summaries.

The CI environment intentionally avoids downloading JSON Schema dependencies.
This validator enforces the repository's schema-owned contract subset that the
conformance gate relies on: version/profile constants, category sets, required
summary fields, deterministic result ordering, totals, paths, statuses, and
reason codes.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{path}: invalid JSON: {exc}") from exc


def schema_contract(path: Path) -> dict[str, Any]:
    schema = load_json(path)
    properties = schema.get("properties", {})
    defs = schema.get("$defs", {})
    result = defs.get("result", {})
    result_properties = result.get("properties", {})

    version = properties.get("version", {}).get("const")
    profile = properties.get("profile", {}).get("const")
    ordering = properties.get("ordering", {}).get("const")
    categories = result_properties.get("category", {}).get("enum")
    case_id_pattern = result_properties.get("case_id", {}).get("pattern")
    expected_ref_pattern = result_properties.get("expected_ref", {}).get("pattern")
    reason_codes = (
        result_properties.get("reason", {})
        .get("properties", {})
        .get("code", {})
        .get("enum", [])
    )

    if not isinstance(version, int):
        raise SystemExit(f"{path}: schema must define integer properties.version.const")
    if not isinstance(profile, str) or not profile:
        raise SystemExit(f"{path}: schema must define non-empty properties.profile.const")
    if ordering != "case_id_asc":
        raise SystemExit(f"{path}: schema ordering const must be case_id_asc")
    if not isinstance(categories, list) or not categories:
        raise SystemExit(f"{path}: schema must define non-empty result category enum")
    if not isinstance(case_id_pattern, str) or not case_id_pattern:
        raise SystemExit(f"{path}: schema must define result case_id pattern")
    if not isinstance(expected_ref_pattern, str) or not expected_ref_pattern:
        raise SystemExit(f"{path}: schema must define result expected_ref pattern")
    if not reason_codes:
        raise SystemExit(f"{path}: schema must define reason code enum")

    return {
        "path": path,
        "version": version,
        "profile": profile,
        "categories": set(categories),
        "case_id_re": re.compile(case_id_pattern),
        "expected_ref_re": re.compile(expected_ref_pattern),
        "reason_codes": set(reason_codes),
    }


def validate_summary(path: Path, contracts: dict[tuple[int, str], dict[str, Any]]) -> None:
    summary = load_json(path)
    version = summary.get("version")
    profile = summary.get("profile")
    contract = contracts.get((version, profile))
    if contract is None:
        known = ", ".join(
            f"{item[0]}/{item[1]}" for item in sorted(contracts)
        )
        raise SystemExit(f"{path}: unsupported version/profile {version}/{profile}; known {known}")

    required = ["generated_at_utc", "ordering", "runtime", "summary", "results"]
    missing = [name for name in required if name not in summary]
    if missing:
        raise SystemExit(f"{path}: missing required fields: {', '.join(missing)}")
    if summary["ordering"] != "case_id_asc":
        raise SystemExit(f"{path}: ordering must be case_id_asc")

    results = summary["results"]
    if not isinstance(results, list):
        raise SystemExit(f"{path}: results must be an array")
    case_ids = [case.get("case_id") for case in results]
    if case_ids != sorted(case_ids):
        raise SystemExit(f"{path}: results are not sorted by case_id")

    totals = summary["summary"]
    for key in ["total", "passed", "failed", "errors", "skipped"]:
        if not isinstance(totals.get(key), int) or totals[key] < 0:
            raise SystemExit(f"{path}: summary.{key} must be a non-negative integer")
    if totals["total"] != len(results):
        raise SystemExit(f"{path}: summary.total does not match result count")
    status_counts = {"passed": 0, "failed": 0, "error": 0, "skipped": 0}

    for case in results:
        validate_case(path, contract, case)
        status_counts[case["status"]] += 1

    if totals["passed"] != status_counts["passed"]:
        raise SystemExit(f"{path}: summary.passed does not match results")
    if totals["failed"] != status_counts["failed"]:
        raise SystemExit(f"{path}: summary.failed does not match results")
    if totals["errors"] != status_counts["error"]:
        raise SystemExit(f"{path}: summary.errors does not match results")
    if totals["skipped"] != status_counts["skipped"]:
        raise SystemExit(f"{path}: summary.skipped does not match results")


def validate_case(path: Path, contract: dict[str, Any], case: dict[str, Any]) -> None:
    for key in ["case_id", "category", "status", "expected_ref"]:
        if key not in case:
            raise SystemExit(f"{path}: result missing {key}: {case}")
    case_id = case["case_id"]
    category = case["category"]
    if category not in contract["categories"]:
        raise SystemExit(f"{path}: unsupported category {category} in {case_id}")
    if not contract["case_id_re"].match(case_id):
        raise SystemExit(f"{path}: case_id does not match schema pattern: {case_id}")
    if not case_id.startswith(f"cfm_{category}_"):
        raise SystemExit(f"{path}: case_id/category mismatch: {case_id} vs {category}")
    expected_ref = case["expected_ref"]
    expected_exact = f"expected/{category}/{case_id}.json"
    if expected_ref != expected_exact:
        raise SystemExit(
            f"{path}: expected_ref must be {expected_exact}, got {expected_ref}"
        )
    if not contract["expected_ref_re"].match(expected_ref):
        raise SystemExit(f"{path}: expected_ref does not match schema pattern: {expected_ref}")
    if case["status"] not in {"passed", "failed", "error", "skipped"}:
        raise SystemExit(f"{path}: invalid status {case['status']} in {case_id}")
    if "duration_ms" in case and (not isinstance(case["duration_ms"], int) or case["duration_ms"] < 0):
        raise SystemExit(f"{path}: duration_ms must be non-negative integer in {case_id}")
    if "cycles" in case and (not isinstance(case["cycles"], int) or case["cycles"] < 0):
        raise SystemExit(f"{path}: cycles must be non-negative integer in {case_id}")
    reason = case.get("reason")
    if reason is not None:
        code = reason.get("code")
        if code not in contract["reason_codes"]:
            raise SystemExit(f"{path}: unsupported reason code {code} in {case_id}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--schema", action="append", required=True, type=Path)
    parser.add_argument("--summary", action="append", default=[], type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    contracts = {}
    for path in args.schema:
        contract = schema_contract(path)
        key = (contract["version"], contract["profile"])
        if key in contracts:
            raise SystemExit(f"duplicate schema contract for {key}")
        contracts[key] = contract
        print(f"validated schema {path} ({key[0]}/{key[1]})")

    for path in args.summary:
        validate_summary(path, contracts)
        print(f"validated summary {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
