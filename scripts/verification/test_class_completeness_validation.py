"""At-rest validation for test-class completeness reports."""

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
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_validation import (
    check_supported_schema_keywords,
    is_safe_relative_path,
    validate_report_payload as validate_generated_catalog_payload,
)
from .test_class_completeness import (
    GENERATOR,
    GENERATOR_VERSION,
    INFERENCE_PROHIBITED,
    LIMITATIONS,
    REPORT_CONTRACT_PATHS,
    CompletenessProvenance,
    TestClassCompletenessReport,
    analyze_test_class_completeness,
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
    "scanner_classification",
    "areas",
    "limitations",
}
ANALYSIS_FIELDS = {"scope", "summary", "scanner_classification", "areas", "limitations"}
SCOPE_FIELDS = {
    "classification_basis",
    "class_completeness_basis",
    "runnable_statuses",
    "excluded_scanner_ignore_states",
    "debt_is_report_failure",
    "inference_prohibited",
}
SUMMARY_FIELDS = {
    "scanner_facts",
    "classified_scanner_facts",
    "unmapped_scanner_facts",
    "catalog_records",
    "runnable_catalog_records",
    "non_runnable_catalog_records",
    "mapped_areas",
    "required_class_slots",
    "complete_class_slots",
    "missing_class_slots",
}
SCANNER_FIELDS = {
    "facts",
    "classified_facts",
    "unmapped_facts",
    "classified_mappings",
    "by_source_kind",
}
AREA_FIELDS = {"area", "required_classes", "additional_classes"}
CLASS_FIELDS = {"test_class", "runnable_test_ids", "non_runnable_tests", "complete"}
MAPPING_FIELDS = {"discovery_id", "test_id"}
SOURCE_COUNT_FIELDS = {"source_kind", "facts", "classified", "unmapped"}
NON_RUNNABLE_FIELDS = {"test_id", "status", "reason"}
COMMIT_RE = re.compile(r"^(?:[0-9a-f]{40}|dirty:[0-9a-f]{40}|unavailable)$")
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
    if not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must be a full Git SHA, dirty full SHA, or unavailable")
    for field in ("timestamp", "platform"):
        if not isinstance(payload.get(field), str) or not payload[field]:
            failures.append(f"{field} must be a non-empty string")
    if isinstance(payload.get("timestamp"), str) and not _is_iso_timestamp(payload["timestamp"]):
        failures.append("timestamp must be an ISO-8601 value with a timezone")

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
                failures.append(f"{field} does not match current completeness analysis")
    return failures


def validate_markdown_binding(payload: Mapping[str, Any], json_bytes: bytes, markdown: str) -> list[str]:
    summary = payload.get("summary", {})
    digest = hashlib.sha256(json_bytes).hexdigest()
    expected_markers = [
        f"Generated JSON SHA-256: `{digest}`",
        f"Input SHA-256: `{payload.get('input_digest')}`",
        f"Source revision: `{payload.get('commit')}`",
        f"- Scanner facts: {summary.get('scanner_facts')}",
        f"- Unmapped scanner facts: {summary.get('unmapped_scanner_facts')}",
        f"- Missing required class slots: {summary.get('missing_class_slots')}",
    ]
    return [
        f"completeness Markdown is missing bound marker: {marker}"
        for marker in expected_markers
        if marker not in markdown
    ]


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    """Recompute the live joins and bind the generated Markdown to JSON."""

    root = root.resolve()
    failures: list[str] = []
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
    try:
        schema = json.loads(schema_path.read_text())
    except Exception as exc:
        schema = None
        failures.append(f"completeness schema cannot be read: {exc}")
    if isinstance(schema, dict):
        failures.extend(validate_schema_contract(schema))

    json_file = _absolute(root, json_path)
    markdown_file = _absolute(root, markdown_path)
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
    except Exception as exc:
        return [*failures, f"completeness JSON cannot be read: {exc}"]
    failures.extend(validate_report_payload(payload))
    if isinstance(schema, dict):
        failures.extend(validate_json_schema_instance(payload, schema))
    if not isinstance(payload, dict):
        return failures

    expected_json_path = _relative(root, json_file, "JSON output", failures)
    expected_markdown_path = _relative(root, markdown_file, "Markdown output", failures)
    outputs = payload.get("output_paths", {})
    if isinstance(outputs, dict):
        if expected_json_path is not None and outputs.get("json") != expected_json_path:
            failures.append("output_paths.json does not identify the validated JSON file")
        if expected_markdown_path is not None and outputs.get("markdown") != expected_markdown_path:
            failures.append("output_paths.markdown does not identify the validated Markdown file")

    scan = scan_repository(root)
    scan_payload = scan.to_dict()
    failures.extend(
        f"generated catalog: {item}" for item in validate_generated_catalog_payload(scan_payload)
    )
    if scan_payload.get("scan_status") != "complete":
        failures.append("generated catalog scan_status is not complete")
    matrix, tests, load_failures = _load_metadata_inputs(root)
    failures.extend(load_failures)
    if matrix is not None and tests is not None:
        tests_by_id = {
            record["id"]: record
            for record in tests
            if isinstance(record, dict) and isinstance(record.get("id"), str)
        }
        failures.extend(
            validate_catalog_staleness(
                root=root,
                tests=tests_by_id,
                facts=scan.inferred_facts,
            )
        )
        try:
            expected_analysis = analyze_test_class_completeness(
                matrix=matrix,
                tests=tests,
                facts=scan.inferred_facts,
            )
        except ValueError as exc:
            failures.append(f"completeness analysis failed: {exc}")
        else:
            failures.extend(validate_report_payload(payload, expected_analysis=expected_analysis))

    expected_inputs = sorted(
        set(scan.provenance.input_paths)
        | set(REPORT_CONTRACT_PATHS)
        | {"verification/matrix.toml", "verification/test-catalog.toml"}
    )
    if payload.get("input_paths") != expected_inputs:
        failures.append("input_paths do not match current scanner, matrix, and catalog inputs")
    missing_inputs = [path for path in expected_inputs if not (root / path).is_file()]
    if missing_inputs:
        failures.append(f"input paths no longer exist: {', '.join(missing_inputs[:5])}")
    if payload.get("input_digest") != input_digest(root, expected_inputs):
        failures.append("input_digest does not match current report inputs")
    failures.extend(_validate_source_commit(root, payload.get("commit"), expected_inputs))

    try:
        markdown = markdown_file.read_text()
    except Exception as exc:
        return [*failures, f"completeness Markdown cannot be read: {exc}"]
    failures.extend(validate_markdown_binding(payload, json_bytes, markdown))
    expected_markdown = _expected_markdown(payload, json_bytes)
    if expected_markdown is not None and not markdown.startswith(expected_markdown):
        failures.append("completeness Markdown generated section does not match the JSON payload")
    return sorted(set(failures))


