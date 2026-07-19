"""Closed payload contract for coverage-matrix gap reports."""

from __future__ import annotations

import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import PurePosixPath
from typing import Any

from .coverage_matrix_gaps import COVERAGE_STATES, GENERATOR, GENERATOR_VERSION, LIMITATIONS
from .test_catalog_validation import check_supported_schema_keywords


ROOT_FIELDS = {
    "schema_version",
    "generator",
    "generator_version",
    "report_status",
    "input_digest",
    "command",
    "commit",
    "timestamp",
    "platform",
    "input_paths",
    "output_paths",
    "scope",
    "summary",
    "areas",
    "out_of_scope_invariants",
    "limitations",
}
ANALYSIS_FIELDS = {"scope", "summary", "areas", "out_of_scope_invariants", "limitations"}
SCOPE_FIELDS = {
    "area_basis",
    "slot_basis",
    "coverage_states",
    "missing_slot_semantics",
    "case_observation_semantics",
    "debt_is_report_failure",
}
SUMMARY_FIELDS = {
    "mapped_areas",
    "mapped_area_invariants",
    "out_of_scope_invariants",
    "required_family_slots",
    "assigned_required_slots",
    "missing_required_slots",
    "additional_recorded_cells",
    "recorded_cells",
    "case_files",
    "case_observations",
    "blocked_case_observations",
    "state_counts",
}
STATE_COUNT_FIELDS = set(COVERAGE_STATES)
OUTPUT_FIELDS = {"json", "markdown"}
AREA_FIELDS = {
    "area",
    "required_case_families",
    "invariant_count",
    "required_family_slots",
    "assigned_required_slots",
    "missing_required_slots",
    "additional_recorded_cells",
    "invariants",
}
INVARIANT_FIELDS = {
    "id",
    "risk",
    "status",
    "contract_kind",
    "proof_level",
    "linked_test_ids",
    "required_slots",
    "additional_cells",
    "additional_case_families",
}
CELL_FIELDS = {
    "dimension",
    "assignment",
    "coverage_state",
    "rationale",
    "spec_gap_ref",
    "decision_ref",
    "state_issues",
    "case_ids",
    "blocked_case_ids",
}
CASE_FAMILY_FIELDS = {"dimension", "case_ids", "blocked_case_ids"}
OUT_OF_SCOPE_FIELDS = {"id", "area", "recorded_cells"}
ASSIGNMENTS = {
    "assigned",
    "missing_cell",
    "additional_recorded",
    "out_of_scope_recorded",
}
CLEAN_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
DATED_EVIDENCE_RE = re.compile(
    r"^docs/internal/testing/evidence/plc-verification-program/\d{4}-\d{2}-\d{2}/[^/]+\.md$"
)


