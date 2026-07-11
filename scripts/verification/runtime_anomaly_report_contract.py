"""Semantic and schema contract for Phase 8 runtime-anomaly reports."""

from __future__ import annotations

import hashlib
import json
import re
from collections import Counter
from collections.abc import Mapping
from datetime import datetime
from typing import Any

from .runtime_anomaly_contract import (
    ASSOCIATION_KINDS,
    CLASS_IDS,
    DISCOVERY_SOURCE_KINDS,
    INJECTION_MECHANISMS,
    MAPPING_ID_RE,
    SUITE_IDS,
)
from .runtime_anomaly_mapping import MAPPING_STATES
from .runtime_anomaly_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    SCOPE,
    render_markdown,
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
    "boundaries",
    "spec_gap_reviews",
    "classes",
    "mappings",
    "gap_rows",
    "summary",
    "limitations",
}
CLASS_FIELDS = {
    "class_id",
    "title",
    "primary_suite",
    "conditional_suites",
    "state",
    "mapping_ids",
    "runnable_mapping_ids",
    "non_runnable_or_partial_mapping_ids",
}
MAPPING_FIELDS = {
    "mapping_id",
    "class_id",
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "association_kind",
    "injection_mechanism",
    "assertion_summary",
    "limitations",
    "last_reviewed",
    "primary_suite",
    "ignore_state",
    "ignored_registry_id",
    "effectively_runnable",
}
GAP_FIELDS = {"class_id", "title", "primary_suite", "state", "mapping_ids", "reason"}
SUMMARY_FIELDS = {
    "taxonomy_classes",
    "mapping_records",
    "scanner_denominator",
    "effectively_runnable_mappings",
    "ignored_or_conditional_mappings",
    "gap_classes",
    "by_state",
    "by_primary_suite",
    "by_association_kind",
}
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
REPORT_DISCOVERY_ID_PATTERN = r"^DISC_[0-9A-F]{20}$"
REPORT_DISCOVERY_ID_RE = re.compile(REPORT_DISCOVERY_ID_PATTERN)
TIMESTAMP_PATTERN = (
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})$"
)
TIMESTAMP_RE = re.compile(TIMESTAMP_PATTERN)


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    _closed_schema(schema, ROOT_FIELDS, "runtime-anomaly report schema", failures)
    properties = _properties(schema)
    for field, expected in {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
    }.items():
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly report schema const for {field} drifts")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        return sorted(set([*failures, "runtime-anomaly report schema definitions missing"]))
    for name, fields in (
        ("class", CLASS_FIELDS),
        ("mapping", MAPPING_FIELDS),
        ("gap", GAP_FIELDS),
        ("summary", SUMMARY_FIELDS),
    ):
        _closed_schema(
            _definition(definitions, name),
            fields,
            f"runtime-anomaly report {name} schema",
            failures,
        )
    for name, fields in (
        ("output_paths", {"json", "markdown"}),
        ("scope", set(SCOPE)),
        ("boundaries", set(BOUNDARIES)),
        ("spec_gap_reviews", {"scan_cycle_allocation_policy", "restart_timebase"}),
        (
            "allocation_review",
            {"outcome", "source_ref", "source_path", "required_text", "rationale"},
        ),
        ("restart_review", {"outcome", "spec_gap_ref", "rationale"}),
        ("count_map_state", set(MAPPING_STATES)),
        ("count_map_suite", set(SUITE_IDS)),
        ("count_map_association", set(ASSOCIATION_KINDS)),
    ):
        _closed_schema(
            _definition(definitions, name),
            fields,
            f"runtime-anomaly report {name}",
            failures,
        )
    scope_properties = _properties(_definition(definitions, "scope"))
    for field, expected in SCOPE.items():
        if scope_properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly report scope const for {field} drifts")
    boundary_properties = _properties(_definition(definitions, "boundaries"))
    for field, expected in BOUNDARIES.items():
        if boundary_properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly report boundary const for {field} drifts")
    if _definition(definitions, "digest").get("pattern") != DIGEST_RE.pattern:
        failures.append("runtime-anomaly report digest pattern drifts")
    timestamp_schema = properties.get("timestamp", {})
    if (
        timestamp_schema.get("type") != "string"
        or timestamp_schema.get("pattern") != TIMESTAMP_PATTERN
    ):
        failures.append("runtime-anomaly report timestamp pattern drifts")
    if properties.get("classes", {}).get("minItems") != len(CLASS_IDS):
        failures.append("runtime-anomaly report class minimum drifts")
    if properties.get("classes", {}).get("maxItems") != len(CLASS_IDS):
        failures.append("runtime-anomaly report class maximum drifts")
    class_schema = _definition(definitions, "class")
    mapping_schema = _definition(definitions, "mapping")
    gap_schema = _definition(definitions, "gap")
    class_properties = _properties(class_schema)
    mapping_properties = _properties(mapping_schema)
    gap_properties = _properties(gap_schema)
    _enum(schema, class_properties.get("class_id"), CLASS_IDS, "class_id", failures)
    _enum(schema, class_properties.get("primary_suite"), SUITE_IDS, "primary_suite", failures)
    _enum(schema, class_properties.get("state"), MAPPING_STATES, "state", failures)
    conditional = _resolve(schema, class_properties.get("conditional_suites", {}))
    conditional_items = _resolve(schema, conditional.get("items", {}))
    if set(conditional_items.get("enum", [])) != set(SUITE_IDS):
        failures.append("runtime-anomaly report conditional suite enum drifts")
    _enum(schema, mapping_properties.get("class_id"), CLASS_IDS, "mapping class_id", failures)
    _enum(
        schema,
        mapping_properties.get("discovery_source_kind"),
        DISCOVERY_SOURCE_KINDS,
        "mapping discovery_source_kind",
        failures,
    )
    _enum(
        schema,
        mapping_properties.get("association_kind"),
        ASSOCIATION_KINDS,
        "mapping association_kind",
        failures,
    )
    _enum(
        schema,
        mapping_properties.get("injection_mechanism"),
        INJECTION_MECHANISMS,
        "mapping injection_mechanism",
        failures,
    )
    _enum(schema, mapping_properties.get("primary_suite"), SUITE_IDS, "mapping suite", failures)
    if mapping_properties.get("mapping_id", {}).get("pattern") != MAPPING_ID_RE.pattern:
        failures.append("runtime-anomaly report mapping ID pattern drifts")
    if (
        mapping_properties.get("discovery_id", {}).get("pattern")
        != REPORT_DISCOVERY_ID_PATTERN
    ):
        failures.append("runtime-anomaly report discovery ID pattern drifts")
    if set(mapping_properties.get("ignore_state", {}).get("enum", [])) != {
        "not_ignored",
        "ignored",
        "conditional",
    }:
        failures.append("runtime-anomaly report ignore_state enum drifts")
    _enum(schema, gap_properties.get("class_id"), CLASS_IDS, "gap class_id", failures)
    _enum(schema, gap_properties.get("state"), MAPPING_STATES, "gap state", failures)
    for property_name, definition_name in (
        ("classes", "class"),
        ("mappings", "mapping"),
        ("gap_rows", "gap"),
    ):
        if properties.get(property_name, {}).get("items", {}).get("$ref") != (
            f"#/$defs/{definition_name}"
        ):
            failures.append(
                f"runtime-anomaly report {property_name} item binding drifts"
            )
    summary_properties = _properties(_definition(definitions, "summary"))
    for field, definition_name in (
        ("by_state", "count_map_state"),
        ("by_primary_suite", "count_map_suite"),
        ("by_association_kind", "count_map_association"),
    ):
        if summary_properties.get(field, {}).get("$ref") != f"#/$defs/{definition_name}":
            failures.append(f"runtime-anomaly report summary {field} binding drifts")
    review_properties = _properties(_definition(definitions, "spec_gap_reviews"))
    if review_properties.get("scan_cycle_allocation_policy", {}).get("$ref") != (
        "#/$defs/allocation_review"
    ):
        failures.append("runtime-anomaly report allocation review binding drifts")
    if review_properties.get("restart_timebase", {}).get("$ref") != (
        "#/$defs/restart_review"
    ):
        failures.append("runtime-anomaly report restart review binding drifts")
    allocation_properties = _properties(_definition(definitions, "allocation_review"))
    for field, expected in {
        "outcome": "written_contract_present",
        "source_ref": "SPEC_RUNTIME_ENGINE_001",
        "source_path": "docs/specs/11-runtime-engine.md",
        "required_text": [
            "dynamic allocation in hot path",
            "No heap allocation during execution",
        ],
    }.items():
        if allocation_properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly report allocation const for {field} drifts")
    restart_properties = _properties(_definition(definitions, "restart_review"))
    for field, expected in {
        "outcome": "existing_open_gap",
        "spec_gap_ref": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
    }.items():
        if restart_properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly report restart const for {field} drifts")
    return sorted(set(failures))


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_state: Any | None = None,
) -> list[str]:
    failures: list[str] = []
    if set(payload) != ROOT_FIELDS:
        failures.append("runtime-anomaly report root fields drift from contract")
    for field, expected in {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
        "scope": SCOPE,
        "boundaries": BOUNDARIES,
        "limitations": list(LIMITATIONS),
    }.items():
        if payload.get(field) != expected:
            failures.append(f"runtime-anomaly report {field} drifts from contract")
    _validate_provenance(payload, failures)
    reviews = _validate_spec_gap_reviews(payload.get("spec_gap_reviews"), failures)
    mappings = _validate_mappings(payload.get("mappings"), failures)
    classes = _validate_classes(payload.get("classes"), mappings, failures)
    gaps = _validate_gaps(payload.get("gap_rows"), classes, failures)
    _validate_summary(payload.get("summary"), classes, mappings, gaps, failures)
    if expected_state is not None:
        expected_analysis = expected_state.analysis
        if (
            reviews != expected_state.spec_gap_reviews
            or classes != expected_analysis["classes"]
            or mappings != expected_analysis["mappings"]
            or gaps != expected_analysis["gap_rows"]
            or payload.get("summary") != expected_analysis["summary"]
        ):
            failures.append("report rows do not match current runtime-anomaly analysis")
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    failures: list[str] = []
    canonical = json.dumps(payload, indent=2, sort_keys=True).encode() + b"\n"
    if json_bytes != canonical:
        failures.append("runtime-anomaly report JSON is not canonical")
    digest = hashlib.sha256(json_bytes).hexdigest()
    try:
        expected = render_markdown(payload, json_digest=digest)
    except Exception:
        failures.append(
            "runtime-anomaly Markdown cannot be rendered from invalid report payload"
        )
        return failures
    if markdown != expected:
        failures.append("runtime-anomaly Markdown does not exactly match report JSON")
    return failures


