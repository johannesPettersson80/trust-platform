"""Closed payload and schema contract for specification-completeness reports."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import PurePosixPath
from typing import Any

from .spec_completeness_report import (
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    ORACLE_BINDING_FIELDS,
    PILOT_AREA,
    PILOT_CLASSIFICATIONS,
    SpecCompletenessProvenance,
    SpecCompletenessReport,
)
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
    "invariants_without_spec",
    "tests_without_oracle",
    "spec_gap_cells",
    "bytecode_pilot",
    "public_claim_context",
    "limitations",
}
ANALYSIS_FIELDS = {
    "scope",
    "summary",
    "invariants_without_spec",
    "tests_without_oracle",
    "spec_gap_cells",
    "bytecode_pilot",
    "public_claim_context",
    "limitations",
}
OUTPUT_FIELDS = {"json", "markdown"}
SCOPE_FIELDS = {
    "invariant_basis",
    "test_oracle_basis",
    "coverage_basis",
    "bytecode_pilot_basis",
    "public_claim_basis",
    "debt_is_report_failure",
}
SUMMARY_FIELDS = {
    "invariants_total",
    "invariants_without_spec",
    "expected_result_tests",
    "tests_without_oracle",
    "coverage_cells",
    "spec_gap_cells",
    "bytecode_pilot_gaps",
    "registered_public_claims",
}
INVARIANT_FIELDS = {
    "invariant_id",
    "area",
    "risk",
    "invariant_status",
    "spec_status",
    "spec_source_refs",
    "spec_gap_refs",
}
TEST_FIELDS = {"test_id", "area", "test_class", "status", "missing_bindings"}
CELL_FIELDS = {
    "invariant_id",
    "area",
    "risk",
    "invariant_status",
    "cell_index",
    "dimension",
    "spec_gap_ref",
    "rationale",
}
PILOT_FIELDS = {"denominator", "summary", "gaps"}
DENOMINATOR_FIELDS = {
    "area",
    "basis",
    "open_resolution_statuses",
    "runnable_test_statuses",
    "ignored_catalog_tests_are_runnable",
    "hardware_tool_or_na_inference",
}
PILOT_SUMMARY_FIELDS = {"total", "by_classification"}
PILOT_GAP_FIELDS = {
    "gap_id",
    "classification",
    "source_kind",
    "area",
    "risk",
    "detail",
    "related_record_ids",
}
PUBLIC_CONTEXT_FIELDS = {"basis", "exhaustive", "claims"}
PUBLIC_CLAIM_FIELDS = {
    "source_id",
    "area",
    "source_status",
    "surface_ref",
    "linked_invariant_ids",
    "oracle_invariant_ids",
    "linked_spec_gap_ids",
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
                failures.append(
                    f"{field} does not match current specification-completeness analysis"
                )
    return sorted(set(failures))


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("spec-completeness schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("spec-completeness schema root fields drift")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if not isinstance(properties, Mapping) or properties.get(field, {}).get("const") != expected:
            failures.append(f"spec-completeness schema const for {field} drifts")
    if not isinstance(properties, Mapping) or properties.get("commit", {}).get("pattern") != "^[0-9a-f]{40}$":
        failures.append("spec-completeness schema clean-commit pattern drifts")
    definitions = schema.get("$defs", {})
    expected_definitions = {
        "output_paths": OUTPUT_FIELDS,
        "scope": SCOPE_FIELDS,
        "summary": SUMMARY_FIELDS,
        "invariant_debt": INVARIANT_FIELDS,
        "test_debt": TEST_FIELDS,
        "spec_gap_cell": CELL_FIELDS,
        "pilot": PILOT_FIELDS,
        "denominator": DENOMINATOR_FIELDS,
        "pilot_summary": PILOT_SUMMARY_FIELDS,
        "classification_counts": set(PILOT_CLASSIFICATIONS),
        "pilot_gap": PILOT_GAP_FIELDS,
        "public_context": PUBLIC_CONTEXT_FIELDS,
        "public_claim": PUBLIC_CLAIM_FIELDS,
    }
    for name, fields in expected_definitions.items():
        definition = definitions.get(name, {}) if isinstance(definitions, Mapping) else {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"spec-completeness schema {name} must be a closed object")
        if set(definition.get("required", [])) != fields:
            failures.append(f"spec-completeness schema {name} fields drift")
    pilot_gap = definitions.get("pilot_gap", {}) if isinstance(definitions, Mapping) else {}
    gap_properties = pilot_gap.get("properties", {}) if isinstance(pilot_gap, Mapping) else {}
    if set(gap_properties.get("classification", {}).get("enum", [])) != set(PILOT_CLASSIFICATIONS):
        failures.append("spec-completeness schema pilot classification enum drifts")
    denominator = definitions.get("denominator", {}) if isinstance(definitions, Mapping) else {}
    denominator_properties = denominator.get("properties", {}) if isinstance(denominator, Mapping) else {}
    for field, expected in (
        ("area", PILOT_AREA),
        ("basis", "open_spec_gaps_union_missing_required_runnable_test_classes"),
        ("ignored_catalog_tests_are_runnable", False),
        ("hardware_tool_or_na_inference", False),
    ):
        if denominator_properties.get(field, {}).get("const") != expected:
            failures.append(f"spec-completeness schema denominator.{field} const drifts")
    public = definitions.get("public_context", {}) if isinstance(definitions, Mapping) else {}
    public_properties = public.get("properties", {}) if isinstance(public, Mapping) else {}
    if public_properties.get("basis", {}).get("const") != "registered_spec_sources_only":
        failures.append("spec-completeness schema public-context basis drifts")
    if public_properties.get("exhaustive", {}).get("const") is not False:
        failures.append("spec-completeness schema public-context exhaustiveness drifts")
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    failures: list[str] = []
    canonical = (json.dumps(dict(payload), indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("spec-completeness JSON is not canonical")
    expected = _expected_markdown(payload, json_bytes)
    if expected is not None and markdown != expected:
        failures.append("spec-completeness Markdown does not exactly match JSON payload")
    return failures


def _validate_analysis_shape(payload: Mapping[str, Any], failures: list[str]) -> None:
    expected_scope = {
        "invariant_basis": "all_committed_invariant_records",
        "test_oracle_basis": "catalog_rows_with_expected_result",
        "coverage_basis": "all_committed_invariant_coverage_cells",
        "bytecode_pilot_basis": "open_spec_gaps_union_missing_required_runnable_test_classes",
        "public_claim_basis": "registered_spec_sources_non_exhaustive_context",
        "debt_is_report_failure": False,
    }
    scope = payload.get("scope")
    if not isinstance(scope, dict):
        failures.append("scope must be an object")
    else:
        _check_exact_fields(scope, SCOPE_FIELDS, "scope", failures)
        if scope != expected_scope:
            failures.append("scope does not match specification-completeness contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match specification-completeness contract")
    summary = payload.get("summary")
    if not isinstance(summary, dict):
        failures.append("summary must be an object")
    else:
        _check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
        for field in SUMMARY_FIELDS:
            _check_nonnegative(summary.get(field), f"summary.{field}", failures)

    _validate_rows(
        payload.get("invariants_without_spec"),
        INVARIANT_FIELDS,
        "invariants_without_spec",
        "invariant_id",
        failures,
    )
    _validate_rows(
        payload.get("tests_without_oracle"),
        TEST_FIELDS,
        "tests_without_oracle",
        "test_id",
        failures,
    )
    _validate_cell_rows(payload.get("spec_gap_cells"), failures)
    _validate_pilot(payload.get("bytecode_pilot"), failures)
    _validate_public_context(payload.get("public_claim_context"), failures)


def _validate_rows(
    value: Any,
    fields: set[str],
    label: str,
    id_field: str,
    failures: list[str],
) -> None:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return
    keys: list[Any] = []
    for index, row in enumerate(value):
        if not isinstance(row, dict):
            failures.append(f"{label}[{index}] must be an object")
            continue
        _check_exact_fields(row, fields, f"{label}[{index}]", failures)
        keys.append(row.get(id_field))
        for field in ("spec_source_refs", "spec_gap_refs"):
            if field in row:
                _check_string_set(row.get(field), f"{label}[{index}].{field}", failures)
    if keys != sorted(keys) or len(keys) != len(set(keys)):
        failures.append(f"{label} must be canonical and unique by {id_field}")
    if label == "tests_without_oracle":
        for index, row in enumerate(value):
            if isinstance(row, Mapping) and row.get("missing_bindings") != list(ORACLE_BINDING_FIELDS):
                failures.append(f"tests_without_oracle[{index}] missing_bindings drifts")


def _validate_cell_rows(value: Any, failures: list[str]) -> None:
    if not isinstance(value, list):
        failures.append("spec_gap_cells must be an array")
        return
    keys: list[tuple[Any, Any]] = []
    for index, row in enumerate(value):
        if not isinstance(row, dict):
            failures.append(f"spec_gap_cells[{index}] must be an object")
            continue
        _check_exact_fields(row, CELL_FIELDS, f"spec_gap_cells[{index}]", failures)
        _check_nonnegative(row.get("cell_index"), f"spec_gap_cells[{index}].cell_index", failures)
        if not isinstance(row.get("spec_gap_ref"), str) or not row["spec_gap_ref"]:
            failures.append(f"spec_gap_cells[{index}].spec_gap_ref must be non-empty")
        keys.append((row.get("invariant_id"), row.get("cell_index")))
    if keys != sorted(keys) or len(keys) != len(set(keys)):
        failures.append("spec_gap_cells must be canonical and unique by invariant/cell index")


def _validate_pilot(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("bytecode_pilot must be an object")
        return
    _check_exact_fields(value, PILOT_FIELDS, "bytecode_pilot", failures)
    denominator = value.get("denominator")
    expected_denominator = {
        "area": PILOT_AREA,
        "basis": "open_spec_gaps_union_missing_required_runnable_test_classes",
        "open_resolution_statuses": ["decision_recorded", "open", "spec_updated", "test_mapped"],
        "runnable_test_statuses": ["implemented", "mapped", "test_written", "validated"],
        "ignored_catalog_tests_are_runnable": False,
        "hardware_tool_or_na_inference": False,
    }
    if not isinstance(denominator, dict):
        failures.append("bytecode_pilot.denominator must be an object")
    else:
        _check_exact_fields(denominator, DENOMINATOR_FIELDS, "bytecode_pilot.denominator", failures)
        if denominator != expected_denominator:
            failures.append("bytecode_pilot denominator contract drifts")
    summary = value.get("summary")
    if not isinstance(summary, dict):
        failures.append("bytecode_pilot.summary must be an object")
    else:
        _check_exact_fields(summary, PILOT_SUMMARY_FIELDS, "bytecode_pilot.summary", failures)
        _check_nonnegative(summary.get("total"), "bytecode_pilot.summary.total", failures)
        counts = summary.get("by_classification")
        if not isinstance(counts, dict):
            failures.append("bytecode_pilot.summary.by_classification must be an object")
        else:
            _check_exact_fields(
                counts,
                set(PILOT_CLASSIFICATIONS),
                "bytecode_pilot.summary.by_classification",
                failures,
            )
            for classification in PILOT_CLASSIFICATIONS:
                _check_nonnegative(
                    counts.get(classification),
                    f"bytecode_pilot.summary.by_classification.{classification}",
                    failures,
                )
            if counts.get("hardware_tool_blocked") != 0 or counts.get("not_applicable") != 0:
                failures.append("bytecode pilot v1 cannot infer hardware/tool or not-applicable gaps")
    gaps = value.get("gaps")
    if not isinstance(gaps, list):
        failures.append("bytecode_pilot.gaps must be an array")
        return
    ids: list[Any] = []
    actual_counts: dict[str, int] = {name: 0 for name in PILOT_CLASSIFICATIONS}
    for index, row in enumerate(gaps):
        if not isinstance(row, dict):
            failures.append(f"bytecode_pilot.gaps[{index}] must be an object")
            continue
        _check_exact_fields(row, PILOT_GAP_FIELDS, f"bytecode_pilot.gaps[{index}]", failures)
        gap_id = row.get("gap_id")
        ids.append(gap_id)
        classification = row.get("classification")
        if classification not in PILOT_CLASSIFICATIONS:
            failures.append(f"bytecode_pilot.gaps[{index}] has unknown classification")
        else:
            actual_counts[classification] += 1
        if row.get("area") != PILOT_AREA:
            failures.append(f"bytecode_pilot.gaps[{index}] must use bytecode_vm area")
        _check_string_set(
            row.get("related_record_ids"),
            f"bytecode_pilot.gaps[{index}].related_record_ids",
            failures,
        )
        if classification == "spec_gap":
            if row.get("source_kind") != "spec_gap_record" or not str(gap_id).startswith("SPEC_GAP_"):
                failures.append(f"bytecode_pilot.gaps[{index}] spec-gap provenance drifts")
        elif classification == "test_gap":
            if row.get("source_kind") != "required_test_class_slot" or not str(gap_id).startswith("TEST_CLASS_GAP:bytecode_vm:"):
                failures.append(f"bytecode_pilot.gaps[{index}] test-gap provenance drifts")
    if ids != sorted(ids) or len(ids) != len(set(ids)):
        failures.append("bytecode_pilot gaps must be canonical and disjoint by gap_id")
    if isinstance(summary, Mapping):
        if summary.get("total") != len(gaps):
            failures.append("bytecode_pilot.summary.total does not match gap rows")
        if summary.get("by_classification") != actual_counts:
            failures.append("bytecode_pilot.summary.by_classification does not match gap rows")


def _validate_public_context(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("public_claim_context must be an object")
        return
    _check_exact_fields(value, PUBLIC_CONTEXT_FIELDS, "public_claim_context", failures)
    if value.get("basis") != "registered_spec_sources_only" or value.get("exhaustive") is not False:
        failures.append("public-claim context must remain explicitly non-exhaustive registry context")
    claims = value.get("claims")
    if not isinstance(claims, list):
        failures.append("public_claim_context.claims must be an array")
        return
    ids: list[Any] = []
    for index, row in enumerate(claims):
        if not isinstance(row, dict):
            failures.append(f"public_claim_context.claims[{index}] must be an object")
            continue
        _check_exact_fields(row, PUBLIC_CLAIM_FIELDS, f"public_claim_context.claims[{index}]", failures)
        ids.append(row.get("source_id"))
        for field in ("linked_invariant_ids", "oracle_invariant_ids", "linked_spec_gap_ids"):
            _check_string_set(
                row.get(field),
                f"public_claim_context.claims[{index}].{field}",
                failures,
            )
    if ids != sorted(ids) or len(ids) != len(set(ids)):
        failures.append("public_claim_context claims must be canonical and unique")


def _validate_internal_counts(payload: Mapping[str, Any], failures: list[str]) -> None:
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        return
    invariants = payload.get("invariants_without_spec")
    tests = payload.get("tests_without_oracle")
    cells = payload.get("spec_gap_cells")
    pilot = payload.get("bytecode_pilot")
    claims = payload.get("public_claim_context")
    expected = {
        "invariants_without_spec": len(invariants) if isinstance(invariants, list) else None,
        "tests_without_oracle": len(tests) if isinstance(tests, list) else None,
        "spec_gap_cells": len(cells) if isinstance(cells, list) else None,
        "bytecode_pilot_gaps": pilot.get("summary", {}).get("total") if isinstance(pilot, Mapping) else None,
        "registered_public_claims": len(claims.get("claims", [])) if isinstance(claims, Mapping) and isinstance(claims.get("claims"), list) else None,
    }
    for field, value in expected.items():
        if value is not None and summary.get(field) != value:
            failures.append(f"summary.{field} does not match report rows")
    for total_field, debt_field in (
        ("invariants_total", "invariants_without_spec"),
        ("expected_result_tests", "tests_without_oracle"),
        ("coverage_cells", "spec_gap_cells"),
    ):
        total = summary.get(total_field)
        debt = summary.get(debt_field)
        if isinstance(total, int) and isinstance(debt, int) and total < debt:
            failures.append(f"summary.{total_field} cannot be less than {debt_field}")


def _validate_output_paths(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("output_paths must be an object")
        return
    _check_exact_fields(value, OUTPUT_FIELDS, "output_paths", failures)
    json_path = value.get("json")
    markdown_path = value.get("markdown")
    if not isinstance(json_path, str) or not json_path.startswith("target/gate-artifacts/verification/"):
        failures.append("output_paths.json must be under target/gate-artifacts/verification")
    elif not _is_safe_relative_path(json_path):
        failures.append("output_paths.json must be workspace-relative")
    if not isinstance(markdown_path, str) or not (
        markdown_path.startswith("target/gate-artifacts/verification/")
        or DATED_EVIDENCE_RE.fullmatch(markdown_path)
    ):
        failures.append(
            "output_paths.markdown must be under target/gate-artifacts/verification or dated PLC evidence"
        )
    elif not _is_safe_relative_path(markdown_path):
        failures.append("output_paths.markdown must be workspace-relative")


def _validate_canonical_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, Mapping) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_spec_completeness.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical specification-completeness invocation")


def _expected_markdown(payload: Mapping[str, Any], json_bytes: bytes) -> str | None:
    try:
        report = SpecCompletenessReport(
            provenance=SpecCompletenessProvenance(
                command=tuple(payload["command"]),
                commit=payload["commit"],
                timestamp=payload["timestamp"],
                platform=payload["platform"],
                input_paths=tuple(payload["input_paths"]),
                output_json=payload["output_paths"]["json"],
                output_markdown=payload["output_paths"]["markdown"],
            ),
            input_digest=payload["input_digest"],
            analysis={field: payload[field] for field in ANALYSIS_FIELDS},
        )
    except (KeyError, TypeError):
        return None
    return report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())


def _check_exact_fields(
    value: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    missing = sorted(expected - set(value))
    extra = sorted(set(value) - expected)
    if missing:
        failures.append(f"{label} missing fields: {', '.join(missing)}")
    if extra:
        failures.append(f"{label} has unexpected fields: {', '.join(extra)}")


def _check_sorted_unique(values: list[str], label: str, failures: list[str]) -> None:
    if values != sorted(set(values)):
        failures.append(f"{label} must be sorted and duplicate-free")


def _check_string_set(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        failures.append(f"{label} must be a string array")
    elif value != sorted(set(value)):
        failures.append(f"{label} must be sorted and duplicate-free")


def _check_nonnegative(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        failures.append(f"{label} must be a non-negative integer")


def _is_iso_timestamp(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _is_safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts
