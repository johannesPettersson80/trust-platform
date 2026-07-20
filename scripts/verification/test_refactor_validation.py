"""At-rest validation for the Phase 2A existing-test refactor assessment."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import (
    check_supported_schema_keywords,
    is_safe_relative_path,
)
from .test_refactor_live import build_live_test_refactor_state
from .test_refactor_report import (
    GENERATOR,
    GENERATOR_VERSION,
    RefactorAssessmentProvenance,
    TestRefactorAssessmentReport,
)


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_PATTERN = r"^[0-9a-f]{40}$"
DIGEST_PATTERN = r"^sha256:[0-9a-f]{64}$"
DATED_EVIDENCE_RE = re.compile(
    r"^docs/internal/testing/evidence/plc-verification-program/\d{4}-\d{2}-\d{2}/[^/]+\.md$"
)
ASSESSMENT_FIELDS = (
    "summary",
    "file_assessment",
    "broad_claim_assessment",
    "duplicate_assessment",
    "vscode_registration",
    "duration_classification",
    "proposal_evaluations",
)
TOP_LEVEL_FIELDS = {
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
    *ASSESSMENT_FIELDS,
    "limitations",
}
SCOPE_FIELDS = {
    "large_file_line_threshold",
    "large_threshold_source",
    "mixed_purpose_basis",
    "broad_claim_basis",
    "duplicate_basis",
    "duration_basis",
    "debt_is_report_failure",
}
SUMMARY_FIELDS = {
    "broad_claim_candidates",
    "catalog_records",
    "catalog_slow_records",
    "exact_case_input_duplicate_groups",
    "exact_fact_file_duplicate_groups",
    "fact_files",
    "large_file_candidates",
    "malformed_class_overlap_groups",
    "proposals",
    "reviewed_mapping_diversity_candidates",
    "scanner_duration_classified",
    "scanner_duration_unclassified",
    "scanner_facts",
    "shared_case_reference_groups",
    "structural_case_input_peer_groups",
    "supported_proposals",
    "vscode_facts",
    "vscode_files",
    "vscode_large_candidates",
    "vscode_registrations",
    "whitespace_normalized_fact_file_duplicate_groups",
}
SCHEMA_OBJECT_FIELDS = {
    "output_paths": {"json", "markdown"},
    "scope": SCOPE_FIELDS,
    "summary": SUMMARY_FIELDS,
    "file_assessment": {
        "candidate_reasons", "conditional_count", "ignored_count",
        "mapped_test_ids", "packages", "path", "physical_lines",
        "reviewed_areas", "reviewed_invariant_ids", "reviewed_test_classes",
        "scanner_fact_count", "source_kinds", "unmapped_fact_count",
    },
    "broad_claim": {
        "coverage_dimensions", "invariant_count", "invariants", "path", "result", "test_id",
    },
    "content_group": {"content_digest", "paths"},
    "exact_case_group": {"case_files", "case_ids", "input_digest"},
    "structural_case_group": {"case_file", "case_ids", "shape_digest"},
    "shared_case_group": {"case_file", "record_paths", "test_ids"},
    "malformed_overlap": {"malformed_input_class_id", "paths", "test_ids"},
    "duplicate_assessment": {
        "case_file_paths", "exact_case_input_groups", "exact_fact_file_groups",
        "malformed_class_overlap_groups", "shared_case_reference_groups",
        "source_body_similarity", "structural_case_input_peer_groups",
        "whitespace_normalized_fact_file_groups",
    },
    "diagnostic": {"kind", "path", "severity"},
    "registration_issues": {
        "duplicate_targets", "missing_targets", "unregistered_fact_files", "unregistered_files",
    },
    "vscode_file": {
        "fact_count", "ignored_count", "large_candidate", "mapped_count", "path",
        "physical_lines", "registration_line", "specifier",
    },
    "vscode_registration": {
        "diagnostics", "fact_count", "files", "index_path", "registration_count",
        "test_file_count", "registration_issues",
    },
    "scanner_duration": {
        "catalog_test_id", "classification_source", "discovery_id", "duration_class",
        "ignore_state", "name", "path", "source_kind",
    },
    "artifact_duration": {"duration_class", "path", "subject_kind", "suite_tiers", "test_id"},
    "suite_tier": {"commands_configured", "placeholder", "suite_id"},
    "duration_classification": {
        "artifact_catalog_records", "commandless_suite_ids", "placeholder_suite_ids",
        "scanner_facts", "suite_tiers", "unassigned_tier_test_ids",
        "unknown_assigned_suite_ids",
    },
    "proposal_evaluation": {
        "disposition", "observed_signals", "proposal_id", "source_paths", "supported",
    },
}


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_assessment: Mapping[str, Any] | None = None,
    expected_scope: Mapping[str, Any] | None = None,
    expected_limitations: tuple[str, ...] | None = None,
) -> list[str]:
    failures: list[str] = []
    _check_exact_fields(payload, TOP_LEVEL_FIELDS, "report", failures)
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
        failures.append("commit must be a clean full Git SHA")
    timestamp = payload.get("timestamp")
    if not isinstance(timestamp, str) or not _is_iso_timestamp(timestamp):
        failures.append("timestamp must be an ISO-8601 value with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")
    if not isinstance(payload.get("input_digest"), str) or not DIGEST_RE.fullmatch(
        str(payload.get("input_digest", ""))
    ):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    input_paths = payload.get("input_paths")
    if not isinstance(input_paths, list) or not all(
        isinstance(item, str) for item in input_paths
    ):
        failures.append("input_paths must be an array of strings")
    elif input_paths != sorted(set(input_paths)):
        failures.append("input_paths must be sorted and duplicate-free")
    limitations = payload.get("limitations")
    if not isinstance(limitations, list) or not limitations or not all(
        isinstance(item, str) and item for item in limitations
    ):
        failures.append("limitations must be a non-empty string array")
    scope = payload.get("scope")
    if not isinstance(scope, Mapping):
        failures.append("scope must be an object")
    else:
        _check_exact_fields(scope, SCOPE_FIELDS, "scope", failures)
        threshold = scope.get("large_file_line_threshold")
        if not isinstance(threshold, int) or isinstance(threshold, bool) or threshold < 1:
            failures.append("scope.large_file_line_threshold must be a positive integer")
        if scope.get("debt_is_report_failure") is not False:
            failures.append("scope.debt_is_report_failure must remain false")
        if expected_scope is not None and dict(scope) != dict(expected_scope):
            failures.append("scope does not match the current refactor-assessment contract")
    _validate_output_paths(payload.get("output_paths"), failures)
    _validate_canonical_command(payload, failures)

    assessment = {field: payload.get(field) for field in ASSESSMENT_FIELDS}
    if expected_assessment is not None and assessment != dict(expected_assessment):
        failures.append("report assessment does not match current refactor assessment")
    if expected_limitations is not None and limitations != list(expected_limitations):
        failures.append("limitations do not match the current refactor-assessment contract")
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    failures: list[str] = []
    expected = _expected_markdown(payload, json_bytes)
    if expected is None:
        return ["test-refactor assessment Markdown cannot be reconstructed from JSON"]
    if markdown != expected:
        failures.append("test-refactor assessment Markdown does not exactly match JSON")
    digest = hashlib.sha256(json_bytes).hexdigest()
    if f"Generated JSON SHA-256: `{digest}`" not in markdown:
        failures.append("test-refactor assessment Markdown has stale JSON digest")
    return failures


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object":
        failures.append("report schema root must be an object")
    if schema.get("additionalProperties") is not False:
        failures.append("report schema root must forbid additional properties")
    if set(schema.get("required", [])) != TOP_LEVEL_FIELDS:
        failures.append("report schema required fields drift from validator")
    if set(schema.get("properties", {})) != TOP_LEVEL_FIELDS:
        failures.append("report schema properties drift from validator")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"report schema const for {field} drifts from validator")
    if properties.get("commit", {}).get("pattern") != COMMIT_PATTERN:
        failures.append("report schema commit pattern must require a clean full SHA")
    if properties.get("input_digest", {}).get("pattern") != DIGEST_PATTERN:
        failures.append("report schema input_digest pattern drifts from validator")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        failures.append("report schema must define closed object contracts")
    else:
        for name, expected_fields in SCHEMA_OBJECT_FIELDS.items():
            definition = definitions.get(name)
            if not isinstance(definition, Mapping):
                failures.append(f"report schema lacks {name} definition")
                continue
            if set(definition.get("required", [])) != expected_fields:
                failures.append(f"report schema {name} required fields drift from validator")
            if set(definition.get("properties", {})) != expected_fields:
                failures.append(f"report schema {name} properties drift from validator")
        scope = definitions.get("scope", {})
        if scope.get("properties", {}).get("debt_is_report_failure", {}).get("const") is not False:
            failures.append("report schema scope.debt_is_report_failure must remain false")
        broad = definitions.get("broad_claim", {})
        if set(broad.get("properties", {}).get("result", {}).get("enum", [])) != {
            "no_invariant_claim",
            "single_invariant",
            "candidate_missing_coverage_dimensions",
        }:
            failures.append("report schema broad-claim result enum drifts from validator")
        scanner_duration = definitions.get("scanner_duration", {})
        if set(
            scanner_duration.get("properties", {})
            .get("classification_source", {})
            .get("enum", [])
        ) != {"hand_catalog", "unclassified"}:
            failures.append("report schema duration classification source enum drifts")
        proposal = definitions.get("proposal_evaluation", {})
        if set(proposal.get("properties", {}).get("disposition", {}).get("enum", [])) != {
            "move",
            "rename",
            "split",
            "no_refactor_needed",
        }:
            failures.append("report schema proposal disposition enum drifts")
    _require_closed_objects(schema, "$", failures)
    return sorted(set(failures))


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    root = root.resolve()
    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    schema_file = _absolute(root, schema_path)
    failures: list[str] = []
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
        markdown = markdown_file.read_text()
        schema = json.loads(schema_file.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [f"test-refactor assessment files cannot be read: {exc}"]
    if not isinstance(payload, dict) or not isinstance(schema, dict):
        return ["test-refactor assessment JSON and schema roots must be objects"]

    canonical = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("test-refactor assessment JSON must use canonical serialization")
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    failures.extend(validate_report_payload(payload))

    expected_json = _relative(root, json_file, "JSON output", failures)
    expected_markdown = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths")
    if isinstance(outputs, Mapping):
        if expected_json is not None and outputs.get("json") != expected_json:
            failures.append("output_paths.json does not identify the validated JSON file")
        if expected_markdown is not None and outputs.get("markdown") != expected_markdown:
            failures.append("output_paths.markdown does not identify the validated Markdown file")

    try:
        state = build_live_test_refactor_state(
            root,
            timestamp=payload.get("timestamp") if isinstance(payload.get("timestamp"), str) else None,
        )
    except ValueError as exc:
        failures.append(f"live refactor assessment failed: {exc}")
    else:
        failures.extend(
            validate_report_payload(
                payload,
                expected_assessment=state.assessment,
                expected_scope=state.scope,
                expected_limitations=state.limitations,
            )
        )
        expected_inputs = list(state.input_paths)
        if payload.get("input_paths") != expected_inputs:
            failures.append("input_paths do not match the complete live report input closure")
        failures.extend(_validate_source_commit(root, payload.get("commit"), expected_inputs))
        if payload.get("input_digest") != input_digest(root, expected_inputs):
            failures.append("input_digest does not match current report inputs")

    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    return sorted(set(failures))


def _validate_output_paths(value: Any, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
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
        failures.append("output_paths.json must be normalized and workspace-relative")
    if not isinstance(markdown_path, str) or not (
        markdown_path.startswith("target/gate-artifacts/verification/")
        or DATED_EVIDENCE_RE.fullmatch(markdown_path)
    ):
        failures.append(
            "output_paths.markdown must be under target/gate-artifacts/verification "
            "or a dated PLC verification evidence path"
        )
    elif not is_safe_relative_path(markdown_path):
        failures.append("output_paths.markdown must be normalized and workspace-relative")


def _validate_canonical_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, Mapping) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_test_refactor_assessment.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical test-refactor assessment invocation")


def _validate_source_commit(root: Path, value: Any, input_paths: list[str]) -> list[str]:
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(value, str) or not COMMIT_RE.fullmatch(value):
        return sorted(set([*failures, "commit must identify a clean source revision"]))
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{value}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in the repository: {value}"]
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "-z", value],
        check=False,
        capture_output=True,
    )
    if tree.returncode != 0:
        return [f"could not read source commit tree: {value}"]
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append(f"source commit lacks report inputs: {', '.join(missing[:5])}")
    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", value, "--", *input_paths],
        check=False,
    )
    if diff.returncode == 1:
        failures.append("current report inputs differ from the clean source commit")
    elif diff.returncode != 0:
        failures.append(f"could not compare report inputs with source commit: exit {diff.returncode}")
    return failures


def _expected_markdown(payload: Mapping[str, Any], json_bytes: bytes) -> str | None:
    try:
        provenance = RefactorAssessmentProvenance(
            command=tuple(payload["command"]),
            commit=payload["commit"],
            timestamp=payload["timestamp"],
            platform=payload["platform"],
            input_paths=tuple(payload["input_paths"]),
            output_json=payload["output_paths"]["json"],
            output_markdown=payload["output_paths"]["markdown"],
        )
        report = TestRefactorAssessmentReport(
            provenance=provenance,
            input_digest=payload["input_digest"],
            scope=dict(payload["scope"]),
            assessment={field: payload[field] for field in ASSESSMENT_FIELDS},
            limitations=tuple(payload["limitations"]),
        )
    except (KeyError, TypeError, ValueError):
        return None
    return report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())


def _require_closed_objects(value: Any, path: str, failures: list[str]) -> None:
    if isinstance(value, Mapping):
        if value.get("type") == "object" and value.get("additionalProperties") is not False:
            failures.append(f"{path} object schema must forbid additional properties")
        for key, child in value.items():
            _require_closed_objects(child, f"{path}.{key}", failures)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _require_closed_objects(child, f"{path}[{index}]", failures)


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


def _is_iso_timestamp(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(
    root: Path,
    path: Path,
    label: str,
    failures: list[str],
) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        failures.append(f"{label} escapes the workspace")
        return None