def _validate_provenance(payload: Mapping[str, Any], failures: list[str]) -> None:
    digest = payload.get("input_digest")
    if not isinstance(digest, str) or not DIGEST_RE.fullmatch(digest):
        failures.append("runtime-anomaly input_digest must be sha256:<64 hex>")
    commit = payload.get("commit")
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        failures.append("runtime-anomaly commit must identify a clean full Git SHA")
    timestamp = payload.get("timestamp")
    if not _is_iso_timestamp(timestamp):
        failures.append("runtime-anomaly timestamp must be ISO-8601 with a timezone")
    platform = payload.get("platform")
    if not isinstance(platform, str) or not platform:
        failures.append("runtime-anomaly platform must be non-empty")
    paths = payload.get("input_paths")
    if (
        not isinstance(paths, list)
        or not paths
        or not all(isinstance(path, str) and path for path in paths)
        or paths != sorted(set(paths))
    ):
        failures.append("runtime-anomaly input_paths must be sorted unique strings")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping) or set(outputs) != {"json", "markdown"}:
        failures.append("runtime-anomaly output_paths fields drift")
        outputs = {}
    command = payload.get("command")
    expected_command = [
        "python3",
        "scripts/report_runtime_anomaly_audit.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        payload.get("timestamp"),
    ]
    if command != expected_command:
        failures.append("runtime-anomaly command does not match canonical generator invocation")