def validate_schema_contract(schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("completeness schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("completeness schema root required fields drift from semantic validation")
    properties = schema.get("properties", {})
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"completeness schema const for {field} drifts from report contract")
    definitions = schema.get("$defs", {})
    expected_definitions = {
        "area": AREA_FIELDS,
        "class": CLASS_FIELDS,
        "mapping": MAPPING_FIELDS,
        "non_runnable": NON_RUNNABLE_FIELDS,
        "scanner": SCANNER_FIELDS,
        "scope": SCOPE_FIELDS,
        "source_count": SOURCE_COUNT_FIELDS,
        "summary": SUMMARY_FIELDS,
    }
    for name, expected_fields in expected_definitions.items():
        definition = definitions.get(name, {}) if isinstance(definitions, dict) else {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"completeness schema {name} must be a closed object")
        if set(definition.get("required", [])) != expected_fields:
            failures.append(
                f"completeness schema {name} required fields drift from semantic validation"
            )
    scope = definitions.get("scope", {}) if isinstance(definitions, dict) else {}
    scope_properties = scope.get("properties", {}) if isinstance(scope, dict) else {}
    expected_scope_consts = {
        "classification_basis": "exact_generated_test_discovery_id",
        "class_completeness_basis": (
            "mapped_area_required_classes_with_effectively_runnable_catalog_rows"
        ),
        "debt_is_report_failure": False,
    }
    for field, expected in expected_scope_consts.items():
        if scope_properties.get(field, {}).get("const") != expected:
            failures.append(f"completeness schema const for scope.{field} drifts from report contract")
    expected_scope_enums = {
        "runnable_statuses": {"implemented", "mapped", "test_written", "validated"},
        "excluded_scanner_ignore_states": {"conditional", "ignored"},
        "inference_prohibited": set(INFERENCE_PROHIBITED),
    }
    for field, expected in expected_scope_enums.items():
        actual = scope_properties.get(field, {}).get("items", {}).get("enum")
        if set(actual or []) != expected:
            failures.append(f"completeness schema enum for scope.{field} drifts from report contract")
    return failures


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


def _validate_analysis_shape(payload: Mapping[str, Any], failures: list[str]) -> None:
    scope = payload.get("scope")
    if isinstance(scope, dict):
        _check_exact_fields(scope, SCOPE_FIELDS, "scope", failures)
        expected_scope = {
            "classification_basis": "exact_generated_test_discovery_id",
            "class_completeness_basis": (
                "mapped_area_required_classes_with_effectively_runnable_catalog_rows"
            ),
            "runnable_statuses": ["implemented", "mapped", "test_written", "validated"],
            "excluded_scanner_ignore_states": ["conditional", "ignored"],
            "debt_is_report_failure": False,
            "inference_prohibited": list(INFERENCE_PROHIBITED),
        }
        if scope != expected_scope:
            failures.append("scope does not match the completeness contract")
    else:
        failures.append("scope must be an object")
    summary = payload.get("summary")
    if isinstance(summary, dict):
        _check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
    else:
        failures.append("summary must be an object")
    scanner = payload.get("scanner_classification")
    if isinstance(scanner, dict):
        _check_exact_fields(scanner, SCANNER_FIELDS, "scanner_classification", failures)
        _validate_object_list(scanner.get("classified_mappings"), MAPPING_FIELDS, "classified_mappings", failures)
        _validate_object_list(scanner.get("by_source_kind"), SOURCE_COUNT_FIELDS, "by_source_kind", failures)
    else:
        failures.append("scanner_classification must be an object")
    areas = payload.get("areas")
    if not isinstance(areas, list):
        failures.append("areas must be an array")
    else:
        for index, area in enumerate(areas):
            label = f"areas[{index}]"
            if not isinstance(area, dict):
                failures.append(f"{label} must be an object")
                continue
            _check_exact_fields(area, AREA_FIELDS, label, failures)
            for field in ("required_classes", "additional_classes"):
                classes = area.get(field)
                if not isinstance(classes, list):
                    failures.append(f"{label}.{field} must be an array")
                    continue
                for class_index, item in enumerate(classes):
                    class_label = f"{label}.{field}[{class_index}]"
                    if not isinstance(item, dict):
                        failures.append(f"{class_label} must be an object")
                        continue
                    _check_exact_fields(item, CLASS_FIELDS, class_label, failures)
                    _validate_object_list(
                        item.get("non_runnable_tests"),
                        NON_RUNNABLE_FIELDS,
                        f"{class_label}.non_runnable_tests",
                        failures,
                    )
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the reviewed report contract")


def _validate_internal_counts(payload: Mapping[str, Any], failures: list[str]) -> None:
    summary = payload.get("summary")
    scanner = payload.get("scanner_classification")
    areas = payload.get("areas")
    if not isinstance(summary, dict) or not isinstance(scanner, dict) or not isinstance(areas, list):
        return
    expected_scanner = {
        "scanner_facts": scanner.get("facts"),
        "classified_scanner_facts": scanner.get("classified_facts"),
        "unmapped_scanner_facts": scanner.get("unmapped_facts"),
    }
    for field, value in expected_scanner.items():
        if summary.get(field) != value:
            failures.append(f"summary.{field} does not match scanner classification")
    if all(isinstance(scanner.get(field), int) for field in ("facts", "classified_facts", "unmapped_facts")):
        if scanner["classified_facts"] + scanner["unmapped_facts"] != scanner["facts"]:
            failures.append("scanner classified and unmapped counts do not partition facts")
    required_items = [
        item
        for area in areas
        if isinstance(area, dict)
        for item in area.get("required_classes", [])
        if isinstance(item, dict)
    ]
    complete = sum(1 for item in required_items if item.get("complete") is True)
    expected_slots = {
        "mapped_areas": len(areas),
        "required_class_slots": len(required_items),
        "complete_class_slots": complete,
        "missing_class_slots": len(required_items) - complete,
    }
    for field, value in expected_slots.items():
        if summary.get(field) != value:
            failures.append(f"summary.{field} does not match area class rows")
    for item in required_items:
        runnable = item.get("runnable_test_ids")
        if isinstance(runnable, list) and item.get("complete") is not bool(runnable):
            failures.append(
                f"required class {item.get('test_class')} complete flag does not match runnable tests"
            )


def _validate_object_list(
    value: Any,
    fields: set[str],
    label: str,
    failures: list[str],
) -> None:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            failures.append(f"{label}[{index}] must be an object")
        else:
            _check_exact_fields(item, fields, f"{label}[{index}]", failures)


def _load_metadata_inputs(root: Path) -> tuple[dict | None, list[dict] | None, list[str]]:
    failures: list[str] = []
    try:
        matrix = tomllib.loads((root / "verification/matrix.toml").read_text())
    except Exception as exc:
        matrix = None
        failures.append(f"matrix cannot be read: {exc}")
    try:
        catalog = tomllib.loads((root / "verification/test-catalog.toml").read_text())
        tests = catalog.get("tests")
        if not isinstance(tests, list) or not all(isinstance(item, dict) for item in tests):
            raise ValueError("expected [[tests]] records")
    except Exception as exc:
        tests = None
        failures.append(f"test catalog cannot be read: {exc}")
    return matrix, tests, failures


def _validate_canonical_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, dict) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_test_class_completeness.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical completeness generator invocation")


def _is_iso_timestamp(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _validate_source_commit(root: Path, value: Any, input_paths: list[str]) -> list[str]:
    if not isinstance(value, str) or value == "unavailable":
        return ["commit must resolve to a repository commit for at-rest validation"]
    dirty = value.startswith("dirty:")
    commit = value.removeprefix("dirty:")
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in the repository: {commit}"]
    if dirty:
        return []
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "-z", commit],
        check=False,
        capture_output=True,
    )
    if tree.returncode != 0:
        return [f"could not read source commit tree: {commit}"]
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    failures: list[str] = []
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


def _expected_markdown(payload: Mapping[str, Any], json_bytes: bytes) -> str | None:
    try:
        provenance = CompletenessProvenance(
            command=tuple(payload["command"]),
            commit=payload["commit"],
            timestamp=payload["timestamp"],
            platform=payload["platform"],
            input_paths=tuple(payload["input_paths"]),
            output_json=payload["output_paths"]["json"],
            output_markdown=payload["output_paths"]["markdown"],
        )
        analysis = {field: payload[field] for field in ANALYSIS_FIELDS}
        report = TestClassCompletenessReport(
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
