"""Provenance and shape rules for generated and hand-authored case files."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Callable

from .oracle_refs import validate_oracle_ref
from ..case_contract_fields import TRACE_STEP_FIELDS


Fail = Callable[[Path, str], None]

GENERATED_DECISION_TABLE_V1 = "generated_decision_table_v1"
HAND_AUTHORED_STATE_MACHINE_V1 = "hand_authored_state_machine_v1"
CASE_PROVENANCE_KINDS = {
    GENERATED_DECISION_TABLE_V1,
    HAND_AUTHORED_STATE_MACHINE_V1,
}


def trace_definition_digest(cases: Any) -> str:
    """Return the canonical digest of case IDs and their trace definitions."""

    definitions = []
    if isinstance(cases, list):
        for case in cases:
            if isinstance(case, dict):
                definitions.append(
                    {
                        "id": case.get("id"),
                        "trace": case.get("trace"),
                    }
                )
            else:
                definitions.append({"id": None, "trace": None})
    encoded = json.dumps(
        definitions,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def validate_case_provenance(
    *,
    fail: Fail,
    path: Path,
    test_id: str,
    case_data: dict[str, Any],
    invariant: dict[str, Any],
    spec_sources: dict[str, dict[str, Any]],
    expected_generator_digest: str,
    expected_generator_v2_digest: str | None = None,
    expected_source_digest: str,
    expected_generator_v2_source_digest: str | None = None,
) -> str:
    """Validate the mutually exclusive generated and hand-authored contracts."""

    kind = case_data.get("case_provenance_kind", GENERATED_DECISION_TABLE_V1)
    if kind not in CASE_PROVENANCE_KINDS:
        fail(path, f"{test_id} case_file has unknown case_provenance_kind {kind!r}")
        return str(kind)

    expected_case_source_digest = expected_source_digest
    if (
        kind == GENERATED_DECISION_TABLE_V1
        and case_data.get("generator") == "gen_cases_v2.py v1"
    ):
        expected_case_source_digest = expected_generator_v2_source_digest
    if (
        expected_case_source_digest is None
        or case_data.get("source_digest") != expected_case_source_digest
    ):
        fail(
            path,
            f"{test_id} case_file source_digest mismatch: expected "
            f"{expected_case_source_digest}, actual {case_data.get('source_digest')}",
        )

    if kind == GENERATED_DECISION_TABLE_V1:
        _validate_generated_case(
            fail=fail,
            path=path,
            test_id=test_id,
            case_data=case_data,
            expected_generator_digest=expected_generator_digest,
            expected_generator_v2_digest=expected_generator_v2_digest,
        )
    else:
        _validate_hand_authored_state_machine(
            fail=fail,
            path=path,
            test_id=test_id,
            case_data=case_data,
            invariant=invariant,
            spec_sources=spec_sources,
        )
    return kind


def _validate_generated_case(
    *,
    fail: Fail,
    path: Path,
    test_id: str,
    case_data: dict[str, Any],
    expected_generator_digest: str,
    expected_generator_v2_digest: str | None,
) -> None:
    generator = case_data.get("generator")
    expected_by_generator = {
        "gen_cases.py v1": expected_generator_digest,
        "gen_cases_v2.py v1": expected_generator_v2_digest,
    }
    if generator not in expected_by_generator:
        fail(path, f"{test_id} generated case_file names unknown generator {generator!r}")
    expected_digest = expected_by_generator.get(generator)
    if expected_digest is None or case_data.get("generator_digest") != expected_digest:
        fail(
            path,
            f"{test_id} case_file generator_digest mismatch: expected "
            f"{expected_digest}, actual {case_data.get('generator_digest')}",
        )
    if "trace_definition_digest" in case_data:
        fail(path, f"{test_id} generated case_file forbids trace_definition_digest")
    if any(isinstance(case, dict) and "trace" in case for case in case_data.get("case", [])):
        fail(path, f"{test_id} generated case_file forbids case trace records")


def _validate_hand_authored_state_machine(
    *,
    fail: Fail,
    path: Path,
    test_id: str,
    case_data: dict[str, Any],
    invariant: dict[str, Any],
    spec_sources: dict[str, dict[str, Any]],
) -> None:
    if "generator" in case_data or "generator_digest" in case_data:
        fail(
            path,
            f"{test_id} hand-authored case_file forbids generator and generator_digest",
        )
    if invariant.get("contract_kind") not in {"state_machine", "protocol_trace"}:
        fail(
            path,
            f"{test_id} hand-authored case_file requires invariant "
            "contract_kind = state_machine or protocol_trace",
        )
    cases = case_data.get("case")
    if not isinstance(cases, list) or not cases:
        return
    for case in cases:
        _validate_trace_case(fail, path, test_id, case, spec_sources)
    try:
        expected_digest = trace_definition_digest(cases)
    except (TypeError, ValueError) as exc:
        fail(path, f"{test_id} case_file trace definition is not canonical JSON: {exc}")
        return
    if case_data.get("trace_definition_digest") != expected_digest:
        fail(
            path,
            f"{test_id} case_file trace_definition_digest mismatch: expected "
            f"{expected_digest}, actual {case_data.get('trace_definition_digest')}",
        )


def _validate_trace_case(
    fail: Fail,
    path: Path,
    test_id: str,
    case: Any,
    spec_sources: dict[str, dict[str, Any]],
) -> None:
    if not isinstance(case, dict):
        return
    case_id = case.get("id")
    if case.get("state") == "blocked":
        if "trace" in case:
            fail(
                path,
                f"{test_id} blocked hand-authored case {case_id} forbids a "
                "trace with asserted expected states",
            )
        return
    if "expect" not in case:
        fail(path, f"{test_id} hand-authored case {case_id} must carry expect")
    steps = case.get("trace")
    if not isinstance(steps, list) or not steps:
        fail(path, f"{test_id} hand-authored case {case_id} must carry a non-empty trace")
        return
    for expected_sequence, step in enumerate(steps):
        label = f"{test_id} hand-authored case {case_id} trace step {expected_sequence}"
        if not isinstance(step, dict):
            fail(path, f"{label} must be a table")
            continue
        if set(step) != TRACE_STEP_FIELDS:
            fail(
                path,
                f"{label} trace step fields must be exactly "
                f"{sorted(TRACE_STEP_FIELDS)}",
            )
        sequence = step.get("sequence")
        if isinstance(sequence, bool) or sequence != expected_sequence:
            fail(path, f"{label} trace sequence must be contiguous from zero")
        for field in ("stimulus", "expected"):
            if not isinstance(step.get(field), dict) or not step.get(field):
                fail(path, f"{label} trace {field} must be a non-empty table")
            elif _contains_toml_float(step[field]):
                fail(
                    path,
                    f"{label} trace {field} must not contain TOML floats; "
                    "use integer units for cross-language digest parity",
                )
            elif not _is_canonical_trace_value(step[field]):
                fail(
                    path,
                    f"{label} trace {field} must contain only canonical JSON values",
                )
        validate_oracle_ref(
            fail=fail,
            path=path,
            owner_id=label,
            oracle_ref=step.get("oracle_ref"),
            spec_sources=spec_sources,
        )


def _is_canonical_trace_value(value: Any) -> bool:
    if value is None or isinstance(value, (str, bool, int)):
        return True
    if isinstance(value, list):
        return all(_is_canonical_trace_value(item) for item in value)
    if isinstance(value, dict):
        return all(
            isinstance(key, str) and _is_canonical_trace_value(item)
            for key, item in value.items()
        )
    return False


def _contains_toml_float(value: Any) -> bool:
    if isinstance(value, float):
        return True
    if isinstance(value, list):
        return any(_contains_toml_float(item) for item in value)
    if isinstance(value, dict):
        return any(_contains_toml_float(item) for item in value.values())
    return False
