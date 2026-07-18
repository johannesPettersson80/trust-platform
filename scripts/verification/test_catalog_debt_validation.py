"""At-rest validation for unmapped-test debt reports."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import tomllib
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest
from .test_catalog_debt import (
    GENERATOR,
    GENERATOR_VERSION,
    INFERENCE_PROHIBITED,
    LIMITATIONS,
    REPORT_CONTRACT_PATHS,
    UnmappedDebtProvenance,
    UnmappedTestDebtReport,
    analyze_unmapped_test_debt,
)
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_denominator import (
    DENOMINATOR_PATH,
    DENOMINATOR_SCHEMA_PATH,
    NONMAPPING_REASON_CODES,
    analyze_test_catalog_denominator,
    load_test_catalog_denominator,
    validate_test_catalog_denominator_document,
)
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_validation import (
    IGNORE_STATES,
    SOURCE_KINDS,
    check_supported_schema_keywords,
    is_safe_relative_path,
    validate_report_payload as validate_generated_catalog_payload,
)


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
    "denominator_review",
    "unmapped_tests",
    "limitations",
}
ANALYSIS_FIELDS = {
    "scope",
    "summary",
    "denominator_review",
    "unmapped_tests",
    "limitations",
}
SCOPE_FIELDS = {
    "classification_basis",
    "artifact_rows_classify_facts",
    "debt_is_report_failure",
    "inference_prohibited",
}
SUMMARY_FIELDS = {
    "scanner_facts",
    "mapped_scanner_facts",
    "unmapped_scanner_facts",
    "generated_test_catalog_rows",
    "artifact_catalog_rows",
    "ignored_unmapped_scanner_facts",
    "conditional_unmapped_scanner_facts",
    "by_source_kind",
}
SOURCE_COUNT_FIELDS = {"source_kind", "scanner_facts", "mapped", "unmapped"}
UNMAPPED_FIELDS = {"discovery_id", "source_kind", "path", "name", "ignore_state"}
DENOMINATOR_FIELDS = {"review_digest", "summary"}
DENOMINATOR_SUMMARY_FIELDS = {
    "scanner_facts",
    "catalog_mapped_facts",
    "reviewed_nonmapping_facts",
    "unreviewed_facts",
    "exhaustive",
    "ignored_registry_owned_facts",
    "by_nonmapping_reason",
    "by_source_kind",
}
DENOMINATOR_SOURCE_FIELDS = {
    "source_kind",
    "scanner_facts",
    "catalog_mapped",
    "reviewed_nonmapping",
}
COMMIT_PATTERN = r"^[0-9a-f]{40}$"
COMMIT_RE = re.compile(COMMIT_PATTERN)
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
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
    if not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must be a clean full Git SHA")
    timestamp = payload.get("timestamp")
    if not isinstance(timestamp, str) or not timestamp:
        failures.append("timestamp must be a non-empty string")
    elif not _is_iso_timestamp(timestamp):
        failures.append("timestamp must be an ISO-8601 value with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload["platform"]:
        failures.append("platform must be a non-empty string")

    command = payload.get("command")
    if not isinstance(command, list) or not command or not all(
        isinstance(item, str) and item for item in command
    ):
        failures.append("command must be a non-empty string array")
    input_paths = payload.get("input_paths")
    if not isinstance(input_paths, list) or not all(isinstance(item, str) for item in input_paths):
        failures.append("input_paths must be a string array")
    else:
        _check_sorted_unique(input_paths, "input_paths", failures)
        for path in input_paths:
            if not is_safe_relative_path(path):
                failures.append(f"input path must be normalized and workspace-relative: {path!r}")
    _validate_output_paths(payload.get("output_paths"), failures)
    _validate_canonical_command(payload, failures)
    _validate_analysis_shape(payload, failures)
    _validate_internal_counts(payload, failures)

    if expected_analysis is not None:
        for field in sorted(ANALYSIS_FIELDS):
            if payload.get(field) != expected_analysis.get(field):
                failures.append(f"{field} does not match current debt analysis")
    return failures


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    summary = payload.get("summary", {})
    digest = hashlib.sha256(json_bytes).hexdigest()
    markers = [
        f"Generated JSON SHA-256: `{digest}`",
        f"Input SHA-256: `{payload.get('input_digest')}`",
        f"Source revision: `{payload.get('commit')}`",
        f"- Scanner facts: {summary.get('scanner_facts')}",
        f"- Unmapped scanner facts: {summary.get('unmapped_scanner_facts')}",
        f"- Unreviewed scanner facts: {payload.get('denominator_review', {}).get('summary', {}).get('unreviewed_facts')}",
        "- Unreviewed debt fails this report: yes",
    ]
    failures = [
        f"unmapped-test debt Markdown is missing bound marker: {marker}"
        for marker in markers
        if marker not in markdown
    ]
    canonical_json = (json.dumps(dict(payload), indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical_json:
        failures.append("unmapped-test debt JSON is not canonical")
    expected_markdown = _expected_markdown(payload, json_bytes)
    if expected_markdown is not None and markdown != expected_markdown:
        failures.append("unmapped-test debt Markdown does not exactly match JSON")
    return failures


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    """Recompute the live subtraction and bind source, schema, JSON, and Markdown."""

    root = root.resolve()
    failures: list[str] = []
    validator: Validator | None = None
    if root != METADATA_ROOT.resolve():
        failures.append("root does not identify the repository that loaded the verification modules")
    else:
        validator = Validator()
        validator.load_records()
        validator.validate()
        failures.extend(
            f"metadata: {_display_path(root, failure.path)}: {failure.message}"
            for failure in validator.failures
        )

    schema_file = _absolute(root, schema_path)
    try:
        schema = json.loads(schema_file.read_text())
    except Exception as exc:
        schema = None
        failures.append(f"unmapped-test debt schema cannot be read: {exc}")
    if isinstance(schema, dict):
        failures.extend(validate_schema_contract(schema))

    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
    except Exception as exc:
        return sorted(set([*failures, f"unmapped-test debt JSON cannot be read: {exc}"]))
    failures.extend(validate_report_payload(payload))
    if isinstance(schema, dict):
        failures.extend(validate_json_schema_instance(payload, schema))
    if not isinstance(payload, dict):
        return sorted(set(failures))

    expected_json_path = _relative(root, json_file, "JSON output", failures)
    expected_markdown_path = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths")
    if isinstance(outputs, dict):
        if expected_json_path is not None and outputs.get("json") != expected_json_path:
            failures.append("output_paths.json does not identify the validated JSON file")
        if expected_markdown_path is not None and outputs.get("markdown") != expected_markdown_path:
            failures.append("output_paths.markdown does not identify the validated Markdown file")

    scan = scan_repository(root)
    scan_payload = scan.to_dict()
    failures.extend(
        f"generated catalog: {failure}"
        for failure in validate_generated_catalog_payload(scan_payload)
    )
    if scan_payload.get("scan_status") != "complete":
        failures.append("generated catalog scan_status is not complete")

    tests, tests_by_id, load_failures = _load_catalog(root)
    failures.extend(load_failures)
    if tests is not None and tests_by_id is not None and validator is not None:
        failures.extend(
            validate_catalog_staleness(
                root=root,
                tests=tests_by_id,
                facts=scan.inferred_facts,
            )
        )
        try:
            denominator = load_test_catalog_denominator(root)
            denominator_schema = json.loads((root / DENOMINATOR_SCHEMA_PATH).read_text())
            failures.extend(
                validate_test_catalog_denominator_document(
                    denominator, schema=denominator_schema
                )
            )
            denominator_review = analyze_test_catalog_denominator(
                facts=scan.inferred_facts,
                tests=tests,
                reviews=denominator.get("reviews", []),
                ignored_tests=validator.ignored_tests,
            )
            expected_analysis = analyze_unmapped_test_debt(
                tests=tests,
                facts=scan.inferred_facts,
                denominator_review=denominator_review,
            )
        except ValueError as exc:
            failures.append(f"unmapped-test debt analysis failed: {exc}")
        except (OSError, json.JSONDecodeError) as exc:
            failures.append(f"test-catalog denominator cannot be read: {exc}")
        else:
            failures.extend(validate_report_payload(payload, expected_analysis=expected_analysis))

    expected_inputs = sorted(
        set(scan.provenance.input_paths)
        | set(REPORT_CONTRACT_PATHS)
        | validator_code_input_paths(root)
        | {"verification/test-catalog.toml", DENOMINATOR_PATH, DENOMINATOR_SCHEMA_PATH}
    )
    failures.extend(_validate_input_binding(root, payload, expected_inputs))
    failures.extend(_validate_source_commit(root, payload.get("commit"), expected_inputs))

    try:
        markdown = markdown_file.read_text()
    except Exception as exc:
        return sorted(
            set([*failures, f"unmapped-test debt Markdown cannot be read: {exc}"])
        )
    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    return sorted(set(failures))


def validate_schema_contract(schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("unmapped-test debt schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("unmapped-test debt schema root required fields drift from validation")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"unmapped-test debt schema const for {field} drifts from contract")
    if properties.get("commit", {}).get("pattern") != COMMIT_PATTERN:
        failures.append("unmapped-test debt schema commit pattern must require a clean full SHA")

    definitions = schema.get("$defs", {})
    for name, expected_fields in (
        ("scope", SCOPE_FIELDS),
        ("summary", SUMMARY_FIELDS),
        ("source_count", SOURCE_COUNT_FIELDS),
        ("denominator_review", DENOMINATOR_FIELDS),
        ("denominator_summary", DENOMINATOR_SUMMARY_FIELDS),
        ("denominator_source_count", DENOMINATOR_SOURCE_FIELDS),
        ("unmapped_test", UNMAPPED_FIELDS),
    ):
        definition = definitions.get(name, {}) if isinstance(definitions, dict) else {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"unmapped-test debt schema {name} must be a closed object")
        if set(definition.get("required", [])) != expected_fields:
            failures.append(f"unmapped-test debt schema {name} fields drift from validation")

    scope = definitions.get("scope", {}) if isinstance(definitions, dict) else {}
    scope_properties = scope.get("properties", {}) if isinstance(scope, dict) else {}
    for field, expected in (
        ("classification_basis", "exact_catalog_subtraction_plus_reviewed_denominator"),
        ("artifact_rows_classify_facts", False),
        ("debt_is_report_failure", True),
    ):
        if scope_properties.get(field, {}).get("const") != expected:
            failures.append(f"unmapped-test debt schema const for scope.{field} drifts")
    inference_enum = (
        scope_properties.get("inference_prohibited", {}).get("items", {}).get("enum", [])
    )
    if set(inference_enum) != set(INFERENCE_PROHIBITED):
        failures.append("unmapped-test debt schema inference_prohibited enum drifts")

    unmapped = definitions.get("unmapped_test", {}) if isinstance(definitions, dict) else {}
    unmapped_properties = unmapped.get("properties", {}) if isinstance(unmapped, dict) else {}
    if set(unmapped_properties.get("source_kind", {}).get("enum", [])) != SOURCE_KINDS:
        failures.append("unmapped-test debt schema source_kind enum drifts")
    if set(unmapped_properties.get("ignore_state", {}).get("enum", [])) != IGNORE_STATES:
        failures.append("unmapped-test debt schema ignore_state enum drifts")
    source_count = definitions.get("source_count", {}) if isinstance(definitions, dict) else {}
    source_count_properties = (
        source_count.get("properties", {}) if isinstance(source_count, dict) else {}
    )
    if set(source_count_properties.get("source_kind", {}).get("enum", [])) != SOURCE_KINDS:
        failures.append("unmapped-test debt schema source-count source_kind enum drifts")
    denominator_summary = (
        definitions.get("denominator_summary", {}) if isinstance(definitions, dict) else {}
    )
    denominator_properties = (
        denominator_summary.get("properties", {})
        if isinstance(denominator_summary, dict)
        else {}
    )
    reason_properties = denominator_properties.get("by_nonmapping_reason", {}).get(
        "properties", {}
    )
    if set(reason_properties) != set(NONMAPPING_REASON_CODES):
        failures.append("unmapped-test debt schema nonmapping reason fields drift")
    if denominator_properties.get("unreviewed_facts", {}).get("const") != 0:
        failures.append("unmapped-test debt schema must pin zero unreviewed facts")
    if denominator_properties.get("exhaustive", {}).get("const") is not True:
        failures.append("unmapped-test debt schema must pin exhaustive denominator review")
    return failures


def _validate_analysis_shape(payload: Mapping[str, Any], failures: list[str]) -> None:
    scope = payload.get("scope")
    expected_scope = {
        "classification_basis": "exact_catalog_subtraction_plus_reviewed_denominator",
        "artifact_rows_classify_facts": False,
        "debt_is_report_failure": True,
        "inference_prohibited": list(INFERENCE_PROHIBITED),
    }
    if not isinstance(scope, dict):
        failures.append("scope must be an object")
    else:
        _check_exact_fields(scope, SCOPE_FIELDS, "scope", failures)
        if scope != expected_scope:
            failures.append("scope does not match the unmapped-test debt contract")

    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the unmapped-test debt contract")

    summary = payload.get("summary")
    if not isinstance(summary, dict):
        failures.append("summary must be an object")
    else:
        _check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
        for field in SUMMARY_FIELDS - {"by_source_kind"}:
            value = summary.get(field)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                failures.append(f"summary.{field} must be a nonnegative integer")
        rows = summary.get("by_source_kind")
        if not isinstance(rows, list):
            failures.append("summary.by_source_kind must be an array")
        else:
            for index, row in enumerate(rows):
                if not isinstance(row, dict):
                    failures.append(f"summary.by_source_kind[{index}] must be an object")
                    continue
                _check_exact_fields(
                    row,
                    SOURCE_COUNT_FIELDS,
                    f"summary.by_source_kind[{index}]",
                    failures,
                )
                if row.get("source_kind") not in SOURCE_KINDS:
                    failures.append(
                        f"summary.by_source_kind[{index}].source_kind is unsupported"
                    )
                for field in SOURCE_COUNT_FIELDS - {"source_kind"}:
                    value = row.get(field)
                    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                        failures.append(
                            f"summary.by_source_kind[{index}].{field} must be nonnegative"
                        )
            keys = [row.get("source_kind") for row in rows if isinstance(row, dict)]
            if keys != sorted(set(keys)):
                failures.append("summary.by_source_kind must use canonical unique source order")

    denominator = payload.get("denominator_review")
    if not isinstance(denominator, dict):
        failures.append("denominator_review must be an object")
    else:
        _check_exact_fields(denominator, DENOMINATOR_FIELDS, "denominator_review", failures)
        if not DIGEST_RE.fullmatch(str(denominator.get("review_digest", ""))):
            failures.append("denominator_review.review_digest must be sha256:<64 lowercase hex>")
        denominator_summary = denominator.get("summary")
        if not isinstance(denominator_summary, dict):
            failures.append("denominator_review.summary must be an object")
        else:
            _validate_denominator_summary(denominator_summary, failures)

    unmapped_tests = payload.get("unmapped_tests")
    if not isinstance(unmapped_tests, list):
        failures.append("unmapped_tests must be an array")
        return
    for index, row in enumerate(unmapped_tests):
        if not isinstance(row, dict):
            failures.append(f"unmapped_tests[{index}] must be an object")
            continue
        _check_exact_fields(row, UNMAPPED_FIELDS, f"unmapped_tests[{index}]", failures)
        if not DISCOVERY_ID_RE.fullmatch(str(row.get("discovery_id", ""))):
            failures.append(f"unmapped_tests[{index}].discovery_id is invalid")
        if row.get("source_kind") not in SOURCE_KINDS:
            failures.append(f"unmapped_tests[{index}].source_kind is unsupported")
        if not isinstance(row.get("path"), str) or not is_safe_relative_path(row["path"]):
            failures.append(f"unmapped_tests[{index}].path must be workspace-relative")
        if not isinstance(row.get("name"), str) or not row["name"]:
            failures.append(f"unmapped_tests[{index}].name must be non-empty")
        if row.get("ignore_state") not in IGNORE_STATES:
            failures.append(f"unmapped_tests[{index}].ignore_state is unsupported")
    canonical = sorted(
        (
            row
            for row in unmapped_tests
            if isinstance(row, dict)
        ),
        key=_unmapped_sort_key,
    )
    if unmapped_tests != canonical:
        failures.append("unmapped_tests must use canonical source/path/name/discovery order")
    discovery_ids = [
        row.get("discovery_id") for row in unmapped_tests if isinstance(row, dict)
    ]
    if len(discovery_ids) != len(set(discovery_ids)):
        failures.append("unmapped_tests discovery IDs must be unique")


def _validate_internal_counts(payload: Mapping[str, Any], failures: list[str]) -> None:
    summary = payload.get("summary")
    rows = payload.get("unmapped_tests")
    if not isinstance(summary, dict) or not isinstance(rows, list):
        return
    if summary.get("unmapped_scanner_facts") != len(rows):
        failures.append("summary.unmapped_scanner_facts does not match unmapped_tests")
    scanner = summary.get("scanner_facts")
    mapped = summary.get("mapped_scanner_facts")
    unmapped = summary.get("unmapped_scanner_facts")
    if all(isinstance(value, int) and not isinstance(value, bool) for value in (scanner, mapped, unmapped)):
        if scanner != mapped + unmapped:
            failures.append("scanner fact totals are inconsistent")
    if summary.get("generated_test_catalog_rows") != mapped:
        failures.append("generated-test row count must equal mapped scanner facts")
    denominator = payload.get("denominator_review")
    denominator_summary = denominator.get("summary") if isinstance(denominator, dict) else None
    if isinstance(denominator_summary, dict):
        reviewed = denominator_summary.get("reviewed_nonmapping_facts")
        unreviewed = denominator_summary.get("unreviewed_facts")
        if all(
            isinstance(value, int) and not isinstance(value, bool)
            for value in (scanner, mapped, reviewed, unreviewed)
        ) and scanner != mapped + reviewed + unreviewed:
            failures.append("denominator scanner fact totals are inconsistent")
        if reviewed != unmapped:
            failures.append("reviewed nonmapping count must equal raw unmapped scanner facts")
        if denominator_summary.get("scanner_facts") != scanner:
            failures.append("denominator scanner count does not match report summary")
        if denominator_summary.get("catalog_mapped_facts") != mapped:
            failures.append("denominator mapped count does not match report summary")
    ignored = sum(
        isinstance(row, dict) and row.get("ignore_state") == "ignored" for row in rows
    )
    conditional = sum(
        isinstance(row, dict) and row.get("ignore_state") == "conditional" for row in rows
    )
    if summary.get("ignored_unmapped_scanner_facts") != ignored:
        failures.append("ignored unmapped count does not match unmapped_tests")
    if summary.get("conditional_unmapped_scanner_facts") != conditional:
        failures.append("conditional unmapped count does not match unmapped_tests")
    source_rows = summary.get("by_source_kind")
    if not isinstance(source_rows, list):
        return
    totals = {"scanner_facts": 0, "mapped": 0, "unmapped": 0}
    for row in source_rows:
        if not isinstance(row, dict):
            continue
        for field in totals:
            value = row.get(field)
            if isinstance(value, int) and not isinstance(value, bool):
                totals[field] += value
        if all(
            isinstance(row.get(field), int) and not isinstance(row.get(field), bool)
            for field in ("scanner_facts", "mapped", "unmapped")
        ) and row["scanner_facts"] != row["mapped"] + row["unmapped"]:
            failures.append(f"source counts are inconsistent for {row.get('source_kind')}")
    for field, summary_field in (
        ("scanner_facts", "scanner_facts"),
        ("mapped", "mapped_scanner_facts"),
        ("unmapped", "unmapped_scanner_facts"),
    ):
        if totals[field] != summary.get(summary_field):
            failures.append(f"source totals do not match summary.{summary_field}")


def _validate_denominator_summary(value: Mapping[str, Any], failures: list[str]) -> None:
    _check_exact_fields(value, DENOMINATOR_SUMMARY_FIELDS, "denominator_review.summary", failures)
    for field in {
        "scanner_facts",
        "catalog_mapped_facts",
        "reviewed_nonmapping_facts",
        "unreviewed_facts",
        "ignored_registry_owned_facts",
    }:
        count = value.get(field)
        if not isinstance(count, int) or isinstance(count, bool) or count < 0:
            failures.append(f"denominator_review.summary.{field} must be nonnegative")
    if value.get("unreviewed_facts") != 0:
        failures.append("denominator_review.summary.unreviewed_facts must be zero")
    if value.get("exhaustive") is not True:
        failures.append("denominator_review.summary.exhaustive must be true")
    reasons = value.get("by_nonmapping_reason")
    if not isinstance(reasons, dict) or set(reasons) != set(NONMAPPING_REASON_CODES):
        failures.append("denominator nonmapping reason counts drift from closed vocabulary")
    elif any(
        not isinstance(count, int) or isinstance(count, bool) or count < 0
        for count in reasons.values()
    ):
        failures.append("denominator nonmapping reason counts must be nonnegative")
    elif sum(reasons.values()) != value.get("reviewed_nonmapping_facts"):
        failures.append("denominator nonmapping reason counts do not reconcile")
    source_rows = value.get("by_source_kind")
    if not isinstance(source_rows, list):
        failures.append("denominator_review.summary.by_source_kind must be an array")
        return
    keys: list[str] = []
    totals = {"scanner_facts": 0, "catalog_mapped": 0, "reviewed_nonmapping": 0}
    expected_fields = {"source_kind", *totals}
    for index, row in enumerate(source_rows):
        if not isinstance(row, dict):
            failures.append(f"denominator source row {index} must be an object")
            continue
        _check_exact_fields(row, expected_fields, f"denominator source row {index}", failures)
        source_kind = row.get("source_kind")
        keys.append(str(source_kind))
        if source_kind not in SOURCE_KINDS:
            failures.append(f"denominator source row {index} source_kind is unsupported")
        for field in totals:
            count = row.get(field)
            if not isinstance(count, int) or isinstance(count, bool) or count < 0:
                failures.append(f"denominator source row {index}.{field} must be nonnegative")
            else:
                totals[field] += count
        if all(isinstance(row.get(field), int) for field in totals) and row.get(
            "scanner_facts"
        ) != row.get("catalog_mapped") + row.get("reviewed_nonmapping"):
            failures.append(f"denominator source row {index} counts are inconsistent")
    if keys != sorted(set(keys)):
        failures.append("denominator source rows must use canonical unique source order")
    for field, summary_field in (
        ("scanner_facts", "scanner_facts"),
        ("catalog_mapped", "catalog_mapped_facts"),
        ("reviewed_nonmapping", "reviewed_nonmapping_facts"),
    ):
        if totals[field] != value.get(summary_field):
            failures.append(f"denominator source totals do not match {summary_field}")


def _validate_output_paths(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("output_paths must be an object")
        return
    _check_exact_fields(value, {"json", "markdown"}, "output_paths", failures)
    json_path = value.get("json")
    markdown_path = value.get("markdown")
    if not isinstance(json_path, str) or not json_path.startswith(
        "target/gate-artifacts/verification/"
    ):
        failures.append("output_paths.json must be under target/gate-artifacts/verification")
    elif not is_safe_relative_path(json_path):
        failures.append("output_paths.json must be workspace-relative")
    if not isinstance(markdown_path, str) or not (
        markdown_path.startswith("target/gate-artifacts/verification/")
        or DATED_EVIDENCE_RE.fullmatch(markdown_path)
    ):
        failures.append(
            "output_paths.markdown must be under target/gate-artifacts/verification "
            "or a dated PLC verification evidence path"
        )
    elif not is_safe_relative_path(markdown_path):
        failures.append("output_paths.markdown must be workspace-relative")


def _validate_canonical_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, dict) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_unmapped_test_debt.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical unmapped-test debt invocation")


def _load_catalog(
    root: Path,
) -> tuple[list[dict[str, Any]] | None, dict[str, dict[str, Any]] | None, list[str]]:
    try:
        payload = tomllib.loads((root / "verification/test-catalog.toml").read_text())
    except Exception as exc:
        return None, None, [f"test catalog cannot be read: {exc}"]
    records = payload.get("tests")
    if not isinstance(records, list) or not all(isinstance(record, dict) for record in records):
        return None, None, ["test catalog must contain [[tests]] object records"]
    by_id: dict[str, dict[str, Any]] = {}
    failures: list[str] = []
    for record in records:
        test_id = record.get("id")
        if not isinstance(test_id, str) or not test_id:
            failures.append("test catalog record lacks a string id")
        elif test_id in by_id:
            failures.append(f"test catalog duplicates test id {test_id}")
        else:
            by_id[test_id] = record
    return records, by_id, failures


def _validate_source_commit(root: Path, value: Any, input_paths: list[str]) -> list[str]:
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(value, str) or not COMMIT_RE.fullmatch(value):
        return sorted(
            set([*failures, "commit must identify a clean source revision for at-rest validation"])
        )
    commit = value
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in the repository: {commit}"]
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "-z", commit],
        check=False,
        capture_output=True,
    )
    if tree.returncode != 0:
        return [f"could not read source commit tree: {commit}"]
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append(f"source commit lacks report inputs: {', '.join(missing[:5])}")
    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", commit, "--", *input_paths],
        check=False,
    )
    if diff.returncode == 1:
        failures.append("current report inputs differ from the clean source commit")
    elif diff.returncode != 0:
        failures.append(f"could not compare report inputs with source commit: exit {diff.returncode}")
    return failures


def _validate_input_binding(
    root: Path,
    payload: Mapping[str, Any],
    expected_inputs: list[str],
) -> list[str]:
    failures: list[str] = []
    if payload.get("input_paths") != expected_inputs:
        failures.append("input_paths do not match current scanner, catalog, tool, and schema inputs")
    failures.extend(validate_bound_input_paths(root, expected_inputs))
    if payload.get("input_digest") != input_digest(root, expected_inputs):
        failures.append("input_digest does not match current report inputs")
    return failures


def _expected_markdown(payload: Mapping[str, Any], json_bytes: bytes) -> str | None:
    try:
        provenance = UnmappedDebtProvenance(
            command=tuple(payload["command"]),
            commit=payload["commit"],
            timestamp=payload["timestamp"],
            platform=payload["platform"],
            input_paths=tuple(payload["input_paths"]),
            output_json=payload["output_paths"]["json"],
            output_markdown=payload["output_paths"]["markdown"],
        )
        analysis = {field: payload[field] for field in ANALYSIS_FIELDS}
        report = UnmappedTestDebtReport(
            provenance=provenance,
            input_digest=payload["input_digest"],
            analysis=analysis,
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


def _unmapped_sort_key(row: Mapping[str, Any]) -> tuple[str, str, str, str]:
    return tuple(
        str(row.get(field, "")) for field in ("source_kind", "path", "name", "discovery_id")
    )


def _is_iso_timestamp(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(root: Path, path: Path, label: str, failures: list[str]) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        failures.append(f"{label} escapes the workspace")
        return None


def _display_path(root: Path, path: Path) -> str:
    if not path.is_absolute():
        return path.as_posix()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
