"""Validation for committed verification case files."""

from __future__ import annotations

import tomllib
from pathlib import Path
from typing import Any, Callable

from ..case_digests import current_generator_digest, file_digest

from .constants import CASE_FAMILIES, OUTCOMES, ROOT, SCHEMA_REQUIRED_FIELDS
from .oracle_refs import validate_oracle_ref


Fail = Callable[[Path, str], None]


def validate_case_file(
    *,
    fail: Fail,
    path: Path,
    test_record: dict[str, Any],
    invariants: dict[str, dict[str, Any]],
    spec_sources: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
) -> None:
    case_file = test_record.get("case_file")
    if not case_file:
        return
    case_path = ROOT / case_file
    if not case_path.exists():
        return
    try:
        case_data = tomllib.loads(case_path.read_text())
    except Exception as exc:
        fail(path, f"{test_record['id']} case_file TOML parse failed: {exc}")
        return

    required = SCHEMA_REQUIRED_FIELDS["case-file.schema.json"]
    for field in required:
        if field not in case_data:
            fail(path, f"{test_record['id']} case_file missing {field}")
    if case_data.get("schema_version") != 1:
        fail(path, f"{test_record['id']} case_file must use schema_version = 1")

    invariant_id = case_data.get("invariant")
    invariant = invariants.get(invariant_id)
    if invariant is None:
        fail(path, f"{test_record['id']} case_file references unknown invariant {invariant_id!r}")
        return
    if invariant_id not in test_record.get("invariants", []):
        fail(path, f"{test_record['id']} case_file invariant {invariant_id} is not listed in test invariants")
    if case_data.get("area") != invariant.get("area"):
        fail(path, f"{test_record['id']} case_file area does not match invariant area")

    expected_generator_digest = current_generator_digest()
    if case_data.get("generator_digest") != expected_generator_digest:
        fail(
            path,
            f"{test_record['id']} case_file generator_digest mismatch: expected {expected_generator_digest}, actual {case_data.get('generator_digest')}",
        )
    expected_source_digest = file_digest(invariant["_path"])
    if case_data.get("source_digest") != expected_source_digest:
        fail(
            path,
            f"{test_record['id']} case_file source_digest mismatch: expected {expected_source_digest}, actual {case_data.get('source_digest')}",
        )

    cases = case_data.get("case")
    if not isinstance(cases, list) or not cases:
        fail(path, f"{test_record['id']} case_file must contain at least one [[case]]")
        return
    seen_case_ids: set[str] = set()
    for case in cases:
        validate_case_record(fail, path, test_record["id"], case, invariant, spec_sources, spec_gaps, seen_case_ids)


def validate_case_record(
    fail: Fail,
    path: Path,
    test_id: str,
    case: Any,
    invariant: dict[str, Any],
    spec_sources: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
    seen_case_ids: set[str],
) -> None:
    if not isinstance(case, dict):
        fail(path, f"{test_id} case_file has non-table case entry")
        return
    for field in ("id", "family", "input"):
        if field not in case:
            fail(path, f"{test_id} case missing {field}")
    case_id = case.get("id")
    if case_id in seen_case_ids:
        fail(path, f"{test_id} duplicate case id {case_id}")
    if isinstance(case_id, str):
        seen_case_ids.add(case_id)
    if case.get("family") not in CASE_FAMILIES:
        fail(path, f"{test_id} case {case_id} uses unknown family {case.get('family')!r}")
    if not isinstance(case.get("input"), dict) or not case.get("input"):
        fail(path, f"{test_id} case {case_id} must carry input table")
    validate_shape_descriptor(fail, path, test_id, case_id, case.get("input"), invariant)

    has_expect = "expect" in case
    is_blocked = case.get("state") == "blocked"
    if has_expect == is_blocked:
        fail(path, f"{test_id} case {case_id} must have exactly one of expect or state = blocked")
    if is_blocked:
        gap_id = case.get("spec_gap_ref")
        if gap_id not in spec_gaps:
            fail(path, f"{test_id} blocked case {case_id} references unknown spec_gap_ref {gap_id!r}")
        if "expect" in case:
            fail(path, f"{test_id} blocked case {case_id} cannot carry expect")
    if has_expect:
        expect = case.get("expect")
        if not isinstance(expect, dict):
            fail(path, f"{test_id} case {case_id} expect must be a table")
            return
        if expect.get("outcome") not in OUTCOMES:
            fail(path, f"{test_id} case {case_id} expect has unknown outcome {expect.get('outcome')!r}")
        if "oracle_ref" not in expect:
            fail(path, f"{test_id} case {case_id} expect must name oracle_ref")
        else:
            validate_oracle_ref(
                fail=fail,
                path=path,
                owner_id=f"{test_id} case {case_id}",
                oracle_ref=expect["oracle_ref"],
                spec_sources=spec_sources,
            )
        if expect not in oracle_backed_expected_rows(invariant):
            fail(path, f"{test_id} case {case_id} expect does not match an oracle-backed behavior row")


def validate_shape_descriptor(
    fail: Fail,
    path: Path,
    test_id: str,
    case_id: Any,
    case_input: Any,
    invariant: dict[str, Any],
) -> None:
    if not isinstance(case_input, dict):
        return
    source_partition = case_input.get("source_partition")
    if not isinstance(source_partition, dict):
        return
    if "wrong_type" not in source_partition and "malformed" not in source_partition:
        return
    if "shape_descriptor" not in case_input:
        fail(path, f"{test_id} case {case_id} wrong-type/malformed input must use shape_descriptor")
    typed_input_name = invariant.get("input", {}).get("name")
    if typed_input_name and typed_input_name in case_input:
        fail(path, f"{test_id} case {case_id} wrong-type/malformed input cannot use typed field {typed_input_name!r}")


def oracle_backed_expected_rows(invariant: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        copy_expected_behavior(behavior)
        for behavior in invariant.get("behavior", [])
        if isinstance(behavior, dict) and "oracle_ref" in behavior
    ]


def copy_expected_behavior(behavior: dict[str, Any]) -> dict[str, Any]:
    fields = [
        "outcome",
        "delta",
        "error_code",
        "no_partial_apply",
        "fault_surface",
        "oracle_ref",
    ]
    return {field: behavior[field] for field in fields if field in behavior}