def _validate_spec_gap_reviews(value: Any, failures: list[str]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        failures.append("runtime-anomaly spec_gap_reviews must be an object")
        return {}
    expected_keys = {"scan_cycle_allocation_policy", "restart_timebase"}
    if set(value) != expected_keys:
        failures.append("runtime-anomaly spec_gap_reviews fields drift")
    allocation = value.get("scan_cycle_allocation_policy")
    restart = value.get("restart_timebase")
    expected_allocation = {
        "outcome": "written_contract_present",
        "source_ref": "SPEC_RUNTIME_ENGINE_001",
        "source_path": "docs/specs/11-runtime-engine.md",
        "required_text": [
            "dynamic allocation in hot path",
            "No heap allocation during execution",
        ],
    }
    if not isinstance(allocation, Mapping):
        failures.append("scan_cycle_allocation_policy report review must be an object")
    else:
        if set(allocation) != {*expected_allocation, "rationale"}:
            failures.append("scan_cycle_allocation_policy report fields drift")
        for field, expected in expected_allocation.items():
            if allocation.get(field) != expected:
                failures.append(f"scan_cycle_allocation_policy report {field} drifts")
        if not _text(allocation.get("rationale")):
            failures.append("scan_cycle_allocation_policy report rationale must be non-empty")
    if not isinstance(restart, Mapping):
        failures.append("restart_timebase report review must be an object")
    else:
        if set(restart) != {"outcome", "spec_gap_ref", "rationale"}:
            failures.append("restart_timebase report fields drift")
        if restart.get("outcome") != "existing_open_gap":
            failures.append("restart_timebase report outcome drifts")
        if restart.get("spec_gap_ref") != "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001":
            failures.append("restart_timebase report spec_gap_ref drifts")
        if not _text(restart.get("rationale")):
            failures.append("restart_timebase report rationale must be non-empty")
    return {key: dict(item) for key, item in value.items() if isinstance(item, Mapping)}


def _validate_mappings(value: Any, failures: list[str]) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append("runtime-anomaly mappings must be an array")
        return []
    rows = [row for row in value if isinstance(row, dict)]
    if len(rows) != len(value):
        failures.append("runtime-anomaly mapping rows must be objects")
    ids = [row.get("mapping_id") for row in rows]
    discoveries = [row.get("discovery_id") for row in rows]
    valid_ids = [
        value
        for value in ids
        if isinstance(value, str) and MAPPING_ID_RE.fullmatch(value)
    ]
    if len(valid_ids) != len(ids):
        failures.append("runtime-anomaly mapping_id values must match the closed ID pattern")
    elif ids != sorted(valid_ids) or len(valid_ids) != len(set(valid_ids)):
        failures.append("runtime-anomaly mapping IDs must be unique canonical order")
    valid_discoveries = [
        value
        for value in discoveries
        if isinstance(value, str) and REPORT_DISCOVERY_ID_RE.fullmatch(value)
    ]
    if len(valid_discoveries) != len(discoveries):
        failures.append(
            "runtime-anomaly discovery_id values must match the closed ID pattern"
        )
    elif len(valid_discoveries) != len(set(valid_discoveries)):
        failures.append("runtime-anomaly mapping discovery IDs must be unique")
    for row in rows:
        label = row.get("mapping_id", "<unknown>")
        if set(row) != MAPPING_FIELDS:
            failures.append(f"runtime-anomaly mapping {label} fields drift")
        if row.get("class_id") not in CLASS_IDS:
            failures.append(f"runtime-anomaly mapping {label} has unknown class")
        if row.get("discovery_source_kind") not in DISCOVERY_SOURCE_KINDS:
            failures.append(f"runtime-anomaly mapping {label} has unknown source kind")
        if row.get("association_kind") not in ASSOCIATION_KINDS:
            failures.append(f"runtime-anomaly mapping {label} has unknown association kind")
        if row.get("injection_mechanism") not in INJECTION_MECHANISMS:
            failures.append(f"runtime-anomaly mapping {label} has unknown mechanism")
        if row.get("primary_suite") not in SUITE_IDS:
            failures.append(f"runtime-anomaly mapping {label} has unknown suite")
        for field in ("path", "name", "assertion_summary", "last_reviewed"):
            if not _text(row.get(field)):
                failures.append(f"runtime-anomaly mapping {label} requires {field}")
        limitations = row.get("limitations")
        if not isinstance(limitations, list) or not limitations or not all(
            _text(item) for item in limitations
        ):
            failures.append(f"runtime-anomaly mapping {label} limitations must be non-empty")
        ignore_state = row.get("ignore_state")
        if ignore_state not in ("not_ignored", "ignored", "conditional"):
            failures.append(f"runtime-anomaly mapping {label} has unknown ignore_state")
        expected_runnable = (
            row.get("association_kind") == "direct" and ignore_state == "not_ignored"
        )
        if row.get("effectively_runnable") is not expected_runnable:
            failures.append(f"runtime-anomaly mapping {label} runnability is inconsistent")
        registry_id = row.get("ignored_registry_id")
        if (ignore_state == "not_ignored") != (registry_id is None):
            failures.append(f"runtime-anomaly mapping {label} ignored registry binding drifts")
    return rows


def _validate_classes(
    value: Any,
    mappings: list[dict[str, Any]],
    failures: list[str],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append("runtime-anomaly classes must be an array")
        return []
    rows = [row for row in value if isinstance(row, dict)]
    if len(rows) != len(value):
        failures.append("runtime-anomaly class rows must be objects")
    if [row.get("class_id") for row in rows] != list(CLASS_IDS):
        failures.append("runtime-anomaly report classes must use exact taxonomy order")
    mappings_by_class: dict[str, list[dict[str, Any]]] = {
        class_id: [row for row in mappings if row.get("class_id") == class_id]
        for class_id in CLASS_IDS
    }
    for row in rows:
        class_id = row.get("class_id")
        if set(row) != CLASS_FIELDS:
            failures.append(f"runtime-anomaly class {class_id} fields drift")
        if row.get("primary_suite") not in SUITE_IDS:
            failures.append(f"runtime-anomaly class {class_id} has unknown suite")
        conditional = row.get("conditional_suites")
        if not isinstance(conditional, list) or any(item not in SUITE_IDS for item in conditional):
            failures.append(f"runtime-anomaly class {class_id} conditional suites drift")
        members = mappings_by_class.get(str(class_id), [])
        expected_ids = [member.get("mapping_id") for member in members]
        runnable = [
            member.get("mapping_id")
            for member in members
            if member.get("effectively_runnable") is True
        ]
        other = [
            member.get("mapping_id")
            for member in members
            if member.get("effectively_runnable") is not True
        ]
        expected_state = (
            "mapped_runnable"
            if runnable
            else "mapped_non_runnable_or_partial"
            if members
            else "unmapped"
        )
        for field, expected in {
            "mapping_ids": expected_ids,
            "runnable_mapping_ids": runnable,
            "non_runnable_or_partial_mapping_ids": other,
            "state": expected_state,
        }.items():
            if row.get(field) != expected:
                failures.append(f"runtime-anomaly class {class_id} {field} is inconsistent")
    return rows


def _validate_gaps(
    value: Any,
    classes: list[dict[str, Any]],
    failures: list[str],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append("runtime-anomaly gap_rows must be an array")
        return []
    rows = [row for row in value if isinstance(row, dict)]
    expected_classes = [row for row in classes if row.get("state") != "mapped_runnable"]
    if [row.get("class_id") for row in rows] != [
        row.get("class_id") for row in expected_classes
    ]:
        failures.append("runtime-anomaly gap rows are not the exhaustive class partition")
    expected_by_id: dict[str, dict[str, Any]] = {}
    for source in expected_classes:
        source_class_id = source.get("class_id")
        if isinstance(source_class_id, str):
            expected_by_id[source_class_id] = source
    for row in rows:
        class_id = row.get("class_id")
        if set(row) != GAP_FIELDS:
            failures.append(f"runtime-anomaly gap {class_id} fields drift")
        if not isinstance(class_id, str):
            failures.append("runtime-anomaly gap class_id must be a string")
            continue
        source = expected_by_id.get(class_id)
        if source is None:
            continue
        expected = {
            "title": source.get("title"),
            "primary_suite": source.get("primary_suite"),
            "state": source.get("state"),
            "mapping_ids": source.get("mapping_ids"),
            "reason": (
                "no_explicit_mapping"
                if source.get("state") == "unmapped"
                else "no_effectively_runnable_direct_mapping"
            ),
        }
        for field, item in expected.items():
            if row.get(field) != item:
                failures.append(f"runtime-anomaly gap {class_id} {field} is inconsistent")
    return rows


def _validate_summary(
    value: Any,
    classes: list[dict[str, Any]],
    mappings: list[dict[str, Any]],
    gaps: list[dict[str, Any]],
    failures: list[str],
) -> None:
    if not isinstance(value, Mapping):
        failures.append("runtime-anomaly summary must be an object")
        return
    if set(value) != SUMMARY_FIELDS:
        failures.append("runtime-anomaly summary fields drift")
    state_counts = Counter(
        row.get("state") for row in classes if isinstance(row.get("state"), str)
    )
    suite_counts = Counter(
        row.get("primary_suite")
        for row in classes
        if isinstance(row.get("primary_suite"), str)
    )
    association_counts = Counter(
        row.get("association_kind")
        for row in mappings
        if isinstance(row.get("association_kind"), str)
    )
    expected = {
        "taxonomy_classes": len(classes),
        "mapping_records": len(mappings),
        "effectively_runnable_mappings": sum(
            row.get("effectively_runnable") is True for row in mappings
        ),
        "ignored_or_conditional_mappings": sum(
            row.get("ignore_state") != "not_ignored" for row in mappings
        ),
        "gap_classes": len(gaps),
        "by_state": {state: state_counts[state] for state in MAPPING_STATES},
        "by_primary_suite": {suite: suite_counts[suite] for suite in SUITE_IDS},
        "by_association_kind": {
            kind: association_counts[kind] for kind in ASSOCIATION_KINDS
        },
    }
    for field, item in expected.items():
        if value.get(field) != item:
            failures.append(f"runtime-anomaly summary {field} is inconsistent")
    denominator = value.get("scanner_denominator")
    if not isinstance(denominator, int) or isinstance(denominator, bool) or denominator < 1:
        failures.append("runtime-anomaly summary scanner_denominator must be positive")


def _closed_schema(
    schema: Mapping[str, Any],
    fields: set[str],
    label: str,
    failures: list[str],
) -> None:
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append(f"{label} must be a closed object")
    if set(schema.get("required", [])) != fields:
        failures.append(f"{label} required fields drift")
    if set(_properties(schema)) != fields:
        failures.append(f"{label} properties drift")


def _enum(
    root: Mapping[str, Any],
    schema: Any,
    expected: tuple[str, ...],
    label: str,
    failures: list[str],
) -> None:
    resolved = _resolve(root, schema)
    if set(resolved.get("enum", [])) != set(expected):
        failures.append(f"runtime-anomaly report schema enum for {label} drifts")


def _resolve(root: Mapping[str, Any], schema: Any) -> Mapping[str, Any]:
    if not isinstance(schema, Mapping):
        return {}
    reference = schema.get("$ref")
    if not isinstance(reference, str) or not reference.startswith("#/"):
        return schema
    current: Any = root
    for part in reference[2:].split("/"):
        if not isinstance(current, Mapping) or part not in current:
            return {}
        current = current[part]
    return current if isinstance(current, Mapping) else {}


def _properties(schema: Mapping[str, Any]) -> Mapping[str, Any]:
    value = schema.get("properties")
    return value if isinstance(value, Mapping) else {}


def _definition(definitions: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    value = definitions.get(name)
    return value if isinstance(value, Mapping) else {}


def _text(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _is_iso_timestamp(value: Any) -> bool:
    if not isinstance(value, str) or TIMESTAMP_RE.fullmatch(value) is None:
        return False
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None and parsed.utcoffset() is not None
