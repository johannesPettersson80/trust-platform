"""Semantic validation for generated existing-test catalog reports."""

from __future__ import annotations

import hashlib
import json
import re
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any

from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_models import GENERATOR, GENERATOR_VERSION, HAND_OWNED_FIELDS


ROOT_FIELDS = {
    "schema_version",
    "generator",
    "generator_version",
    "scan_status",
    "input_digest",
    "command",
    "commit",
    "timestamp",
    "platform",
    "input_paths",
    "output_paths",
    "hand_owned_intent",
    "inferred_facts",
    "diagnostics",
    "limitations",
    "summary",
}
FACT_FIELDS = {
    "stable_id",
    "native_id",
    "source_kind",
    "name",
    "path",
    "line",
    "package",
    "command_hint",
    "command_hint_authority",
    "discovery_confidence",
    "ignore_state",
    "ignore_reason",
    "reference_candidates",
    "provenance",
}
DIAGNOSTIC_FIELDS = {"severity", "kind", "path", "line", "message"}
SUMMARY_FIELDS = {
    "records",
    "files",
    "ignored",
    "conditional_ignores",
    "diagnostics",
    "errors",
    "warnings",
    "by_source_kind",
}
SOURCE_KINDS = {
    "rust_integration_test",
    "rust_unit_test",
    "structured_text_test",
    "vscode_test",
    "conformance_case",
    "fuzz_target",
    "gate_script",
    "github_workflow_job",
}
COMMAND_AUTHORITIES = {
    "exact",
    "conservative",
    "package_only",
    "file_entrypoint",
    "workflow_only",
}
DISCOVERY_CONFIDENCE = {
    "exact_attribute",
    "literal_call",
    "parsed_manifest",
    "filename_pattern",
    "yaml_job_indentation",
    "lexical_declaration",
}
IGNORE_STATES = {"not_ignored", "ignored", "conditional"}
COMMIT_RE = re.compile(r"^(?:[0-9a-f]{40}|dirty:[0-9a-f]{40}|unavailable)$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
STABLE_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
DATED_EVIDENCE_RE = re.compile(
    r"^docs/internal/testing/evidence/plc-verification-program/\d{4}-\d{2}-\d{2}/[^/]+\.md$"
)
TARGET_MARKDOWN_RE = re.compile(r"^target/gate-artifacts/verification/[^/]+\.md$")
SOURCE_PREFIX = {
    "rust_integration_test": "crates/",
    "rust_unit_test": "crates/",
    "structured_text_test": "crates/",
    "vscode_test": "editors/vscode/src/test/",
    "conformance_case": "conformance/cases/",
    "fuzz_target": "fuzz/fuzz_targets/",
    "gate_script": "scripts/",
    "github_workflow_job": ".github/workflows/",
}
SUPPORTED_SCHEMA_KEYWORDS = {
    "$schema",
    "$id",
    "$ref",
    "$defs",
    "title",
    "type",
    "required",
    "properties",
    "additionalProperties",
    "const",
    "enum",
    "pattern",
    "minItems",
    "maxItems",
    "items",
    "uniqueItems",
    "minLength",
    "minimum",
}


def validate_report_payload(payload: Any) -> list[str]:
    failures: list[str] = []
    if not isinstance(payload, dict):
        return ["report root must be an object"]
    check_exact_fields(payload, ROOT_FIELDS, "top-level", failures)
    if payload.get("schema_version") != 1:
        failures.append("schema_version must equal 1")
    if payload.get("generator") != GENERATOR:
        failures.append(f"generator must equal {GENERATOR}")
    if payload.get("generator_version") != GENERATOR_VERSION:
        failures.append(f"generator_version must equal {GENERATOR_VERSION}")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must be a full Git SHA, dirty full SHA, or unavailable")
    for field in ("timestamp", "platform"):
        if not isinstance(payload.get(field), str) or not payload[field]:
            failures.append(f"{field} must be a non-empty string")

    command = payload.get("command")
    if not isinstance(command, list) or not command or not all(isinstance(item, str) and item for item in command):
        failures.append("command must be a non-empty string array")
    input_paths = payload.get("input_paths")
    if not isinstance(input_paths, list) or not all(isinstance(item, str) for item in input_paths):
        failures.append("input_paths must be a string array")
        input_paths = []
    else:
        check_sorted_unique(input_paths, "input_paths", failures)
        for path in input_paths:
            check_relative_path(path, "input path", failures)

    outputs = payload.get("output_paths")
    if not isinstance(outputs, dict):
        failures.append("output_paths must be an object")
    else:
        check_exact_fields(outputs, {"json", "markdown"}, "output_paths", failures)
        json_path = outputs.get("json")
        markdown_path = outputs.get("markdown")
        if not isinstance(json_path, str) or not json_path.startswith("target/gate-artifacts/verification/"):
            failures.append("output_paths.json must be under target/gate-artifacts/verification")
        elif not is_safe_relative_path(json_path):
            failures.append("output_paths.json must be workspace-relative")
        if not isinstance(markdown_path, str) or not (
            DATED_EVIDENCE_RE.fullmatch(markdown_path) or TARGET_MARKDOWN_RE.fullmatch(markdown_path)
        ):
            failures.append(
                "output_paths.markdown must be under target/gate-artifacts/verification "
                "or a dated PLC verification evidence path"
            )

    intent = payload.get("hand_owned_intent")
    if not isinstance(intent, dict):
        failures.append("hand_owned_intent must be an object")
    else:
        check_exact_fields(intent, {"included", "fields"}, "hand_owned_intent", failures)
        if intent.get("included") is not False:
            failures.append("hand_owned_intent.included must be false")
        if intent.get("fields") != HAND_OWNED_FIELDS:
            failures.append("hand_owned_intent.fields drift from the hand-owned catalog contract")

    facts = payload.get("inferred_facts")
    if not isinstance(facts, list):
        failures.append("inferred_facts must be an array")
        facts = []
    for index, fact in enumerate(facts):
        validate_fact(fact, index, failures)
    fact_order = [fact_sort_key(fact) for fact in facts if isinstance(fact, dict)]
    if fact_order != sorted(fact_order):
        failures.append("inferred_facts must use canonical ordering")
    stable_ids = [fact.get("stable_id") for fact in facts if isinstance(fact, dict)]
    if len(stable_ids) != len(set(stable_ids)):
        failures.append("inferred_facts stable_id values must be unique")

    diagnostics = payload.get("diagnostics")
    if not isinstance(diagnostics, list):
        failures.append("diagnostics must be an array")
        diagnostics = []
    for index, item in enumerate(diagnostics):
        validate_diagnostic(item, index, failures)
    diagnostic_order = [diagnostic_sort_key(item) for item in diagnostics if isinstance(item, dict)]
    if diagnostic_order != sorted(diagnostic_order):
        failures.append("diagnostics must use canonical ordering")

    limitations = payload.get("limitations")
    if not isinstance(limitations, list) or not limitations or not all(
        isinstance(item, str) and item for item in limitations
    ):
        failures.append("limitations must be a non-empty string array")

    validate_summary(payload.get("summary"), facts, diagnostics, failures)
    error_count = sum(
        1 for item in diagnostics if isinstance(item, dict) and item.get("severity") == "error"
    )
    expected_status = "incomplete" if error_count else "complete"
    if payload.get("scan_status") != expected_status:
        failures.append(f"scan_status must equal {expected_status} for the diagnostic set")
    return failures


def validate_schema_file(schema_path: Path) -> list[str]:
    try:
        schema = json.loads(schema_path.read_text())
    except Exception as exc:
        return [f"generated catalog schema cannot be read: {exc}"]
    failures: list[str] = []
    if not isinstance(schema, dict):
        return ["generated catalog schema root must be an object"]
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("generated catalog schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("generated catalog schema root required fields drift from semantic validation")
    definitions = schema.get("$defs", {})
    for name, expected in (
        ("fact", FACT_FIELDS),
        ("diagnostic", DIAGNOSTIC_FIELDS),
        ("summary", SUMMARY_FIELDS),
    ):
        definition = definitions.get(name, {}) if isinstance(definitions, dict) else {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"generated catalog schema {name} must be a closed object")
        if set(definition.get("required", [])) != expected:
            failures.append(f"generated catalog schema {name} required fields drift from semantic validation")
    return failures


def check_supported_schema_keywords(
    schema: dict[str, Any],
    path: str,
    failures: list[str],
) -> None:
    for keyword in sorted(set(schema) - SUPPORTED_SCHEMA_KEYWORDS):
        failures.append(f"{path}: unsupported schema keyword {keyword}")
    properties = schema.get("properties")
    if isinstance(properties, dict):
        for name, child in properties.items():
            if isinstance(child, dict):
                check_supported_schema_keywords(child, f"{path}.properties.{name}", failures)
    definitions = schema.get("$defs")
    if isinstance(definitions, dict):
        for name, child in definitions.items():
            if isinstance(child, dict):
                check_supported_schema_keywords(child, f"{path}.$defs.{name}", failures)
    items = schema.get("items")
    if isinstance(items, dict):
        check_supported_schema_keywords(items, f"{path}.items", failures)
    additional = schema.get("additionalProperties")
    if isinstance(additional, dict):
        check_supported_schema_keywords(additional, f"{path}.additionalProperties", failures)


def validate_payload_against_schema(payload: Any, schema: dict[str, Any]) -> list[str]:
    return validate_json_schema_instance(payload, schema)


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    failures = validate_schema_file(schema_path)
    try:
        json_bytes = json_path.read_bytes()
        payload = json.loads(json_bytes)
    except Exception as exc:
        return [*failures, f"generated catalog JSON cannot be read: {exc}"]
    failures.extend(validate_report_payload(payload))
    try:
        schema = json.loads(schema_path.read_text())
    except Exception:
        schema = None
    if isinstance(schema, dict):
        failures.extend(validate_payload_against_schema(payload, schema))
    input_paths = payload.get("input_paths", []) if isinstance(payload, dict) else []
    if isinstance(input_paths, list) and all(isinstance(item, str) for item in input_paths):
        missing = [path for path in input_paths if not (root / path).is_file()]
        if missing:
            failures.append(f"input paths no longer exist: {', '.join(missing[:5])}")
        expected_input_digest = input_digest(root, input_paths)
        if payload.get("input_digest") != expected_input_digest:
            failures.append("input_digest does not match current source inputs")
    facts = payload.get("inferred_facts", []) if isinstance(payload, dict) else []
    input_set = set(input_paths) if isinstance(input_paths, list) else set()
    missing_fact_inputs = sorted(
        {
            fact.get("path")
            for fact in facts
            if isinstance(fact, dict) and fact.get("path") not in input_set
        }
    )
    if missing_fact_inputs:
        failures.append(f"fact paths are absent from input_paths: {', '.join(missing_fact_inputs[:5])}")
    try:
        markdown = markdown_path.read_text()
    except Exception as exc:
        return [*failures, f"generated catalog Markdown cannot be read: {exc}"]
    json_digest = hashlib.sha256(json_bytes).hexdigest()
    expected_markers = [
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload.get('input_digest')}`",
        f"Source revision: `{payload.get('commit')}`",
        f"- Records: {payload.get('summary', {}).get('records')}",
        f"- Visible scan diagnostics: {payload.get('summary', {}).get('diagnostics')}",
    ]
    for marker in expected_markers:
        if marker not in markdown:
            failures.append(f"generated catalog Markdown is missing bound marker: {marker}")
    return failures


def validate_fact(fact: Any, index: int, failures: list[str]) -> None:
    label = f"inferred_facts[{index}]"
    if not isinstance(fact, dict):
        failures.append(f"{label} must be an object")
        return
    check_exact_fields(fact, FACT_FIELDS, label, failures)
    stable_id = fact.get("stable_id")
    if not STABLE_ID_RE.fullmatch(str(stable_id or "")):
        failures.append(f"{label}.stable_id has invalid syntax")
    native_id = fact.get("native_id")
    source_kind = fact.get("source_kind")
    package = fact.get("package")
    if not isinstance(native_id, str) or not native_id:
        failures.append(f"{label}.native_id must be non-empty")
    if source_kind not in SOURCE_KINDS:
        failures.append(f"{label}.source_kind is unknown")
    if package is not None and (not isinstance(package, str) or not package):
        failures.append(f"{label}.package must be null or a non-empty string")
    if isinstance(native_id, str) and source_kind in SOURCE_KINDS and (package is None or isinstance(package, str)):
        identity = f"{source_kind}\0{package or ''}\0{native_id}".encode()
        expected = "DISC_" + hashlib.sha256(identity).hexdigest()[:20].upper()
        if stable_id != expected:
            failures.append(f"{label}.stable_id does not match its semantic identity")
    for field in ("name", "command_hint", "command_hint_authority", "discovery_confidence", "provenance"):
        if not isinstance(fact.get(field), str) or not fact[field]:
            failures.append(f"{label}.{field} must be non-empty")
    if fact.get("command_hint_authority") not in COMMAND_AUTHORITIES:
        failures.append(f"{label}.command_hint_authority is unknown")
    if fact.get("discovery_confidence") not in DISCOVERY_CONFIDENCE:
        failures.append(f"{label}.discovery_confidence is unknown")
    if fact.get("provenance") != "inferred":
        failures.append(f"{label}.provenance must equal inferred")
    path = fact.get("path")
    if not isinstance(path, str) or not is_safe_relative_path(path):
        failures.append(f"{label}.path must be workspace-relative")
    elif source_kind in SOURCE_PREFIX and not path.startswith(SOURCE_PREFIX[source_kind]):
        failures.append(f"{label}.path is outside its declared source surface")
    if not isinstance(fact.get("line"), int) or isinstance(fact.get("line"), bool) or fact["line"] < 1:
        failures.append(f"{label}.line must be a positive integer")
    ignore_state = fact.get("ignore_state")
    reason = fact.get("ignore_reason")
    if ignore_state not in IGNORE_STATES:
        failures.append(f"{label}.ignore_state is unknown")
    if ignore_state == "not_ignored" and reason is not None:
        failures.append(f"{label}.ignore_reason must be null when not ignored")
    if ignore_state != "not_ignored" and (not isinstance(reason, str) or not reason):
        failures.append(f"{label}.ignore_reason must explain the ignore state")
    references = fact.get("reference_candidates")
    if not isinstance(references, list) or not all(isinstance(item, str) and item for item in references):
        failures.append(f"{label}.reference_candidates must be a string array")
    else:
        check_sorted_unique(references, f"{label}.reference_candidates", failures)
    validate_source_contract(fact, label, failures)


def validate_source_contract(fact: dict[str, Any], label: str, failures: list[str]) -> None:
    kind = fact.get("source_kind")
    package = fact.get("package")
    command = fact.get("command_hint")
    authority = fact.get("command_hint_authority")
    confidence = fact.get("discovery_confidence")
    path = fact.get("path")
    name = fact.get("name")
    native_id = fact.get("native_id")

    def require(condition: bool, message: str) -> None:
        if not condition:
            failures.append(f"{label} {kind} {message}")

    if kind == "rust_integration_test":
        require(isinstance(package, str), "package must be source-derived")
        require(authority in {"conservative", "package_only"}, "command authority is invalid")
        require(confidence == "exact_attribute", "discovery confidence is invalid")
        require(isinstance(path, str) and "/tests/" in path, "path must be under a crate tests tree")
        require(
            isinstance(command, str) and command.startswith(f"cargo test -p {package} "),
            "command_hint must be a package-scoped cargo test command",
        )
    elif kind == "rust_unit_test":
        require(isinstance(package, str), "package must be source-derived")
        require(authority == "package_only", "command authority is invalid")
        require(confidence == "exact_attribute", "discovery confidence is invalid")
        require(isinstance(path, str) and "/src/" in path, "path must be under a crate source tree")
        require(
            isinstance(command, str) and command.startswith(f"cargo test -p {package} "),
            "command_hint must be a package-scoped cargo test command",
        )
    elif kind == "structured_text_test":
        require(isinstance(package, str), "package must be source-derived")
        require(authority == "conservative", "command authority is invalid")
        require(confidence == "lexical_declaration", "discovery confidence is invalid")
        require(isinstance(path, str) and "/tests/" in path, "path must be under a crate tests tree")
        require(
            isinstance(command, str)
            and command.startswith("cargo run -p trust-dev -- test --project ")
            and f" --filter {name}" in command,
            "command_hint must use the trust-dev project test filter",
        )
        require(
            isinstance(native_id, str)
            and native_id.startswith(("TEST_PROGRAM::", "TEST_FUNCTION_BLOCK::")),
            "native_id must preserve the declaration kind",
        )
    elif kind == "vscode_test":
        require(isinstance(package, str), "package must be source-derived")
        require(authority == "package_only", "command authority is invalid")
        require(confidence == "literal_call", "discovery confidence is invalid")
        require(command == "cd editors/vscode && npm test", "command_hint must be the package test command")
    elif kind == "conformance_case":
        require(package == "trust-runtime", "package must equal trust-runtime")
        require(authority == "exact", "command authority is invalid")
        require(confidence == "parsed_manifest", "discovery confidence is invalid")
        require(
            command
            == (
                "cargo run -p trust-runtime --bin trust-runtime -- conformance "
                f"--suite-root conformance --filter {name}"
            ),
            "command_hint does not match the conformance runner contract",
        )
    elif kind == "fuzz_target":
        require(isinstance(package, str), "package must be source-derived")
        require(authority == "exact", "command authority is invalid")
        require(confidence == "parsed_manifest", "discovery confidence is invalid")
        require(command == f"cd fuzz && cargo fuzz run {name}", "command_hint is invalid")
    elif kind == "gate_script":
        require(package is None, "package must be null")
        require(authority == "file_entrypoint", "command authority is invalid")
        require(confidence == "filename_pattern", "discovery confidence is invalid")
        expected = f"python3 {path}" if isinstance(path, str) and path.endswith(".py") else path
        require(command == expected, "command_hint must match the discovered entrypoint")
    elif kind == "github_workflow_job":
        require(package is None, "package must be null")
        require(authority == "workflow_only", "command authority is invalid")
        require(confidence == "yaml_job_indentation", "discovery confidence is invalid")
        require(command == f"workflow job {native_id}", "command_hint must remain a non-runnable locator")


def validate_diagnostic(item: Any, index: int, failures: list[str]) -> None:
    label = f"diagnostics[{index}]"
    if not isinstance(item, dict):
        failures.append(f"{label} must be an object")
        return
    check_exact_fields(item, DIAGNOSTIC_FIELDS, label, failures)
    if item.get("severity") not in {"warning", "error"}:
        failures.append(f"{label}.severity must be warning or error")
    for field in ("kind", "path", "message"):
        if not isinstance(item.get(field), str) or not item[field]:
            failures.append(f"{label}.{field} must be non-empty")
    if item.get("path") != "<generated>" and isinstance(item.get("path"), str):
        check_relative_path(item["path"], f"{label}.path", failures)
    if not isinstance(item.get("line"), int) or isinstance(item.get("line"), bool) or item["line"] < 1:
        failures.append(f"{label}.line must be a positive integer")


def validate_summary(summary: Any, facts: list[Any], diagnostics: list[Any], failures: list[str]) -> None:
    if not isinstance(summary, dict):
        failures.append("summary must be an object")
        return
    check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
    valid_facts = [fact for fact in facts if isinstance(fact, dict)]
    valid_diagnostics = [item for item in diagnostics if isinstance(item, dict)]
    expected = {
        "records": len(facts),
        "files": len({fact.get("path") for fact in valid_facts}),
        "ignored": sum(1 for fact in valid_facts if fact.get("ignore_state") == "ignored"),
        "conditional_ignores": sum(
            1 for fact in valid_facts if fact.get("ignore_state") == "conditional"
        ),
        "diagnostics": len(diagnostics),
        "errors": sum(1 for item in valid_diagnostics if item.get("severity") == "error"),
        "warnings": sum(1 for item in valid_diagnostics if item.get("severity") == "warning"),
        "by_source_kind": dict(sorted(Counter(fact.get("source_kind") for fact in valid_facts).items())),
    }
    for field, value in expected.items():
        if summary.get(field) != value:
            failures.append(f"summary.{field} does not match inferred report content")


def check_exact_fields(value: dict[str, Any], expected: set[str], label: str, failures: list[str]) -> None:
    actual = set(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing:
        failures.append(f"{label} missing fields: {', '.join(missing)}")
    if extra:
        failures.append(f"{label} has unexpected fields: {', '.join(extra)}")


def check_sorted_unique(values: list[str], label: str, failures: list[str]) -> None:
    if values != sorted(set(values)):
        failures.append(f"{label} must be sorted and duplicate-free")


def check_relative_path(value: str, label: str, failures: list[str]) -> None:
    if not is_safe_relative_path(value):
        failures.append(f"{label} must be a normalized workspace-relative path")


def is_safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and path.as_posix() == value


def fact_sort_key(fact: dict[str, Any]) -> tuple[Any, ...]:
    return (
        fact.get("source_kind", ""),
        fact.get("path", ""),
        fact.get("line", 0),
        fact.get("name", ""),
        fact.get("stable_id", ""),
    )


def diagnostic_sort_key(item: dict[str, Any]) -> tuple[Any, ...]:
    return (
        item.get("path", ""),
        item.get("line", 0),
        item.get("severity", ""),
        item.get("kind", ""),
        item.get("message", ""),
    )