def validate_report_payload(
    payload: Any,
    *,
    expected_analysis: Mapping[str, Any] | None = None,
) -> list[str]:
    failures: list[str] = []
    if not isinstance(payload, dict):
        return ["report root must be an object"]
    _check_exact_fields(payload, ROOT_FIELDS, "top-level", failures)
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected}")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not CLEAN_COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must identify a clean full Git SHA")
    timestamp = payload.get("timestamp")
    if not isinstance(timestamp, str) or not _is_iso_timestamp(timestamp):
        failures.append("timestamp must be an ISO-8601 value with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload["platform"]:
        failures.append("platform must be a non-empty string")

    command = payload.get("command")
    if not isinstance(command, list) or not command or not all(
        isinstance(item, str) and item for item in command
    ):
        failures.append("command must be a non-empty string array")
    paths = payload.get("input_paths")
    if not isinstance(paths, list) or not all(isinstance(item, str) for item in paths):
        failures.append("input_paths must be a string array")
    else:
        _check_sorted_unique(paths, "input_paths", failures)
        for path in paths:
            if not _is_safe_relative_path(path):
                failures.append(
                    f"input path must be normalized and workspace-relative: {path!r}"
                )
    _validate_output_paths(payload.get("output_paths"), failures)
    _validate_canonical_command(payload, failures)
    _validate_analysis_shape(payload, failures)
    _validate_internal_counts(payload, failures)
    if expected_analysis is not None:
        for field in sorted(ANALYSIS_FIELDS):
            if payload.get(field) != expected_analysis.get(field):
                failures.append(f"{field} does not match current coverage analysis")
    return failures


def validate_schema_contract(schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("coverage-matrix gap schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("coverage-matrix gap schema root required fields drift")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"coverage-matrix gap schema const for {field} drifts")
    if properties.get("commit", {}).get("pattern") != "^[0-9a-f]{40}$":
        failures.append("coverage-matrix gap schema clean commit pattern drifts")

    definitions = schema.get("$defs", {})
    for name, expected_fields in (
        ("output_paths", OUTPUT_FIELDS),
        ("scope", SCOPE_FIELDS),
        ("state_counts", STATE_COUNT_FIELDS),
        ("summary", SUMMARY_FIELDS),
        ("cell", CELL_FIELDS),
        ("case_family", CASE_FAMILY_FIELDS),
        ("invariant", INVARIANT_FIELDS),
        ("area", AREA_FIELDS),
        ("out_of_scope", OUT_OF_SCOPE_FIELDS),
    ):
        definition = definitions.get(name, {}) if isinstance(definitions, dict) else {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"coverage-matrix gap schema {name} must be a closed object")
        if set(definition.get("required", [])) != expected_fields:
            failures.append(f"coverage-matrix gap schema {name} fields drift")

    scope = definitions.get("scope", {}) if isinstance(definitions, dict) else {}
    scope_properties = scope.get("properties", {}) if isinstance(scope, dict) else {}
    expected_scope_consts = {
        "area_basis": "planning_matrix_status_mapped",
        "slot_basis": "mapped_area_invariant_x_required_case_family",
        "missing_slot_semantics": "structural_debt_without_synthetic_state",
        "case_observation_semantics": "planning_observation_only_never_state_upgrade",
        "debt_is_report_failure": False,
    }
    for field, expected in expected_scope_consts.items():
        if scope_properties.get(field, {}).get("const") != expected:
            failures.append(f"coverage-matrix gap schema const for scope.{field} drifts")
    coverage_state = definitions.get("coverage_state", {})
    if set(coverage_state.get("enum", [])) != set(COVERAGE_STATES):
        failures.append("coverage-matrix gap schema coverage_state enum drifts")
    cell = definitions.get("cell", {}) if isinstance(definitions, dict) else {}
    cell_properties = cell.get("properties", {}) if isinstance(cell, dict) else {}
    if set(cell_properties.get("assignment", {}).get("enum", [])) != ASSIGNMENTS:
        failures.append("coverage-matrix gap schema assignment enum drifts")
    if set(cell_properties.get("coverage_state", {}).get("enum", [])) != {
        None,
        *COVERAGE_STATES,
    }:
        failures.append("coverage-matrix gap schema nullable coverage_state enum drifts")
    scope_state_ref = scope_properties.get("coverage_states", {}).get("items", {}).get("$ref")
    if scope_state_ref != "#/$defs/coverage_state":
        failures.append("coverage-matrix gap schema scope coverage_states ref drifts")
    return failures


def _validate_analysis_shape(payload: Mapping[str, Any], failures: list[str]) -> None:
    expected_scope = {
        "area_basis": "planning_matrix_status_mapped",
        "slot_basis": "mapped_area_invariant_x_required_case_family",
        "coverage_states": list(COVERAGE_STATES),
        "missing_slot_semantics": "structural_debt_without_synthetic_state",
        "case_observation_semantics": "planning_observation_only_never_state_upgrade",
        "debt_is_report_failure": False,
    }
    scope = payload.get("scope")
    if not isinstance(scope, dict):
        failures.append("scope must be an object")
    else:
        _check_exact_fields(scope, SCOPE_FIELDS, "scope", failures)
        if scope != expected_scope:
            failures.append("scope does not match the coverage-matrix gap contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the coverage-matrix gap contract")

    summary = payload.get("summary")
    if not isinstance(summary, dict):
        failures.append("summary must be an object")
    else:
        _check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
        for field in SUMMARY_FIELDS - {"state_counts"}:
            _check_nonnegative(summary.get(field), f"summary.{field}", failures)
        counts = summary.get("state_counts")
        if not isinstance(counts, dict):
            failures.append("summary.state_counts must be an object")
        else:
            _check_exact_fields(counts, STATE_COUNT_FIELDS, "summary.state_counts", failures)
            for state in COVERAGE_STATES:
                _check_nonnegative(counts.get(state), f"summary.state_counts.{state}", failures)

    areas = payload.get("areas")
    if not isinstance(areas, list):
        failures.append("areas must be an array")
    else:
        for area_index, area in enumerate(areas):
            _validate_area(area, area_index, failures)
        area_ids = [area.get("area") for area in areas if isinstance(area, dict)]
        if area_ids != sorted(set(area_ids)):
            failures.append("areas must use canonical unique area order")

    out_of_scope = payload.get("out_of_scope_invariants")
    if not isinstance(out_of_scope, list):
        failures.append("out_of_scope_invariants must be an array")
    else:
        for index, invariant in enumerate(out_of_scope):
            label = f"out_of_scope_invariants[{index}]"
            if not isinstance(invariant, dict):
                failures.append(f"{label} must be an object")
                continue
            _check_exact_fields(invariant, OUT_OF_SCOPE_FIELDS, label, failures)
            _check_nonempty_string(invariant.get("id"), f"{label}.id", failures)
            _check_nonempty_string(invariant.get("area"), f"{label}.area", failures)
            _validate_cells(
                invariant.get("recorded_cells"),
                f"{label}.recorded_cells",
                {"out_of_scope_recorded"},
                failures,
            )
        order = [
            (item.get("id"), item.get("area"))
            for item in out_of_scope
            if isinstance(item, dict)
        ]
        if order != sorted(set(order)):
            failures.append("out_of_scope_invariants must use canonical unique order")


def _validate_area(value: Any, index: int, failures: list[str]) -> None:
    label = f"areas[{index}]"
    if not isinstance(value, dict):
        failures.append(f"{label} must be an object")
        return
    _check_exact_fields(value, AREA_FIELDS, label, failures)
    _check_nonempty_string(value.get("area"), f"{label}.area", failures)
    families = value.get("required_case_families")
    if not isinstance(families, list) or not all(
        isinstance(item, str) and item for item in families
    ):
        failures.append(f"{label}.required_case_families must be a string array")
    else:
        _check_sorted_unique(families, f"{label}.required_case_families", failures)
    for field in AREA_FIELDS - {"area", "required_case_families", "invariants"}:
        _check_nonnegative(value.get(field), f"{label}.{field}", failures)
    invariants = value.get("invariants")
    if not isinstance(invariants, list):
        failures.append(f"{label}.invariants must be an array")
        return
    for invariant_index, invariant in enumerate(invariants):
        invariant_label = f"{label}.invariants[{invariant_index}]"
        if not isinstance(invariant, dict):
            failures.append(f"{invariant_label} must be an object")
            continue
        _check_exact_fields(invariant, INVARIANT_FIELDS, invariant_label, failures)
        for field in ("id", "risk", "status", "contract_kind", "proof_level"):
            _check_nonempty_string(invariant.get(field), f"{invariant_label}.{field}", failures)
        linked = invariant.get("linked_test_ids")
        if not isinstance(linked, list) or not all(isinstance(item, str) for item in linked):
            failures.append(f"{invariant_label}.linked_test_ids must be a string array")
        else:
            _check_sorted_unique(linked, f"{invariant_label}.linked_test_ids", failures)
        _validate_cells(
            invariant.get("required_slots"),
            f"{invariant_label}.required_slots",
            {"assigned", "missing_cell"},
            failures,
        )
        _validate_cells(
            invariant.get("additional_cells"),
            f"{invariant_label}.additional_cells",
            {"additional_recorded"},
            failures,
        )
        case_families = invariant.get("additional_case_families")
        if not isinstance(case_families, list):
            failures.append(f"{invariant_label}.additional_case_families must be an array")
        else:
            for case_index, case_family in enumerate(case_families):
                case_label = f"{invariant_label}.additional_case_families[{case_index}]"
                _validate_case_family(case_family, case_label, failures)
            _check_dimension_order(
                case_families,
                f"{invariant_label}.additional_case_families",
                failures,
            )
    ids = [invariant.get("id") for invariant in invariants if isinstance(invariant, dict)]
    if ids != sorted(set(ids)):
        failures.append(f"{label}.invariants must use canonical unique id order")


def _validate_cells(
    value: Any,
    label: str,
    allowed_assignments: set[str],
    failures: list[str],
) -> None:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return
    for index, cell in enumerate(value):
        cell_label = f"{label}[{index}]"
        if not isinstance(cell, dict):
            failures.append(f"{cell_label} must be an object")
            continue
        _check_exact_fields(cell, CELL_FIELDS, cell_label, failures)
        _check_nonempty_string(cell.get("dimension"), f"{cell_label}.dimension", failures)
        assignment = cell.get("assignment")
        if assignment not in allowed_assignments:
            failures.append(f"{cell_label}.assignment is invalid for its report section")
        state = cell.get("coverage_state")
        if assignment == "missing_cell":
            if state is not None:
                failures.append(f"{cell_label} missing slot must not synthesize coverage_state")
            for field in ("rationale", "spec_gap_ref", "decision_ref"):
                if cell.get(field) is not None:
                    failures.append(f"{cell_label}.{field} must be null for a missing slot")
            if cell.get("state_issues") != []:
                failures.append(f"{cell_label}.state_issues must be empty for a missing slot")
        elif state not in COVERAGE_STATES:
            failures.append(f"{cell_label}.coverage_state is unsupported")
        elif not isinstance(cell.get("rationale"), str) or not cell["rationale"].strip():
            failures.append(f"{cell_label}.rationale must be non-empty for a declared cell")
        for field in ("spec_gap_ref", "decision_ref"):
            if cell.get(field) is not None and not isinstance(cell.get(field), str):
                failures.append(f"{cell_label}.{field} must be a string or null")
        for field in ("state_issues", "case_ids", "blocked_case_ids"):
            items = cell.get(field)
            if not isinstance(items, list) or not all(isinstance(item, str) for item in items):
                failures.append(f"{cell_label}.{field} must be a string array")
            else:
                _check_sorted_unique(items, f"{cell_label}.{field}", failures)
        case_ids = cell.get("case_ids")
        blocked_ids = cell.get("blocked_case_ids")
        if isinstance(case_ids, list) and isinstance(blocked_ids, list):
            if not set(blocked_ids).issubset(case_ids):
                failures.append(f"{cell_label}.blocked_case_ids must be a subset of case_ids")
    _check_dimension_order(value, label, failures)


def _validate_case_family(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append(f"{label} must be an object")
        return
    _check_exact_fields(value, CASE_FAMILY_FIELDS, label, failures)
    _check_nonempty_string(value.get("dimension"), f"{label}.dimension", failures)
    for field in ("case_ids", "blocked_case_ids"):
        items = value.get(field)
        if not isinstance(items, list) or not all(isinstance(item, str) for item in items):
            failures.append(f"{label}.{field} must be a string array")
        else:
            _check_sorted_unique(items, f"{label}.{field}", failures)
    if isinstance(value.get("case_ids"), list) and isinstance(value.get("blocked_case_ids"), list):
        if not set(value["blocked_case_ids"]).issubset(value["case_ids"]):
            failures.append(f"{label}.blocked_case_ids must be a subset of case_ids")


def _validate_internal_counts(payload: Mapping[str, Any], failures: list[str]) -> None:
    summary = payload.get("summary")
    areas = payload.get("areas")
    out_of_scope = payload.get("out_of_scope_invariants")
    if not isinstance(summary, dict) or not isinstance(areas, list):
        return
    valid_areas = [area for area in areas if isinstance(area, dict)]
    if summary.get("mapped_areas") != len(valid_areas):
        failures.append("summary.mapped_areas does not match areas")
    invariants = [
        invariant
        for area in valid_areas
        for invariant in area.get("invariants", [])
        if isinstance(invariant, dict)
    ]
    if summary.get("mapped_area_invariants") != len(invariants):
        failures.append("summary.mapped_area_invariants does not match areas")
    if isinstance(out_of_scope, list) and summary.get("out_of_scope_invariants") != len(out_of_scope):
        failures.append("summary.out_of_scope_invariants does not match report rows")

    required = [
        slot
        for invariant in invariants
        for slot in invariant.get("required_slots", [])
        if isinstance(slot, dict)
    ]
    additional = [
        slot
        for invariant in invariants
        for slot in invariant.get("additional_cells", [])
        if isinstance(slot, dict)
    ]
    assigned = sum(slot.get("assignment") == "assigned" for slot in required)
    missing = sum(slot.get("assignment") == "missing_cell" for slot in required)
    expected = {
        "required_family_slots": len(required),
        "assigned_required_slots": assigned,
        "missing_required_slots": missing,
        "additional_recorded_cells": len(additional),
        "recorded_cells": assigned + len(additional),
    }
    for field, value in expected.items():
        if summary.get(field) != value:
            failures.append(f"summary.{field} does not match report rows")
    if len(required) != assigned + missing:
        failures.append("required slots are not an exhaustive assigned/missing partition")

    observed_cells = [*required, *additional]
    case_only_families = [
        family
        for invariant in invariants
        for family in invariant.get("additional_case_families", [])
        if isinstance(family, dict)
    ]
    observed_case_ids = {
        case_id for cell in observed_cells for case_id in cell.get("case_ids", [])
    }
    observed_case_ids.update(
        case_id
        for family in case_only_families
        for case_id in family.get("case_ids", [])
    )
    observed_blocked_ids = {
        case_id for cell in observed_cells for case_id in cell.get("blocked_case_ids", [])
    }
    observed_blocked_ids.update(
        case_id
        for family in case_only_families
        for case_id in family.get("blocked_case_ids", [])
    )
    if summary.get("case_observations") != len(observed_case_ids):
        failures.append("summary.case_observations does not match mapped-area observations")
    if summary.get("blocked_case_observations") != len(observed_blocked_ids):
        failures.append("summary.blocked_case_observations does not match mapped-area observations")
    counts = summary.get("state_counts")
    if isinstance(counts, dict):
        actual = {state: 0 for state in COVERAGE_STATES}
        for cell in observed_cells:
            state = cell.get("coverage_state")
            if state in actual:
                actual[state] += 1
        if counts != actual:
            failures.append("summary.state_counts does not match declared mapped-area cells")

    for area_index, area in enumerate(valid_areas):
        area_invariants = [
            item for item in area.get("invariants", []) if isinstance(item, dict)
        ]
        area_required = [
            slot
            for invariant in area_invariants
            for slot in invariant.get("required_slots", [])
            if isinstance(slot, dict)
        ]
        area_additional = [
            slot
            for invariant in area_invariants
            for slot in invariant.get("additional_cells", [])
            if isinstance(slot, dict)
        ]
        area_expected = {
            "invariant_count": len(area_invariants),
            "required_family_slots": len(area_required),
            "assigned_required_slots": sum(
                slot.get("assignment") == "assigned" for slot in area_required
            ),
            "missing_required_slots": sum(
                slot.get("assignment") == "missing_cell" for slot in area_required
            ),
            "additional_recorded_cells": len(area_additional),
        }
        for field, value in area_expected.items():
            if area.get(field) != value:
                failures.append(f"areas[{area_index}].{field} does not match report rows")


def _validate_output_paths(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("output_paths must be an object")
        return
    _check_exact_fields(value, OUTPUT_FIELDS, "output_paths", failures)
    json_path = value.get("json")
    markdown_path = value.get("markdown")
    if not isinstance(json_path, str) or not json_path.startswith(
        "target/gate-artifacts/verification/"
    ):
        failures.append("output_paths.json must be under target/gate-artifacts/verification")
    elif not _is_safe_relative_path(json_path):
        failures.append("output_paths.json must be workspace-relative")
    if not isinstance(markdown_path, str) or not (
        markdown_path.startswith("target/gate-artifacts/verification/")
        or DATED_EVIDENCE_RE.fullmatch(markdown_path)
    ):
        failures.append(
            "output_paths.markdown must be under target/gate-artifacts/verification "
            "or a dated PLC verification evidence path"
        )
    elif not _is_safe_relative_path(markdown_path):
        failures.append("output_paths.markdown must be workspace-relative")


def _validate_canonical_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, dict) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_coverage_matrix_gaps.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical coverage-matrix gap invocation")


def _check_exact_fields(
    value: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    actual = set(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing:
        failures.append(f"{label} missing fields: {', '.join(missing)}")
    if extra:
        failures.append(f"{label} has unexpected fields: {', '.join(extra)}")


def _check_sorted_unique(values: list[str], label: str, failures: list[str]) -> None:
    if values != sorted(set(values)):
        failures.append(f"{label} must be sorted and duplicate-free")


def _check_nonnegative(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        failures.append(f"{label} must be a nonnegative integer")


def _check_nonempty_string(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, str) or not value:
        failures.append(f"{label} must be a non-empty string")


def _check_dimension_order(value: list[Any], label: str, failures: list[str]) -> None:
    dimensions = [item.get("dimension") for item in value if isinstance(item, dict)]
    if dimensions != sorted(set(dimensions)):
        failures.append(f"{label} must use canonical unique dimension order")


def _is_iso_timestamp(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _is_safe_relative_path(value: str) -> bool:
    if not value or "\\" in value or value.startswith("/"):
        return False
    path = PurePosixPath(value)
    return not any(part in {"", ".", ".."} for part in path.parts)
