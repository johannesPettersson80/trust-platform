"""At-rest validation for malformed-input coverage reports."""

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

from .malformed_input_contract import (
    ALLOWED_DISPOSITIONS,
    load_malformed_input_taxonomy,
    validate_catalog_malformed_bindings,
    validate_malformed_input_contract,
)
from .malformed_input_coverage import (
    COVERAGE_STATES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    REPORT_CONTRACT_PATHS,
    MalformedCoverageProvenance,
    MalformedInputCoverageReport,
    analyze_malformed_input_coverage,
)
from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_validation import (
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
    "classes",
    "limitations",
}
ANALYSIS_FIELDS = {"scope", "summary", "classes", "limitations"}
SCOPE_FIELDS = {"area", "surface_id", "mapping_basis", "debt_is_report_failure", "coverage_states"}
SUMMARY_FIELDS = {"taxonomy_classes", "mapped_classes", "test_mappings", "by_state"}
CLASS_FIELDS = {
    "class_id",
    "title",
    "disposition",
    "state",
    "mapped_test_ids",
    "runnable_test_ids",
    "fuzz_test_ids",
    "non_runnable_test_ids",
    "open_spec_gap_refs",
    "rationale",
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
    if payload.get("schema_version") != 1:
        failures.append("schema_version must equal 1")
    if payload.get("generator") != GENERATOR:
        failures.append(f"generator must equal {GENERATOR}")
    if payload.get("generator_version") != GENERATOR_VERSION:
        failures.append(f"generator_version must equal {GENERATOR_VERSION}")
    if payload.get("report_status") != "complete":
        failures.append("report_status must equal complete")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not CLEAN_COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must be a clean full Git SHA")
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
                failures.append(f"{field} does not match current malformed-input analysis")
    return failures


def validate_markdown_binding(payload: Mapping[str, Any], json_bytes: bytes, markdown: str) -> list[str]:
    summary = payload.get("summary", {})
    digest = hashlib.sha256(json_bytes).hexdigest()
    expected_markers = [
        f"Generated JSON SHA-256: `{digest}`",
        f"Input SHA-256: `{payload.get('input_digest')}`",
        f"Source revision: `{payload.get('commit')}`",
        f"- Taxonomy classes: {summary.get('taxonomy_classes')}",
        f"- Classes with catalog mappings: {summary.get('mapped_classes')}",
    ]
    failures = [
        f"malformed-input Markdown is missing bound marker: {marker}"
        for marker in expected_markers
        if marker not in markdown
    ]
    canonical_json = (json.dumps(dict(payload), indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical_json:
        failures.append("malformed-input JSON is not canonical")
    expected = _expected_markdown(payload, json_bytes)
    if expected is not None and markdown != expected:
        failures.append("malformed-input Markdown does not exactly match JSON payload")
    return failures


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    """Recompute live taxonomy/catalog/scanner joins and bind Markdown to JSON."""

    root = root.resolve()
    failures: list[str] = []
    if root != METADATA_ROOT.resolve():
        failures.append("root does not identify the repository that loaded verification modules")
    else:
        validator = Validator()
        validator.load_records()
        validator.validate()
        failures.extend(
            f"metadata: {_display_path(root, failure.path)}: {failure.message}"
            for failure in validator.failures
        )
    try:
        schema = json.loads(schema_path.read_text())
    except Exception as exc:
        schema = None
        failures.append(f"malformed-input report schema cannot be read: {exc}")
    if isinstance(schema, dict):
        failures.extend(validate_schema_contract(schema))

    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
    except Exception as exc:
        return [*failures, f"malformed-input JSON cannot be read: {exc}"]
    failures.extend(validate_report_payload(payload))
    if isinstance(schema, dict):
        failures.extend(validate_json_schema_instance(payload, schema))
    if not isinstance(payload, dict):
        return sorted(set(failures))

    expected_json = _relative(root, json_file, "JSON output", failures)
    expected_markdown = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths", {})
    if isinstance(outputs, dict):
        if expected_json is not None and outputs.get("json") != expected_json:
            failures.append("output_paths.json does not identify the validated JSON file")
        if expected_markdown is not None and outputs.get("markdown") != expected_markdown:
            failures.append("output_paths.markdown does not identify the validated Markdown file")

    scan = scan_repository(root)
    scan_payload = scan.to_dict()
    failures.extend(
        f"generated catalog: {item}" for item in validate_generated_catalog_payload(scan_payload)
    )
    if scan_payload.get("scan_status") != "complete":
        failures.append("generated catalog scan_status is not complete")
    taxonomy, tests, load_failures = _load_inputs(root)
    failures.extend(load_failures)
    if taxonomy is not None:
        failures.extend(validate_malformed_input_contract(root, taxonomy))
    if taxonomy is not None and tests is not None:
        tests_by_id = {
            record["id"]: record
            for record in tests
            if isinstance(record, Mapping) and isinstance(record.get("id"), str)
        }
        if len(tests_by_id) != len(tests):
            failures.append("test catalog IDs must be present and unique")
        failures.extend(validate_catalog_malformed_bindings(tests=tests_by_id, taxonomy=taxonomy))
        failures.extend(
            validate_catalog_staleness(
                root=root,
                tests=tests_by_id,
                facts=scan.inferred_facts,
            )
        )
        try:
            expected_analysis = analyze_malformed_input_coverage(
                taxonomy=taxonomy,
                tests=tests,
                facts=scan.inferred_facts,
            )
        except ValueError as exc:
            failures.append(f"malformed-input analysis failed: {exc}")
        else:
            failures.extend(validate_report_payload(payload, expected_analysis=expected_analysis))

    expected_inputs = sorted(
        set(scan.provenance.input_paths)
        | set(REPORT_CONTRACT_PATHS)
        | validator_code_input_paths(root)
    )
    failures.extend(_validate_input_binding(root, payload, expected_inputs))

    try:
        markdown = markdown_file.read_text()
    except Exception as exc:
        return [*failures, f"malformed-input Markdown cannot be read: {exc}"]
    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    return sorted(set(failures))


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("malformed-input report schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("malformed-input report schema required fields drift")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if not isinstance(properties, Mapping) or properties.get(field, {}).get("const") != expected:
            failures.append(f"malformed-input report schema const for {field} drifts")
    if not isinstance(properties, Mapping) or properties.get("commit", {}).get("pattern") != "^[0-9a-f]{40}$":
        failures.append("malformed-input report schema clean-commit pattern drifts")
    definitions = schema.get("$defs", {})
    expected_definitions = {
        "scope": SCOPE_FIELDS,
        "summary": SUMMARY_FIELDS,
        "class": CLASS_FIELDS,
        "state_counts": set(COVERAGE_STATES),
    }
    for name, fields in expected_definitions.items():
        definition = definitions.get(name, {}) if isinstance(definitions, Mapping) else {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"malformed-input report schema {name} must be a closed object")
        if set(definition.get("required", [])) != fields:
            failures.append(f"malformed-input report schema {name} required fields drift")
    scope_properties = definitions.get("scope", {}).get("properties", {})
    expected_scope = {
        "area": "bytecode_vm",
        "surface_id": "bytecode_container_instruction_stream",
        "mapping_basis": "explicit_generated_test_malformed_input_class_ids",
        "debt_is_report_failure": False,
    }
    for field, expected in expected_scope.items():
        if scope_properties.get(field, {}).get("const") != expected:
            failures.append(f"malformed-input report schema const for scope.{field} drifts")
    actual_states = scope_properties.get("coverage_states", {}).get("items", {}).get("enum")
    if set(actual_states or []) != set(COVERAGE_STATES):
        failures.append("malformed-input report schema coverage-state enum drifts")
    class_properties = definitions.get("class", {}).get("properties", {})
    if set(class_properties.get("state", {}).get("enum", [])) != set(COVERAGE_STATES):
        failures.append("malformed-input report schema class state enum drifts")
    if set(class_properties.get("disposition", {}).get("enum", [])) != ALLOWED_DISPOSITIONS:
        failures.append("malformed-input report schema class disposition enum drifts")
    return failures


def _validate_output_paths(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("output_paths must be an object")
        return
    _check_exact_fields(value, {"json", "markdown"}, "output_paths", failures)
    json_path = value.get("json")
    markdown_path = value.get("markdown")
    if not isinstance(json_path, str) or not json_path.startswith("target/gate-artifacts/verification/"):
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
        "scripts/report_malformed_input_coverage.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical malformed-input generator invocation")


def _validate_analysis_shape(payload: Mapping[str, Any], failures: list[str]) -> None:
    scope = payload.get("scope")
    expected_scope = {
        "area": "bytecode_vm",
        "surface_id": "bytecode_container_instruction_stream",
        "mapping_basis": "explicit_generated_test_malformed_input_class_ids",
        "debt_is_report_failure": False,
        "coverage_states": list(COVERAGE_STATES),
    }
    if not isinstance(scope, dict):
        failures.append("scope must be an object")
    else:
        _check_exact_fields(scope, SCOPE_FIELDS, "scope", failures)
        if scope != expected_scope:
            failures.append("scope does not match malformed-input report contract")
    summary = payload.get("summary")
    if not isinstance(summary, dict):
        failures.append("summary must be an object")
    else:
        _check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
        by_state = summary.get("by_state")
        if not isinstance(by_state, dict):
            failures.append("summary.by_state must be an object")
        else:
            _check_exact_fields(by_state, set(COVERAGE_STATES), "summary.by_state", failures)
    classes = payload.get("classes")
    if not isinstance(classes, list):
        failures.append("classes must be an array")
    else:
        for index, item in enumerate(classes):
            if not isinstance(item, dict):
                failures.append(f"classes[{index}] must be an object")
            else:
                _check_exact_fields(item, CLASS_FIELDS, f"classes[{index}]", failures)
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match malformed-input report contract")


def _validate_internal_counts(payload: Mapping[str, Any], failures: list[str]) -> None:
    summary = payload.get("summary")
    classes = payload.get("classes")
    if not isinstance(summary, dict) or not isinstance(classes, list):
        return
    valid_classes = [item for item in classes if isinstance(item, dict)]
    expected = {
        "taxonomy_classes": len(valid_classes),
        "mapped_classes": sum(1 for item in valid_classes if item.get("mapped_test_ids")),
        "test_mappings": sum(len(item.get("mapped_test_ids", [])) for item in valid_classes),
        "by_state": {
            state: sum(1 for item in valid_classes if item.get("state") == state)
            for state in COVERAGE_STATES
        },
    }
    for field, value in expected.items():
        if summary.get(field) != value:
            failures.append(f"summary.{field} does not match malformed-input class rows")


def _load_inputs(root: Path) -> tuple[dict | None, list[dict] | None, list[str]]:
    failures: list[str] = []
    try:
        taxonomy = load_malformed_input_taxonomy(root)
    except Exception as exc:
        taxonomy = None
        failures.append(f"malformed-input taxonomy cannot be read: {exc}")
    try:
        catalog = tomllib.loads((root / "verification/test-catalog.toml").read_text())
        tests = catalog.get("tests")
        if not isinstance(tests, list) or not all(isinstance(item, dict) for item in tests):
            raise ValueError("expected [[tests]] records")
    except Exception as exc:
        tests = None
        failures.append(f"test catalog cannot be read: {exc}")
    return taxonomy, tests, failures


def _validate_source_commit(root: Path, value: Any, input_paths: list[str]) -> list[str]:
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(value, str) or not CLEAN_COMMIT_RE.fullmatch(value):
        return sorted(
            set([*failures, "commit must be a clean full Git SHA for at-rest validation"])
        )
    commit = value
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in repository: {commit}"]
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
        failures.append(f"source commit lacks malformed-input report inputs: {', '.join(missing[:5])}")
    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", commit, "--", *input_paths],
        check=False,
    )
    if diff.returncode == 1:
        failures.append("current malformed-input report inputs differ from clean source commit")
    elif diff.returncode != 0:
        failures.append(f"could not compare malformed-input inputs with source commit: exit {diff.returncode}")
    return failures


def _validate_input_binding(
    root: Path,
    payload: Mapping[str, Any],
    expected_inputs: list[str],
) -> list[str]:
    failures: list[str] = []
    if payload.get("input_paths") != expected_inputs:
        failures.append("input_paths do not match current scanner and malformed-input contract inputs")
    failures.extend(validate_bound_input_paths(root, expected_inputs))
    if payload.get("input_digest") != input_digest(root, expected_inputs):
        failures.append("input_digest does not match current malformed-input report inputs")
    failures.extend(_validate_source_commit(root, payload.get("commit"), expected_inputs))
    return failures


def _expected_markdown(payload: Mapping[str, Any], json_bytes: bytes) -> str | None:
    try:
        report = MalformedInputCoverageReport(
            provenance=MalformedCoverageProvenance(
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


def _is_iso_timestamp(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


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


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(root: Path, path: Path, label: str, failures: list[str]) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        failures.append(f"{label} escapes workspace")
        return None


def _display_path(root: Path, path: Path) -> str:
    if not path.is_absolute():
        return path.as_posix()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
