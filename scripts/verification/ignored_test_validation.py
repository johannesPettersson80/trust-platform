"""Semantic and at-rest validation for ignored-test inventory reports."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from collections import Counter
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from .ignored_test_models import (
    GENERATOR,
    GENERATOR_VERSION,
    SCOPE,
    diagnostic_sort_key,
    record_sort_key,
    render_markdown,
)
from .ignored_test_report import LIMITATIONS, SURFACE_NOTES
from .ignored_test_source_contract import is_modeled_source_path
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest, stable_discovery_id
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import (
    check_supported_schema_keywords,
    is_safe_relative_path,
)


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
DISCOVERY_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
COMMIT_PATTERN = r"^[0-9a-f]{40}$"
DIGEST_PATTERN = r"^sha256:[0-9a-f]{64}$"
DISCOVERY_PATTERN = r"^DISC_[A-F0-9]{20}$"
DATED_EVIDENCE_RE = re.compile(
    r"^docs/internal/testing/evidence/plc-verification-program/\d{4}-\d{2}-\d{2}/[^/]+\.md$"
)
SOURCE_KINDS = {
    "rust_integration_test",
    "rust_unit_test",
    "vscode_test",
    "playwright_test",
}
IGNORE_STATES = {"ignored", "conditional"}
IGNORE_MECHANISMS = {
    "rust_attribute",
    "rust_cfg_attr",
    "vscode_runtime_skip",
    "playwright_literal_skip",
}
SEVERITIES = {"warning", "error"}
SURFACES = {"rust", "node", "playwright", "shell", "conformance"}
SURFACE_COVERAGE = {"mechanical", "limitation"}
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
    "records",
    "diagnostics",
    "surface_summary",
    "limitations",
    "summary",
}
FACT_FIELDS = {
    "discovery_id",
    "native_id",
    "discovery_source_kind",
    "name",
    "path",
    "line",
    "package",
    "command_hint",
    "ignore_state",
    "ignore_mechanism",
    "ignore_reason",
    "reference_candidates",
}
DIAGNOSTIC_FIELDS = {"severity", "kind", "path", "line", "message"}
SURFACE_FIELDS = {
    "surface",
    "scanned_files",
    "records",
    "ignored",
    "conditional",
    "coverage",
    "note",
}
SUMMARY_FIELDS = {
    "records",
    "ignored",
    "conditional",
    "diagnostics",
    "errors",
    "warnings",
    "by_source_kind",
}
SCHEMA_OBJECT_FIELDS = {
    "output_paths": {"json", "markdown"},
    "scope": set(SCOPE),
    "ignored_test_fact": FACT_FIELDS,
    "diagnostic": DIAGNOSTIC_FIELDS,
    "surface": SURFACE_FIELDS,
    "source_count": {"source_kind", "records"},
    "summary": SUMMARY_FIELDS,
}


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_records: list[dict[str, Any]] | None = None,
    expected_diagnostics: list[dict[str, Any]] | None = None,
    expected_surfaces: list[dict[str, Any]] | None = None,
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
    if not COMMIT_RE.fullmatch(str(payload.get("commit", ""))):
        failures.append("commit must be a clean full Git SHA")
    if not DIGEST_RE.fullmatch(str(payload.get("input_digest", ""))):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _is_iso_timestamp(payload.get("timestamp")):
        failures.append("timestamp must be an ISO-8601 value with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")

    inputs = payload.get("input_paths")
    if not isinstance(inputs, list) or not all(isinstance(item, str) for item in inputs):
        failures.append("input_paths must be a string array")
    elif inputs != sorted(set(inputs)):
        failures.append("input_paths must be canonical, sorted, and duplicate-free")
    elif any(not is_safe_relative_path(item) for item in inputs):
        failures.append("input_paths must be normalized workspace-relative paths")
    _validate_output_paths(payload.get("output_paths"), failures)
    _validate_command(payload, failures)
    if payload.get("scope") != SCOPE:
        failures.append("scope does not match the ignored-test inventory contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the ignored-test inventory contract")

    records = payload.get("records")
    valid_records: list[Mapping[str, Any]] = []
    if not isinstance(records, list):
        failures.append("records must be an array")
    else:
        valid_records = [item for item in records if isinstance(item, Mapping)]
        if len(valid_records) != len(records):
            failures.append("records must contain only objects")
        for index, record in enumerate(valid_records):
            _validate_fact(record, index, failures)
        if records != sorted(valid_records, key=record_sort_key):
            failures.append("records must use canonical source/path/name/id ordering")
        ids = [str(item.get("discovery_id")) for item in valid_records]
        if len(ids) != len(set(ids)):
            failures.append("records must have unique discovery_id values")
    diagnostics = payload.get("diagnostics")
    valid_diagnostics: list[Mapping[str, Any]] = []
    if not isinstance(diagnostics, list):
        failures.append("diagnostics must be an array")
    else:
        valid_diagnostics = [item for item in diagnostics if isinstance(item, Mapping)]
        if len(valid_diagnostics) != len(diagnostics):
            failures.append("diagnostics must contain only objects")
        for index, item in enumerate(valid_diagnostics):
            _validate_diagnostic(item, index, failures)
        if diagnostics != sorted(valid_diagnostics, key=diagnostic_sort_key):
            failures.append("diagnostics must use canonical ordering")
    surfaces = payload.get("surface_summary")
    valid_surfaces: list[Mapping[str, Any]] = []
    if not isinstance(surfaces, list):
        failures.append("surface_summary must be an array")
    else:
        valid_surfaces = [item for item in surfaces if isinstance(item, Mapping)]
        if len(valid_surfaces) != len(surfaces):
            failures.append("surface_summary must contain only objects")
        for index, item in enumerate(valid_surfaces):
            _validate_surface(item, index, failures)
        if [item.get("surface") for item in valid_surfaces] != sorted(SURFACES):
            failures.append("surface_summary must contain every surface in canonical order")
        _validate_surface_counts(valid_records, valid_surfaces, failures)

    expected_summary = _derive_summary(valid_records, valid_diagnostics)
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
    else:
        _check_exact_fields(summary, SUMMARY_FIELDS, "summary", failures)
        if dict(summary) != expected_summary:
            failures.append("summary does not match the report records and diagnostics")
    if expected_summary["errors"]:
        failures.append("complete ignored-test inventory cannot contain error diagnostics")
    if expected_records is not None and records != expected_records:
        failures.append("records do not match live ignored-test discovery")
    if expected_diagnostics is not None and diagnostics != expected_diagnostics:
        failures.append("diagnostics do not match live ignored-test discovery")
    if expected_surfaces is not None and surfaces != expected_surfaces:
        failures.append("surface_summary does not match live ignored-test discovery")
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    digest = hashlib.sha256(json_bytes).hexdigest()
    try:
        expected = render_markdown(payload, json_digest=digest)
    except (KeyError, TypeError, ValueError):
        return ["ignored-test inventory Markdown cannot be reconstructed from JSON"]
    failures: list[str] = []
    if markdown != expected:
        failures.append("ignored-test inventory Markdown does not exactly match JSON")
    if f"Generated JSON SHA-256: `{digest}`" not in markdown:
        failures.append("ignored-test inventory Markdown has a stale JSON digest")
    return failures


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("report schema root must be a closed object")
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
            failures.append(f"report schema const for {field} drifts")
    if properties.get("commit", {}).get("pattern") != COMMIT_PATTERN:
        failures.append("report schema commit pattern drifts")
    if properties.get("input_digest", {}).get("pattern") != DIGEST_PATTERN:
        failures.append("report schema input_digest pattern drifts")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        failures.append("report schema must define closed object contracts")
    else:
        for name, fields in SCHEMA_OBJECT_FIELDS.items():
            definition = definitions.get(name)
            if not isinstance(definition, Mapping):
                failures.append(f"report schema lacks {name} definition")
                continue
            if definition.get("additionalProperties") is not False:
                failures.append(f"report schema {name} must be closed")
            if set(definition.get("required", [])) != fields:
                failures.append(f"report schema {name} required fields drift")
            if set(definition.get("properties", {})) != fields:
                failures.append(f"report schema {name} properties drift")
        fact_properties = definitions.get("ignored_test_fact", {}).get("properties", {})
        if set(fact_properties.get("discovery_source_kind", {}).get("enum", [])) != SOURCE_KINDS:
            failures.append("report schema discovery_source_kind enum drifts")
        if set(fact_properties.get("ignore_state", {}).get("enum", [])) != IGNORE_STATES:
            failures.append("report schema ignore_state enum drifts")
        if set(fact_properties.get("ignore_mechanism", {}).get("enum", [])) != IGNORE_MECHANISMS:
            failures.append("report schema ignore_mechanism enum drifts")
        if fact_properties.get("discovery_id", {}).get("pattern") != DISCOVERY_PATTERN:
            failures.append("report schema discovery_id pattern drifts")
        diagnostic_properties = definitions.get("diagnostic", {}).get("properties", {})
        if set(diagnostic_properties.get("severity", {}).get("enum", [])) != SEVERITIES:
            failures.append("report schema diagnostic severity enum drifts")
        surface_properties = definitions.get("surface", {}).get("properties", {})
        if set(surface_properties.get("surface", {}).get("enum", [])) != SURFACES:
            failures.append("report schema surface enum drifts")
        if set(surface_properties.get("coverage", {}).get("enum", [])) != SURFACE_COVERAGE:
            failures.append("report schema surface coverage enum drifts")
        source_count_properties = definitions.get("source_count", {}).get(
            "properties", {}
        )
        if set(
            source_count_properties.get("source_kind", {}).get("enum", [])
        ) != SOURCE_KINDS:
            failures.append("report schema source-count kind enum drifts")
        scope_properties = definitions.get("scope", {}).get("properties", {})
        for field, expected in SCOPE.items():
            if scope_properties.get(field, {}).get("const") != expected:
                failures.append(f"report schema scope const for {field} drifts")
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
    try:
        json_bytes = json_file.read_bytes()
        payload = json.loads(json_bytes)
        markdown = markdown_file.read_text()
        schema = json.loads(schema_file.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [f"ignored-test inventory files cannot be read: {exc}"]
    if not isinstance(payload, Mapping) or not isinstance(schema, Mapping):
        return ["ignored-test inventory JSON and schema roots must be objects"]
    failures: list[str] = []
    canonical = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("ignored-test inventory JSON must use canonical serialization")
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(payload, dict(schema)))
    failures.extend(validate_report_payload(payload))
    outputs = payload.get("output_paths")
    if isinstance(outputs, Mapping):
        if _relative(root, json_file) != outputs.get("json"):
            failures.append("output_paths.json does not identify the validated JSON file")
        if _relative(root, markdown_file) != outputs.get("markdown"):
            failures.append("output_paths.markdown does not identify the validated Markdown file")
    try:
        from .ignored_test_live import build_live_inventory_state

        state = build_live_inventory_state(
            root,
            timestamp=payload.get("timestamp") if isinstance(payload.get("timestamp"), str) else None,
        )
    except (OSError, ValueError) as exc:
        failures.append(f"live ignored-test inventory failed: {exc}")
    else:
        failures.extend(
            validate_report_payload(
                payload,
                expected_records=[item.to_dict() for item in state.analysis.records],
                expected_diagnostics=[item.to_dict() for item in state.analysis.diagnostics],
                expected_surfaces=[dict(item) for item in state.analysis.surface_summary],
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


def _validate_fact(record: Mapping[str, Any], index: int, failures: list[str]) -> None:
    label = f"records[{index}]"
    _check_exact_fields(record, FACT_FIELDS, label, failures)
    discovery_id = record.get("discovery_id")
    source_kind = record.get("discovery_source_kind")
    package = record.get("package")
    native_id = record.get("native_id")
    if not isinstance(discovery_id, str) or not DISCOVERY_RE.fullmatch(discovery_id):
        failures.append(f"{label}.discovery_id is invalid")
    if source_kind not in SOURCE_KINDS:
        failures.append(f"{label}.discovery_source_kind is unsupported")
    if not isinstance(native_id, str) or not native_id:
        failures.append(f"{label}.native_id must be non-empty")
    elif source_kind in SOURCE_KINDS and (package is None or isinstance(package, str)):
        expected = stable_discovery_id(
            source_kind=str(source_kind), package=package, native_id=native_id
        )
        if discovery_id != expected:
            failures.append(f"{label}.discovery_id does not match its semantic identity")
    for field in ("name", "path", "command_hint", "ignore_reason"):
        if not isinstance(record.get(field), str) or not record[field]:
            failures.append(f"{label}.{field} must be non-empty")
    path = record.get("path")
    if isinstance(path, str) and not is_safe_relative_path(path):
        failures.append(f"{label}.path must be normalized and workspace-relative")
    if source_kind in {"rust_integration_test", "rust_unit_test"} and not str(path).startswith("crates/"):
        failures.append(f"{label}.path is outside the Rust scan surface")
    if source_kind == "vscode_test" and not str(path).startswith("editors/vscode/src/test/"):
        failures.append(f"{label}.path is outside the VS Code scan surface")
    if source_kind == "playwright_test" and not str(path).startswith("scripts/captures/"):
        failures.append(f"{label}.path is outside the Playwright capture surface")
    if not isinstance(record.get("line"), int) or isinstance(record.get("line"), bool) or record["line"] < 1:
        failures.append(f"{label}.line must be a positive integer")
    state = record.get("ignore_state")
    mechanism = record.get("ignore_mechanism")
    if state not in IGNORE_STATES:
        failures.append(f"{label}.ignore_state is unsupported")
    if mechanism not in IGNORE_MECHANISMS:
        failures.append(f"{label}.ignore_mechanism is unsupported")
    expected_pairs = {
        "rust_attribute": ("ignored", {"rust_integration_test", "rust_unit_test"}),
        "rust_cfg_attr": ("conditional", {"rust_integration_test", "rust_unit_test"}),
        "vscode_runtime_skip": ("conditional", {"vscode_test"}),
        "playwright_literal_skip": ("ignored", {"playwright_test"}),
    }
    if mechanism in expected_pairs:
        expected_state, expected_kinds = expected_pairs[mechanism]
        if state != expected_state or source_kind not in expected_kinds:
            failures.append(f"{label}.ignore mechanism/state/source binding is inconsistent")
    if source_kind == "playwright_test":
        if package != "trust-doc-captures":
            failures.append(f"{label}.package must bind the tracked Playwright package")
        expected_command = "cd scripts/captures && npx playwright test " + str(path).removeprefix(
            "scripts/captures/"
        )
        if record.get("command_hint") != expected_command:
            failures.append(f"{label}.command_hint must be package-root-aware")
    elif not isinstance(package, str) or not package:
        failures.append(f"{label}.package must be non-empty")
    references = record.get("reference_candidates")
    if not isinstance(references, list) or not all(isinstance(item, str) and item for item in references):
        failures.append(f"{label}.reference_candidates must be a string array")
    elif references != sorted(set(references)):
        failures.append(f"{label}.reference_candidates must be sorted and unique")


def _validate_diagnostic(item: Mapping[str, Any], index: int, failures: list[str]) -> None:
    label = f"diagnostics[{index}]"
    _check_exact_fields(item, DIAGNOSTIC_FIELDS, label, failures)
    if item.get("severity") not in SEVERITIES:
        failures.append(f"{label}.severity is unsupported")
    for field in ("kind", "path", "message"):
        if not isinstance(item.get(field), str) or not item[field]:
            failures.append(f"{label}.{field} must be non-empty")
    if isinstance(item.get("path"), str) and item["path"] != "<generated>" and not is_safe_relative_path(item["path"]):
        failures.append(f"{label}.path must be workspace-relative")
    if not isinstance(item.get("line"), int) or isinstance(item.get("line"), bool) or item["line"] < 1:
        failures.append(f"{label}.line must be positive")


def _validate_surface(item: Mapping[str, Any], index: int, failures: list[str]) -> None:
    label = f"surface_summary[{index}]"
    _check_exact_fields(item, SURFACE_FIELDS, label, failures)
    surface = item.get("surface")
    if surface not in SURFACES:
        failures.append(f"{label}.surface is unsupported")
        return
    for field in ("scanned_files", "records", "ignored", "conditional"):
        value = item.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            failures.append(f"{label}.{field} must be a non-negative integer")
    expected_coverage = "limitation" if surface in {"shell", "conformance"} else "mechanical"
    if item.get("coverage") != expected_coverage:
        failures.append(f"{label}.coverage must equal {expected_coverage}")
    if item.get("note") != SURFACE_NOTES[surface]:
        failures.append(f"{label}.note drifts from the surface contract")


def _validate_surface_counts(
    records: list[Mapping[str, Any]], surfaces: list[Mapping[str, Any]], failures: list[str]
) -> None:
    expected: dict[str, list[Mapping[str, Any]]] = {
        "rust": [row for row in records if str(row.get("discovery_source_kind", "")).startswith("rust_")],
        "node": [row for row in records if row.get("discovery_source_kind") == "vscode_test"],
        "playwright": [row for row in records if row.get("discovery_source_kind") == "playwright_test"],
        "shell": [],
        "conformance": [],
    }
    for item in surfaces:
        surface = item.get("surface")
        if surface not in expected:
            continue
        rows = expected[surface]
        counts = {
            "records": len(rows),
            "ignored": sum(row.get("ignore_state") == "ignored" for row in rows),
            "conditional": sum(row.get("ignore_state") == "conditional" for row in rows),
        }
        for field, value in counts.items():
            if item.get(field) != value:
                failures.append(f"surface {surface}.{field} does not match inventory records")


def _derive_summary(
    records: list[Mapping[str, Any]], diagnostics: list[Mapping[str, Any]]
) -> dict[str, Any]:
    by_kind = Counter(str(item.get("discovery_source_kind")) for item in records)
    return {
        "records": len(records),
        "ignored": sum(item.get("ignore_state") == "ignored" for item in records),
        "conditional": sum(item.get("ignore_state") == "conditional" for item in records),
        "diagnostics": len(diagnostics),
        "errors": sum(item.get("severity") == "error" for item in diagnostics),
        "warnings": sum(item.get("severity") == "warning" for item in diagnostics),
        "by_source_kind": [
            {"source_kind": source_kind, "records": count}
            for source_kind, count in sorted(by_kind.items())
        ],
    }


def _validate_output_paths(value: Any, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        failures.append("output_paths must be an object")
        return
    _check_exact_fields(value, {"json", "markdown"}, "output_paths", failures)
    json_path = value.get("json")
    markdown_path = value.get("markdown")
    if not isinstance(json_path, str) or not json_path.startswith("target/gate-artifacts/verification/") or not is_safe_relative_path(json_path):
        failures.append("output_paths.json must be a normalized verification gate-artifact path")
    if not isinstance(markdown_path, str) or not (
        markdown_path.startswith("target/gate-artifacts/verification/")
        or DATED_EVIDENCE_RE.fullmatch(markdown_path)
    ) or not is_safe_relative_path(markdown_path):
        failures.append("output_paths.markdown must be a normalized gate-artifact or dated evidence path")


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or not isinstance(payload.get("timestamp"), str):
        return
    expected = [
        "python3",
        "scripts/report_ignored_test_inventory.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        payload["timestamp"],
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical ignored-test inventory invocation")


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
    missing_modeled_sources = sorted(
        path
        for path in tree_paths
        if is_modeled_source_path(path) and path not in input_paths
    )
    if missing_modeled_sources:
        failures.append(
            "claimed source commit has modeled source paths absent from the current "
            "report closure: " + ", ".join(missing_modeled_sources[:5])
        )
    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", value, "--", *input_paths],
        check=False,
    )
    if diff.returncode == 1:
        failures.append("current report inputs differ from the clean source commit")
    elif diff.returncode != 0:
        failures.append(f"could not compare report inputs with source commit: exit {diff.returncode}")
    return failures


def _check_exact_fields(
    value: Mapping[str, Any], expected: set[str], label: str, failures: list[str]
) -> None:
    missing = sorted(expected - set(value))
    extra = sorted(set(value) - expected)
    if missing:
        failures.append(f"{label} missing fields: {', '.join(missing)}")
    if extra:
        failures.append(f"{label} has unexpected fields: {', '.join(extra)}")


def _require_closed_objects(value: Any, path: str, failures: list[str]) -> None:
    if isinstance(value, Mapping):
        if value.get("type") == "object" and value.get("additionalProperties") is not False:
            failures.append(f"{path} object schema must forbid additional properties")
        for key, child in value.items():
            _require_closed_objects(child, f"{path}.{key}", failures)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _require_closed_objects(child, f"{path}[{index}]", failures)


def _is_iso_timestamp(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative(root: Path, path: Path) -> str | None:
    try:
        return path.resolve().relative_to(root).as_posix()
    except (OSError, ValueError):
        return None
