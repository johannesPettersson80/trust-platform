"""Fail-closed payload, schema, and Markdown contract for Phase 7."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from typing import Any

from .conformance_alignment import (
    CI_JOB_REVIEWED_DIGEST,
    COMMS_REVIEWED_SOURCE_PATHS,
    COMMS_REVIEWED_SOURCE_DIGEST,
    CONTRACT_COVERS,
    PUBLIC_PAGE_REVIEWED_DIGEST,
    RUNNER_REVIEWED_BEHAVIORS,
    RUNNER_REVIEWED_SOURCE_DIGEST,
    RUNNER_REVIEWED_SOURCE_PATHS,
    V1_CATEGORIES,
    V2_CATEGORIES,
)
from .conformance_alignment_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    SCOPE,
    render_markdown,
)
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
TOP_FIELDS = {
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
    "boundaries",
    "contract",
    "categories",
    "cases",
    "unlinked_case_ids",
    "coverage_gaps",
    "comms_determinism",
    "publication",
    "summary",
    "limitations",
}
OUTPUT_FIELDS = {"json", "markdown"}
CONTRACT_FIELDS = {
    "spec_source_id",
    "path",
    "area",
    "owner",
    "metadata_status",
    "covers",
    "digest",
    "tracked",
    "visibility",
    "authority",
    "oracle_eligible",
    "public_page_bound",
    "reviewed_runner_source_paths",
    "reviewed_runner_source_digest",
    "reviewed_runner_behaviors",
}
CATEGORY_FIELDS = {
    "category",
    "profile",
    "case_count",
    "expected_artifact_count",
    "linked_case_count",
    "unlinked_case_count",
    "case_ids",
}
CASE_FIELDS = {
    "discovery_id",
    "case_id",
    "category",
    "profile",
    "kind",
    "manifest_path",
    "manifest_digest",
    "program_path",
    "program_digest",
    "expected_artifact_path",
    "expected_artifact_digest",
    "catalog_test_id",
    "invariant_ids",
}
GAP_FIELDS = {
    "category",
    "case_ids",
    "case_present",
    "expected_artifact_present",
    "invariant_mapping_state",
    "semantic_oracle_state",
    "gap_status",
}
COMMS_FIELDS = {
    "case_id",
    "kind",
    "execution_mode",
    "scripted_steps",
    "program_source_present",
    "live_socket_dependency",
    "reviewed_call_path",
    "reviewed_source_paths",
    "reviewed_source_digest",
}
PUBLICATION_FIELDS = {
    "ci_job",
    "ci_job_digest",
    "ci_artifact_name",
    "generated_json_glob",
    "generated_markdown_glob",
    "generated_report_policy",
    "tracked_report_files",
    "public_page_embeds_generated_result",
    "public_page_digest",
}
SUMMARY_FIELDS = {
    "categories",
    "v1_categories",
    "v2_categories",
    "cases",
    "v1_cases",
    "v2_cases",
    "runtime_cases",
    "compile_error_cases",
    "connector_status_trace_cases",
    "program_sources",
    "expected_artifacts",
    "missing_expected_artifacts",
    "orphan_expected_artifacts",
    "explicitly_linked_cases",
    "unlinked_cases",
    "coverage_gaps",
}


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_analysis: Mapping[str, Any] | None = None,
) -> list[str]:
    failures: list[str] = []
    _fields(payload, TOP_FIELDS, "report", failures)
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected!r}")
    if not isinstance(payload.get("commit"), str) or not COMMIT_RE.fullmatch(
        str(payload.get("commit", ""))
    ):
        failures.append("commit must identify a clean full Git SHA")
    if not isinstance(payload.get("input_digest"), str) or not DIGEST_RE.fullmatch(
        str(payload.get("input_digest", ""))
    ):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")
    inputs = payload.get("input_paths")
    if not _strings(inputs) or inputs != sorted(set(inputs)):
        failures.append("input_paths must be a sorted unique non-empty string array")
    elif any(not is_safe_relative_path(path) for path in inputs):
        failures.append("input_paths must be normalized workspace-relative paths")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping):
        failures.append("output_paths must be an object")
    else:
        _fields(outputs, OUTPUT_FIELDS, "output_paths", failures)
        if any(
            not isinstance(outputs.get(field), str)
            or not is_safe_relative_path(str(outputs.get(field)))
            for field in OUTPUT_FIELDS
        ):
            failures.append("output paths must be normalized and workspace-relative")
    _validate_command(payload, failures)
    if payload.get("scope") != SCOPE:
        failures.append("scope does not match the conservative Phase 7 contract")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries do not match the report-only Phase 7 contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the conformance alignment contract")

    contract = payload.get("contract")
    if not isinstance(contract, Mapping):
        failures.append("contract must be an object")
        contract = {}
    else:
        _fields(contract, CONTRACT_FIELDS, "contract", failures)
    _validate_contract(contract, failures)
    categories = _rows(payload, "categories", CATEGORY_FIELDS, failures)
    cases = _rows(payload, "cases", CASE_FIELDS, failures)
    gaps = _rows(payload, "coverage_gaps", GAP_FIELDS, failures)
    _validate_cases(cases, failures)
    _validate_categories(categories, cases, failures)
    _validate_gaps(gaps, cases, failures)
    unlinked = payload.get("unlinked_case_ids")
    expected_unlinked = [row.get("case_id") for row in cases if not row.get("invariant_ids")]
    if unlinked != expected_unlinked:
        failures.append("unlinked_case_ids must exactly match cases without explicit invariant IDs")
    _validate_comms(payload.get("comms_determinism"), cases, failures)
    _validate_publication(payload.get("publication"), failures)
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
        if dict(summary) != _summary(categories, cases, gaps):
            failures.append("summary does not match conformance rows")

    if expected_analysis is not None:
        actual_analysis = {
            "contract": dict(contract),
            "categories": categories,
            "cases": cases,
            "unlinked_case_ids": unlinked,
            "coverage_gaps": gaps,
            "comms_determinism": payload.get("comms_determinism"),
            "publication": payload.get("publication"),
            "summary": dict(summary) if isinstance(summary, Mapping) else summary,
        }
        if actual_analysis != dict(expected_analysis):
            failures.append("report rows do not match current conformance alignment analysis")
    return sorted(set(failures))


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("conformance alignment schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS:
        failures.append("conformance alignment schema root required fields drift")
    properties = schema.get("properties", {})
    if not isinstance(properties, Mapping) or set(properties) != TOP_FIELDS:
        failures.append("conformance alignment schema root properties drift")
        properties = {}
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"conformance alignment schema const for {field} drifts")
    if properties.get("commit", {}).get("pattern") != "^[0-9a-f]{40}$":
        failures.append("conformance alignment schema clean-commit pattern drifts")
    definitions = schema.get("$defs", {})
    if not isinstance(definitions, Mapping):
        failures.append("conformance alignment schema definitions must be an object")
        definitions = {}
    expected_defs = {
        "output_paths": OUTPUT_FIELDS,
        "scope": set(SCOPE),
        "boundaries": set(BOUNDARIES),
        "contract": CONTRACT_FIELDS,
        "category": CATEGORY_FIELDS,
        "case": CASE_FIELDS,
        "coverage_gap": GAP_FIELDS,
        "comms_determinism": COMMS_FIELDS,
        "publication": PUBLICATION_FIELDS,
        "summary": SUMMARY_FIELDS,
    }
    for name, fields in expected_defs.items():
        definition = definitions.get(name, {})
        if not isinstance(definition, Mapping):
            failures.append(f"conformance alignment schema {name} must be an object")
            continue
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"conformance alignment schema {name} must be a closed object")
        if set(definition.get("required", [])) != fields:
            failures.append(f"conformance alignment schema {name} required fields drift")
        if set(definition.get("properties", {})) != fields:
            failures.append(f"conformance alignment schema {name} properties drift")
    _validate_schema_consts(definitions, failures)
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    failures: list[str] = []
    canonical = json.dumps(dict(payload), indent=2, sort_keys=True) + "\n"
    if json_bytes != canonical.encode():
        failures.append("conformance alignment JSON is not canonical")
    digest = hashlib.sha256(json_bytes).hexdigest()
    if markdown != render_markdown(payload, json_digest=digest):
        failures.append("conformance alignment Markdown does not match canonical JSON")
    return failures


def _validate_contract(contract: Mapping[str, Any], failures: list[str]) -> None:
    expected = {
        "spec_source_id": "SPEC_CONFORMANCE_CONTRACT_001",
        "path": "conformance/contract.md",
        "area": "release",
        "owner": "verification",
        "metadata_status": "mapped",
        "covers": list(CONTRACT_COVERS),
        "tracked": True,
        "visibility": "public",
        "authority": "normative_product",
        "oracle_eligible": False,
        "public_page_bound": True,
        "reviewed_runner_source_paths": list(RUNNER_REVIEWED_SOURCE_PATHS),
        "reviewed_runner_source_digest": RUNNER_REVIEWED_SOURCE_DIGEST,
        "reviewed_runner_behaviors": list(RUNNER_REVIEWED_BEHAVIORS),
    }
    for field, value in expected.items():
        if contract.get(field) != value:
            failures.append(f"contract.{field} must equal {value!r}")
    if not isinstance(contract.get("digest"), str) or not DIGEST_RE.fullmatch(
        str(contract.get("digest", ""))
    ):
        failures.append("contract.digest must be sha256:<64 lowercase hex>")


def _validate_cases(cases: list[dict[str, Any]], failures: list[str]) -> None:
    ids = [row.get("case_id") for row in cases]
    if ids != sorted(set(ids)):
        failures.append("cases must use unique canonical case_id order")
    allowed_categories = set((*V1_CATEGORIES, *V2_CATEGORIES))
    for row in cases:
        case_id = row.get("case_id")
        category = row.get("category")
        expected_profile = "v1" if category in V1_CATEGORIES else "v2"
        if category not in allowed_categories:
            failures.append(f"{case_id}: unknown conformance category")
        if row.get("profile") != expected_profile:
            failures.append(f"{case_id}: profile does not match category")
        if row.get("kind") not in {"runtime", "compile_error", "connector_status_trace"}:
            failures.append(f"{case_id}: unknown conformance case kind")
        for field in ("manifest_digest", "expected_artifact_digest"):
            if not isinstance(row.get(field), str) or not DIGEST_RE.fullmatch(str(row.get(field, ""))):
                failures.append(f"{case_id}: {field} must be sha256:<64 lowercase hex>")
        if (row.get("program_path") is None) != (row.get("program_digest") is None):
            failures.append(f"{case_id}: program path and digest must be paired")
        if row.get("program_digest") is not None and not DIGEST_RE.fullmatch(
            str(row.get("program_digest"))
        ):
            failures.append(f"{case_id}: program_digest must be sha256:<64 lowercase hex>")
        invariants = row.get("invariant_ids")
        if not isinstance(invariants, list) or invariants != sorted(set(invariants)):
            failures.append(f"{case_id}: invariant_ids must be a sorted unique array")
        if invariants and not isinstance(row.get("catalog_test_id"), str):
            failures.append(f"{case_id}: invariant links require an explicit catalog test")


def _validate_categories(
    categories: list[dict[str, Any]],
    cases: list[dict[str, Any]],
    failures: list[str],
) -> None:
    expected_order = [*V1_CATEGORIES, *V2_CATEGORIES]
    if [row.get("category") for row in categories] != expected_order:
        failures.append("categories must exactly match written v1/v2 contract order")
    for row in categories:
        category = row.get("category")
        members = [case for case in cases if case.get("category") == category]
        expected = {
            "profile": "v1" if category in V1_CATEGORIES else "v2",
            "case_count": len(members),
            "expected_artifact_count": len(members),
            "linked_case_count": sum(bool(case.get("invariant_ids")) for case in members),
            "unlinked_case_count": sum(not case.get("invariant_ids") for case in members),
            "case_ids": [case.get("case_id") for case in members],
        }
        for field, value in expected.items():
            if row.get(field) != value:
                failures.append(f"category {category}: {field} does not match case rows")


def _validate_gaps(
    gaps: list[dict[str, Any]],
    cases: list[dict[str, Any]],
    failures: list[str],
) -> None:
    if [row.get("category") for row in gaps] != list(V2_CATEGORIES):
        failures.append("coverage_gaps must contain exactly the ten v2 categories")
    for row in gaps:
        category = row.get("category")
        members = [case for case in cases if case.get("category") == category]
        expected = {
            "case_ids": [case.get("case_id") for case in members],
            "case_present": bool(members),
            "expected_artifact_present": bool(members),
            "invariant_mapping_state": (
                "linked" if members and all(case.get("invariant_ids") for case in members) else "missing"
            ),
            "semantic_oracle_state": "not_assessed",
            "gap_status": "open",
        }
        for field, value in expected.items():
            if row.get(field) != value:
                failures.append(f"coverage gap {category}: {field} does not match case rows")


def _validate_comms(value: Any, cases: list[dict[str, Any]], failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        failures.append("comms_determinism must be an object")
        return
    _fields(value, COMMS_FIELDS, "comms_determinism", failures)
    members = [case for case in cases if case.get("category") == "comms_determinism"]
    expected_case = members[0].get("case_id") if len(members) == 1 else None
    expected = {
        "case_id": expected_case,
        "kind": "connector_status_trace",
        "execution_mode": "scripted_in_process",
        "scripted_steps": 8,
        "program_source_present": False,
        "live_socket_dependency": False,
        "reviewed_call_path": [
            "execute_case",
            "execute_connector_status_trace_case",
            "project_connector_status_step",
        ],
        "reviewed_source_paths": list(COMMS_REVIEWED_SOURCE_PATHS),
        "reviewed_source_digest": COMMS_REVIEWED_SOURCE_DIGEST,
    }
    for field, expected_value in expected.items():
        if value.get(field) != expected_value:
            failures.append(f"comms_determinism.{field} must equal {expected_value!r}")
def _validate_publication(value: Any, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        failures.append("publication must be an object")
        return
    _fields(value, PUBLICATION_FIELDS, "publication", failures)
    expected = {
        "ci_job": ".github/workflows/ci.yml#conformance",
        "ci_job_digest": CI_JOB_REVIEWED_DIGEST,
        "ci_artifact_name": "conformance-suite",
        "generated_json_glob": "gate-artifacts/conformance-pass-*.json",
        "generated_markdown_glob": "gate-artifacts/conformance-pass-*.md",
        "generated_report_policy": "ci_artifact_only",
        "tracked_report_files": ["conformance/reports/.gitkeep"],
        "public_page_embeds_generated_result": False,
        "public_page_digest": PUBLIC_PAGE_REVIEWED_DIGEST,
    }
    for field, expected_value in expected.items():
        if value.get(field) != expected_value:
            failures.append(f"publication.{field} must equal {expected_value!r}")


def _summary(
    categories: list[dict[str, Any]],
    cases: list[dict[str, Any]],
    gaps: list[dict[str, Any]],
) -> dict[str, int]:
    return {
        "categories": len(categories),
        "v1_categories": sum(row.get("profile") == "v1" for row in categories),
        "v2_categories": sum(row.get("profile") == "v2" for row in categories),
        "cases": len(cases),
        "v1_cases": sum(row.get("profile") == "v1" for row in cases),
        "v2_cases": sum(row.get("profile") == "v2" for row in cases),
        "runtime_cases": sum(row.get("kind") == "runtime" for row in cases),
        "compile_error_cases": sum(row.get("kind") == "compile_error" for row in cases),
        "connector_status_trace_cases": sum(
            row.get("kind") == "connector_status_trace" for row in cases
        ),
        "program_sources": sum(row.get("program_path") is not None for row in cases),
        "expected_artifacts": len(cases),
        "missing_expected_artifacts": 0,
        "orphan_expected_artifacts": 0,
        "explicitly_linked_cases": sum(bool(row.get("invariant_ids")) for row in cases),
        "unlinked_cases": sum(not row.get("invariant_ids") for row in cases),
        "coverage_gaps": len(gaps),
    }


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping):
        return
    expected = [
        "python3",
        "scripts/report_conformance_alignment.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        payload.get("timestamp"),
    ]
    if payload.get("command") != expected:
        failures.append("command does not match the canonical conformance alignment invocation")


def _validate_schema_consts(definitions: Mapping[str, Any], failures: list[str]) -> None:
    for definition_name, expected in (("scope", SCOPE), ("boundaries", BOUNDARIES)):
        properties = definitions.get(definition_name, {}).get("properties", {})
        for field, value in expected.items():
            if properties.get(field, {}).get("const") != value:
                failures.append(f"conformance alignment schema {definition_name}.{field} const drifts")
    properties = definitions.get("case", {}).get("properties", {})
    if properties.get("category", {}).get("enum") != [*V1_CATEGORIES, *V2_CATEGORIES]:
        failures.append("conformance alignment schema case category enum drifts")
    if properties.get("profile", {}).get("enum") != ["v1", "v2"]:
        failures.append("conformance alignment schema profile enum drifts")
    if properties.get("kind", {}).get("enum") != [
        "runtime",
        "compile_error",
        "connector_status_trace",
    ]:
        failures.append("conformance alignment schema case kind enum drifts")
    category = definitions.get("category", {}).get("properties", {})
    if category.get("category", {}).get("enum") != [*V1_CATEGORIES, *V2_CATEGORIES]:
        failures.append("conformance alignment schema category-row category enum drifts")
    if category.get("profile", {}).get("enum") != ["v1", "v2"]:
        failures.append("conformance alignment schema category-row profile enum drifts")
    gap = definitions.get("coverage_gap", {}).get("properties", {})
    if gap.get("category", {}).get("enum") != list(V2_CATEGORIES):
        failures.append("conformance alignment schema coverage-gap category enum drifts")
    for field, value in (
        ("case_present", True),
        ("expected_artifact_present", True),
        ("semantic_oracle_state", "not_assessed"),
        ("gap_status", "open"),
    ):
        if gap.get(field, {}).get("const") != value:
            failures.append(f"conformance alignment schema coverage_gap.{field} const drifts")
    if gap.get("invariant_mapping_state", {}).get("enum") != ["missing", "linked"]:
        failures.append("conformance alignment schema invariant-mapping enum drifts")
    exact_definitions = {
        "contract": {
            "spec_source_id": "SPEC_CONFORMANCE_CONTRACT_001",
            "path": "conformance/contract.md",
            "area": "release",
            "owner": "verification",
            "metadata_status": "mapped",
            "covers": list(CONTRACT_COVERS),
            "tracked": True,
            "visibility": "public",
            "authority": "normative_product",
            "oracle_eligible": False,
            "public_page_bound": True,
            "reviewed_runner_source_paths": list(RUNNER_REVIEWED_SOURCE_PATHS),
            "reviewed_runner_source_digest": RUNNER_REVIEWED_SOURCE_DIGEST,
            "reviewed_runner_behaviors": list(RUNNER_REVIEWED_BEHAVIORS),
        },
        "comms_determinism": {
            "case_id": "cfm_comms_determinism_connector_projection_001",
            "kind": "connector_status_trace",
            "execution_mode": "scripted_in_process",
            "scripted_steps": 8,
            "program_source_present": False,
            "live_socket_dependency": False,
            "reviewed_call_path": [
                "execute_case",
                "execute_connector_status_trace_case",
                "project_connector_status_step",
            ],
            "reviewed_source_paths": list(COMMS_REVIEWED_SOURCE_PATHS),
            "reviewed_source_digest": COMMS_REVIEWED_SOURCE_DIGEST,
        },
        "publication": {
            "ci_job": ".github/workflows/ci.yml#conformance",
            "ci_job_digest": CI_JOB_REVIEWED_DIGEST,
            "ci_artifact_name": "conformance-suite",
            "generated_json_glob": "gate-artifacts/conformance-pass-*.json",
            "generated_markdown_glob": "gate-artifacts/conformance-pass-*.md",
            "generated_report_policy": "ci_artifact_only",
            "tracked_report_files": ["conformance/reports/.gitkeep"],
            "public_page_embeds_generated_result": False,
            "public_page_digest": PUBLIC_PAGE_REVIEWED_DIGEST,
        },
    }
    for definition_name, expected in exact_definitions.items():
        properties = definitions.get(definition_name, {}).get("properties", {})
        for field, value in expected.items():
            if properties.get(field, {}).get("const") != value:
                failures.append(
                    f"conformance alignment schema {definition_name}.{field} const drifts"
                )


def _rows(
    payload: Mapping[str, Any],
    field: str,
    expected_fields: set[str],
    failures: list[str],
) -> list[dict[str, Any]]:
    value = payload.get(field)
    if not isinstance(value, list):
        failures.append(f"{field} must be an array")
        return []
    rows: list[dict[str, Any]] = []
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"{field}[{index}] must be an object")
            continue
        _fields(row, expected_fields, f"{field}[{index}]", failures)
        rows.append(dict(row))
    return rows


def _fields(
    value: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    actual = set(value)
    if actual != expected:
        failures.append(
            f"{label} fields must equal {sorted(expected)}, got {sorted(actual)}"
        )


def _strings(value: Any) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) for item in value)


def _timestamp(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None
