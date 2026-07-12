"""Fail-closed contract for the reviewed Phase 8 runtime-anomaly taxonomy."""

from __future__ import annotations

import json
import re
import subprocess
import tomllib
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from .metadata_validator.constants import SOURCE_AUTHORITIES
from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS
from .runtime_anomaly_restart_contract import (
    RESTART_GAP_ID,
    validate_restart_review_shape,
    validate_restart_union_schema,
)
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


TAXONOMY_PATH = "verification/runtime-anomaly-taxonomy.toml"
TAXONOMY_SCHEMA_PATH = "verification/schemas/runtime-anomaly-taxonomy.schema.json"

CLASS_IDS = (
    "panic",
    "timeout",
    "deadline",
    "watchdog",
    "slow_device",
    "disconnect",
    "queue_full",
    "stale_data",
    "corrupt_retain",
    "malformed_bytecode",
    "bad_config",
    "bad_signal",
    "partial_web_request",
    "disk_error",
    "clock_step",
    "monotonic_wall_clock_divergence",
    "suspend_resume",
    "timer_duration_overflow",
    "allocation_failure_oom",
)
SUITE_IDS = ("pr", "nightly", "release", "hardware_lab")
INJECTION_BOUNDARIES = (
    "ordinary_input",
    "test_harness",
    "external_harness",
    "design_review_required",
)
ASSOCIATION_KINDS = ("direct", "partial", "protective_red", "context_only")
INJECTION_MECHANISMS = ("ordinary_input", "test_harness", "external_harness")
DISCOVERY_SOURCE_KINDS = ("rust_integration_test", "rust_unit_test")
RESTART_SOURCE_AUTHORITIES = SOURCE_AUTHORITIES - {"public_claim"}
ALLOCATION_REQUIRED_TEXT = (
    "dynamic allocation in hot path",
    "No heap allocation during execution",
)

ROOT_FIELDS = {
    "schema_version",
    "id",
    "title",
    "area",
    "mapping_basis",
    "proof_posture",
    "fault_interface_status",
    "production_hook_policy",
    "last_reviewed",
    "spec_gap_reviews",
    "classes",
    "mappings",
}
CLASS_FIELDS = {
    "id",
    "title",
    "stimulus",
    "primary_suite",
    "conditional_suites",
    "injection_boundary",
    "rationale",
}
MAPPING_FIELDS = {
    "id",
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
}
SPEC_REVIEW_FIELDS = {"scan_cycle_allocation_policy", "restart_timebase"}
ALLOCATION_REVIEW_FIELDS = {
    "outcome",
    "source_ref",
    "source_path",
    "required_text",
    "rationale",
}

ROOT_CONSTS = {
    "schema_version": 1,
    "id": "RUNTIME_ANOMALY_TAXONOMY_V1",
    "area": "runtime_safety",
    "mapping_basis": "explicit_reviewed_discovery_id_only",
    "proof_posture": "association_only",
    "fault_interface_status": "not_implemented",
    "production_hook_policy": "design_review_required",
}
DATE_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
MAPPING_ID_RE = re.compile(r"^ANOM_MAP_[A-Z0-9_]+$")
MAPPING_PATH_RE = re.compile(r"^crates/trust-runtime/.*\.rs$")
FORBIDDEN_CLAIM_RE = re.compile(
    r"\b(?:proof|prove|proves|proved|coverage|covered|validated)\b",
    re.IGNORECASE,
)


def load_runtime_anomaly_taxonomy(root: Path) -> dict[str, Any]:
    """Load the committed reviewed taxonomy."""

    return tomllib.loads((root / TAXONOMY_PATH).read_text())


def validate_runtime_anomaly_contract(
    root: Path,
    taxonomy: Mapping[str, Any],
    *,
    spec_sources: Mapping[str, Mapping[str, Any]] | None = None,
    spec_gaps: Mapping[str, Mapping[str, Any]] | None = None,
) -> list[str]:
    """Validate taxonomy shape, references, source bindings, and honesty posture."""

    failures: list[str] = []
    try:
        schema = json.loads((root / TAXONOMY_SCHEMA_PATH).read_text())
    except Exception as exc:
        return [f"runtime-anomaly taxonomy schema cannot be read: {exc}"]
    failures.extend(validate_runtime_anomaly_schema_contract(schema))
    failures.extend(validate_json_schema_instance(dict(taxonomy), schema))

    if set(taxonomy) != ROOT_FIELDS:
        failures.append("runtime-anomaly taxonomy root fields drift from contract")
    for field, expected in ROOT_CONSTS.items():
        if taxonomy.get(field) != expected:
            failures.append(f"runtime-anomaly taxonomy {field} must equal {expected!r}")
    _require_text(taxonomy.get("title"), "runtime-anomaly taxonomy title", failures)
    _require_date(taxonomy.get("last_reviewed"), "runtime-anomaly taxonomy", failures)

    sources = (
        dict(spec_sources)
        if spec_sources is not None
        else _load_records(root / "verification/spec-sources.toml", "spec_sources")
    )
    gaps = (
        dict(spec_gaps)
        if spec_gaps is not None
        else _load_records(root / "verification/spec-gaps.toml", "spec_gaps")
    )
    _validate_spec_reviews(root, taxonomy.get("spec_gap_reviews"), sources, gaps, failures)

    classes = taxonomy.get("classes")
    if not isinstance(classes, list):
        failures.append("runtime-anomaly classes must be an array")
        classes = []
    class_ids = [row.get("id") if isinstance(row, Mapping) else None for row in classes]
    if class_ids != list(CLASS_IDS):
        failures.append("runtime-anomaly classes must match the exact board order")
    for index, row in enumerate(classes):
        _validate_class(row, index, failures)

    mappings = taxonomy.get("mappings")
    if not isinstance(mappings, list) or not mappings:
        failures.append("runtime-anomaly mappings must be a non-empty array")
        mappings = []
    mapping_ids = [row.get("id") for row in mappings if isinstance(row, Mapping)]
    discovery_ids = [
        row.get("discovery_id") for row in mappings if isinstance(row, Mapping)
    ]
    if len(mapping_ids) != len(set(_hashable_strings(mapping_ids))):
        failures.append("runtime-anomaly taxonomy has duplicate mapping IDs")
    if len(discovery_ids) != len(set(_hashable_strings(discovery_ids))):
        failures.append("runtime-anomaly taxonomy has duplicate discovery IDs")
    known_classes = set(CLASS_IDS)
    for index, row in enumerate(mappings):
        _validate_mapping(root, row, index, known_classes, failures)
    return sorted(set(failures))


def validate_runtime_anomaly_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    """Drift-pin the closed schema to the Python contract vocabulary."""

    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    _closed_object_schema(schema, ROOT_FIELDS, "runtime-anomaly schema root", failures)
    properties = schema.get("properties")
    if not isinstance(properties, Mapping):
        properties = {}
    for field, expected in ROOT_CONSTS.items():
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly schema const for {field} drifts")
    if not _date_schema(properties.get("last_reviewed", {})):
        failures.append("runtime-anomaly schema last_reviewed pattern drifts")

    definitions = schema.get("$defs")
    required_definitions = {
        "class",
        "mapping",
        "allocation_review",
        "restart_timebase_review",
        "restart_existing_open_gap_v1",
        "restart_resolved_source_v1",
    }
    if not isinstance(definitions, Mapping) or not required_definitions.issubset(definitions):
        failures.append("runtime-anomaly schema definitions drift")
        definitions = {}
    class_schema = _definition(definitions, "class")
    mapping_schema = _definition(definitions, "mapping")
    allocation_schema = _definition(definitions, "allocation_review")
    restart_schema = _definition(definitions, "restart_timebase_review")
    _closed_object_schema(class_schema, CLASS_FIELDS, "runtime-anomaly class schema", failures)
    _closed_object_schema(
        mapping_schema,
        MAPPING_FIELDS,
        "runtime-anomaly mapping schema",
        failures,
    )
    _closed_object_schema(
        allocation_schema,
        ALLOCATION_REVIEW_FIELDS,
        "runtime-anomaly allocation review schema",
        failures,
    )
    failures.extend(
        validate_restart_union_schema(
            schema,
            restart_schema,
            definitions,
            label="runtime-anomaly restart review",
        )
    )

    class_properties = _properties(class_schema)
    mapping_properties = _properties(mapping_schema)
    class_id_schema = _resolve_schema(schema, class_properties.get("id", {}))
    if class_id_schema.get("enum") != list(CLASS_IDS):
        failures.append("runtime-anomaly class ID enum drifts")
    _enum_drift(schema, class_properties, "primary_suite", SUITE_IDS, failures)
    conditional_items = _resolve_schema(
        schema,
        class_properties.get("conditional_suites", {}).get("items", {}),
    )
    if set(conditional_items.get("enum", [])) != set(SUITE_IDS):
        failures.append("runtime-anomaly conditional_suites enum drifts")
    if class_properties.get("conditional_suites", {}).get("uniqueItems") is not True:
        failures.append("runtime-anomaly conditional_suites must be schema-unique")
    _enum_drift(
        schema,
        class_properties,
        "injection_boundary",
        INJECTION_BOUNDARIES,
        failures,
    )
    _enum_drift(
        schema,
        mapping_properties,
        "discovery_source_kind",
        DISCOVERY_SOURCE_KINDS,
        failures,
    )
    _enum_drift(
        schema,
        mapping_properties,
        "association_kind",
        ASSOCIATION_KINDS,
        failures,
    )
    _enum_drift(
        schema,
        mapping_properties,
        "injection_mechanism",
        INJECTION_MECHANISMS,
        failures,
    )
    class_ref_schema = _resolve_schema(schema, mapping_properties.get("class_id", {}))
    if class_ref_schema.get("enum") != list(CLASS_IDS):
        failures.append("runtime-anomaly mapping class_id enum drifts")
    if not _discovery_id_schema(mapping_properties.get("discovery_id", {})):
        failures.append("runtime-anomaly discovery_id pattern drifts")
    if mapping_properties.get("id", {}).get("pattern") != MAPPING_ID_RE.pattern:
        failures.append("runtime-anomaly mapping ID pattern drifts")
    if mapping_properties.get("path", {}).get("pattern") != MAPPING_PATH_RE.pattern:
        failures.append("runtime-anomaly mapping path pattern drifts")
    if not _date_schema(mapping_properties.get("last_reviewed", {})):
        failures.append("runtime-anomaly mapping last_reviewed pattern drifts")
    limitations_schema = _resolve_schema(schema, mapping_properties.get("limitations", {}))
    if (
        limitations_schema.get("type") != "array"
        or limitations_schema.get("minItems") != 1
        or limitations_schema.get("uniqueItems") is not True
        or limitations_schema.get("items", {}).get("type") != "string"
    ):
        failures.append("runtime-anomaly mapping limitations schema drifts")

    _validate_spec_review_schema(properties, allocation_schema, restart_schema, failures)
    classes_schema = properties.get("classes", {})
    if (
        classes_schema.get("minItems") != len(CLASS_IDS)
        or classes_schema.get("maxItems") != len(CLASS_IDS)
        or classes_schema.get("items", {}).get("$ref") != "#/$defs/class"
    ):
        failures.append("runtime-anomaly classes schema cardinality or item binding drifts")
    mappings_schema = properties.get("mappings", {})
    if (
        mappings_schema.get("minItems") != 1
        or mappings_schema.get("items", {}).get("$ref") != "#/$defs/mapping"
    ):
        failures.append("runtime-anomaly mappings schema item binding drifts")
    return sorted(set(failures))


def _validate_class(value: Any, index: int, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        failures.append(f"runtime-anomaly class {index} must be a table")
        return
    class_id = value.get("id")
    label = class_id if isinstance(class_id, str) else f"classes[{index}]"
    if set(value) != CLASS_FIELDS:
        failures.append(f"runtime-anomaly class {label} fields drift from contract")
    for field in ("title", "stimulus", "rationale"):
        _require_text(value.get(field), f"runtime-anomaly class {label} {field}", failures)
        _reject_claim_language(value.get(field), f"runtime-anomaly class {label} {field}", failures)
    primary = value.get("primary_suite")
    if primary not in SUITE_IDS:
        failures.append(f"runtime-anomaly class {label} has unknown primary_suite {primary!r}")
    conditional = value.get("conditional_suites")
    if not isinstance(conditional, list) or not all(item in SUITE_IDS for item in conditional):
        failures.append(f"runtime-anomaly class {label} conditional_suites use unknown suite")
    else:
        if len(conditional) != len(set(conditional)):
            failures.append(f"runtime-anomaly class {label} conditional_suites must be unique")
        if primary in conditional:
            failures.append(
                f"runtime-anomaly class {label} conditional_suites must exclude primary_suite"
            )
    boundary = value.get("injection_boundary")
    if boundary not in INJECTION_BOUNDARIES:
        failures.append(
            f"runtime-anomaly class {label} has unknown injection_boundary {boundary!r}"
        )


def _validate_mapping(
    root: Path,
    value: Any,
    index: int,
    known_classes: set[str],
    failures: list[str],
) -> None:
    if not isinstance(value, Mapping):
        failures.append(f"runtime-anomaly mapping {index} must be a table")
        return
    mapping_id = value.get("id")
    label = mapping_id if isinstance(mapping_id, str) and mapping_id else f"mappings[{index}]"
    if set(value) != MAPPING_FIELDS:
        failures.append(f"runtime-anomaly mapping {label} fields drift from contract")
    _require_text(mapping_id, f"runtime-anomaly mapping {label} id", failures)
    if not isinstance(mapping_id, str) or not MAPPING_ID_RE.fullmatch(mapping_id):
        failures.append(f"runtime-anomaly mapping {label} has invalid mapping ID")
    class_id = value.get("class_id")
    if not isinstance(class_id, str) or class_id not in known_classes:
        failures.append(f"runtime-anomaly mapping {label} has unknown class_id {class_id!r}")
    discovery_id = value.get("discovery_id")
    if not isinstance(discovery_id, str) or not DISCOVERY_ID_RE.fullmatch(discovery_id):
        failures.append(f"runtime-anomaly mapping {label} has invalid discovery_id")
    source_kind = value.get("discovery_source_kind")
    if source_kind not in DISCOVERY_SOURCE_KINDS:
        failures.append(
            f"runtime-anomaly mapping {label} has unknown discovery_source_kind {source_kind!r}"
        )
    path = value.get("path")
    _validate_durable_path(root, path, f"runtime-anomaly mapping {label} path", failures)
    if not isinstance(path, str) or not MAPPING_PATH_RE.fullmatch(path):
        failures.append(
            f"runtime-anomaly mapping {label} path must stay under crates/trust-runtime"
        )
    if isinstance(path, str) and is_safe_relative_path(path):
        _validate_source_kind_path(source_kind, path, label, failures)
    _require_text(value.get("name"), f"runtime-anomaly mapping {label} name", failures)
    association = value.get("association_kind")
    if association not in ASSOCIATION_KINDS:
        failures.append(
            f"runtime-anomaly mapping {label} has unknown association_kind {association!r}"
        )
    mechanism = value.get("injection_mechanism")
    if mechanism not in INJECTION_MECHANISMS:
        failures.append(
            f"runtime-anomaly mapping {label} has unknown injection_mechanism {mechanism!r}"
        )
    _require_text(
        value.get("assertion_summary"),
        f"runtime-anomaly mapping {label} assertion_summary",
        failures,
    )
    _reject_claim_language(
        value.get("assertion_summary"),
        f"runtime-anomaly mapping {label} assertion_summary",
        failures,
    )
    limitations = _require_string_array(
        value.get("limitations"),
        f"runtime-anomaly mapping {label} limitations",
        failures,
    )
    for item in limitations:
        _reject_claim_language(
            item,
            f"runtime-anomaly mapping {label} limitations",
            failures,
        )
    _require_date(value.get("last_reviewed"), f"runtime-anomaly mapping {label}", failures)


def _validate_spec_reviews(
    root: Path,
    value: Any,
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    if not isinstance(value, Mapping):
        failures.append("runtime-anomaly spec_gap_reviews must be an object")
        return
    if set(value) != SPEC_REVIEW_FIELDS:
        failures.append("runtime-anomaly spec_gap_reviews fields drift from contract")
    allocation = value.get("scan_cycle_allocation_policy")
    if not isinstance(allocation, Mapping):
        failures.append("scan_cycle_allocation_policy review must be an object")
        allocation = {}
    elif set(allocation) != ALLOCATION_REVIEW_FIELDS:
        failures.append("scan_cycle_allocation_policy review fields drift from contract")
    expected_allocation = {
        "outcome": "written_contract_present",
        "source_ref": "SPEC_RUNTIME_ENGINE_001",
        "source_path": "docs/specs/11-runtime-engine.md",
    }
    for field, expected in expected_allocation.items():
        if allocation.get(field) != expected:
            failures.append(f"scan_cycle_allocation_policy {field} must equal {expected!r}")
    source = spec_sources.get("SPEC_RUNTIME_ENGINE_001")
    if (
        source is None
        or source.get("area") != "runtime_safety"
        or source.get("path") != "docs/specs/11-runtime-engine.md"
        or source.get("source_status") != "active"
        or source.get("oracle_eligible") is not True
        or source.get("authority") == "public_claim"
    ):
        failures.append(
            "scan_cycle_allocation_policy requires an active oracle-eligible runtime source"
        )
    required_text = allocation.get("required_text")
    if required_text != list(ALLOCATION_REQUIRED_TEXT):
        failures.append(
            "scan_cycle_allocation_policy required_text must equal the reviewed phrases"
        )
    source_path = allocation.get("source_path")
    before = len(failures)
    _validate_durable_path(root, source_path, "scan_cycle_allocation_policy source_path", failures)
    if len(failures) == before and isinstance(source_path, str):
        text = (root / source_path).read_text()
        for phrase in ALLOCATION_REQUIRED_TEXT:
            if phrase not in text:
                failures.append(
                    f"scan_cycle_allocation_policy source is missing required text {phrase!r}"
                )
    _require_text(
        allocation.get("rationale"), "scan_cycle_allocation_policy rationale", failures
    )
    _reject_claim_language(
        allocation.get("rationale"), "scan_cycle_allocation_policy rationale", failures
    )

    restart = value.get("restart_timebase")
    failures.extend(validate_restart_review_shape(restart, label="restart_timebase"))
    if not isinstance(restart, Mapping):
        return
    gap = spec_gaps.get(RESTART_GAP_ID)
    outcome = restart.get("outcome")
    if gap is None or gap.get("status") != "spec_gap":
        failures.append("restart_timebase requires the known superseded spec gap")
    elif outcome == "existing_open_gap":
        if gap.get("resolution_status") not in OPEN_GAP_RESOLUTIONS:
            failures.append("restart_timebase existing_open_gap requires an open gap")
    elif outcome == "resolved_source":
        resolution_status = gap.get("resolution_status")
        if resolution_status == "closed":
            if gap.get("resolution_source_ref") != restart.get("source_ref"):
                failures.append(
                    "restart_timebase closed superseded gap must bind the "
                    "resolved_source source_ref"
                )
        elif resolution_status not in OPEN_GAP_RESOLUTIONS:
            failures.append(
                "restart_timebase resolved_source requires an open or closed superseded gap"
            )
    if outcome == "resolved_source":
        source_ref = restart.get("source_ref")
        source = spec_sources.get(source_ref) if isinstance(source_ref, str) else None
        if (
            source is None
            or source.get("source_status") != "active"
            or source.get("oracle_eligible") is not True
            or source.get("authority") not in RESTART_SOURCE_AUTHORITIES
        ):
            failures.append(
                "restart_timebase resolved_source requires an active "
                "oracle-eligible non-public-claim spec source"
            )
        source_path = restart.get("source_path")
        if source is not None and source.get("path") != source_path:
            failures.append(
                "restart_timebase resolved_source source_path must match source metadata"
            )
        _validate_durable_path(
            root,
            source_path,
            "restart_timebase resolved_source source_path",
            failures,
        )
    _require_text(restart.get("rationale"), "restart_timebase rationale", failures)
    _reject_claim_language(restart.get("rationale"), "restart_timebase rationale", failures)


def _validate_spec_review_schema(
    root_properties: Mapping[str, Any],
    allocation_schema: Mapping[str, Any],
    restart_schema: Mapping[str, Any],
    failures: list[str],
) -> None:
    reviews = root_properties.get("spec_gap_reviews", {})
    _closed_object_schema(
        reviews,
        SPEC_REVIEW_FIELDS,
        "runtime-anomaly spec_gap_reviews schema",
        failures,
    )
    review_properties = _properties(reviews)
    expected_refs = {
        "scan_cycle_allocation_policy": "#/$defs/allocation_review",
        "restart_timebase": "#/$defs/restart_timebase_review",
    }
    for field, expected in expected_refs.items():
        if review_properties.get(field, {}).get("$ref") != expected:
            failures.append(f"runtime-anomaly spec review schema ref for {field} drifts")
    allocation_properties = _properties(allocation_schema)
    for field, expected in {
        "outcome": "written_contract_present",
        "source_ref": "SPEC_RUNTIME_ENGINE_001",
        "source_path": "docs/specs/11-runtime-engine.md",
    }.items():
        if allocation_properties.get(field, {}).get("const") != expected:
            failures.append(f"runtime-anomaly allocation schema const for {field} drifts")
    required_text = allocation_properties.get("required_text", {})
    if required_text.get("const") != list(ALLOCATION_REQUIRED_TEXT):
        failures.append("runtime-anomaly allocation required_text schema drifts")


def _validate_durable_path(root: Path, value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, str) or not is_safe_relative_path(value):
        failures.append(f"{label} must be a normalized workspace-relative path")
        return
    candidate = root
    for part in PurePosixPath(value).parts:
        candidate /= part
        if candidate.is_symlink():
            failures.append(f"{label} must not contain a symlink")
            return
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(root.resolve())
    except (OSError, ValueError):
        failures.append(f"{label} is missing or escapes the workspace")
        return
    if not resolved.is_file():
        failures.append(f"{label} must identify a regular file")
        return
    if not _is_git_worktree(root):
        return
    ignored = subprocess.run(
        ["git", "-C", str(root), "check-ignore", "-q", "--", value],
        check=False,
        capture_output=True,
    )
    if ignored.returncode == 0:
        failures.append(f"{label} is gitignored")
        return
    if ignored.returncode != 1:
        failures.append(f"{label} git check-ignore failed")
        return
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", value],
        check=False,
        capture_output=True,
    )
    if tracked.returncode != 0:
        failures.append(f"{label} must identify a tracked durable file")


def _validate_source_kind_path(
    source_kind: Any, path: str, label: str, failures: list[str]
) -> None:
    parts = PurePosixPath(path).parts
    integration = len(parts) >= 4 and parts[0] == "crates" and parts[2] == "tests"
    unit = len(parts) >= 4 and parts[0] == "crates" and parts[2] == "src"
    if not path.endswith(".rs"):
        integration = False
        unit = False
    if source_kind == "rust_integration_test" and not integration:
        failures.append(
            f"runtime-anomaly mapping {label} path does not match rust_integration_test"
        )
    elif source_kind == "rust_unit_test" and not unit:
        failures.append(f"runtime-anomaly mapping {label} path does not match rust_unit_test")


def _closed_object_schema(
    schema: Mapping[str, Any], fields: set[str], label: str, failures: list[str]
) -> None:
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append(f"{label} must be a closed object")
    if set(schema.get("required", [])) != fields:
        failures.append(f"{label} required fields drift")
    properties = schema.get("properties")
    if not isinstance(properties, Mapping) or set(properties) != fields:
        failures.append(f"{label} properties drift")


def _enum_drift(
    root_schema: Mapping[str, Any],
    properties: Mapping[str, Any],
    field: str,
    expected: tuple[str, ...],
    failures: list[str],
) -> None:
    field_schema = _resolve_schema(root_schema, properties.get(field, {}))
    if set(field_schema.get("enum", [])) != set(expected):
        failures.append(f"runtime-anomaly {field} enum drifts")


def _resolve_schema(
    root_schema: Mapping[str, Any], value: Any
) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        return {}
    reference = value.get("$ref")
    if not isinstance(reference, str) or not reference.startswith("#/"):
        return value
    current: Any = root_schema
    for part in reference[2:].split("/"):
        key = part.replace("~1", "/").replace("~0", "~")
        if not isinstance(current, Mapping) or key not in current:
            return {}
        current = current[key]
    return current if isinstance(current, Mapping) else {}


def _date_schema(value: Any) -> bool:
    if not isinstance(value, Mapping):
        return False
    const = value.get("const")
    return value.get("pattern") == DATE_RE.pattern or (
        isinstance(const, str) and DATE_RE.fullmatch(const) is not None
    )


def _discovery_id_schema(value: Any) -> bool:
    if not isinstance(value, Mapping) or not isinstance(value.get("pattern"), str):
        return False
    try:
        pattern = re.compile(value["pattern"])
    except re.error:
        return False
    return (
        pattern.fullmatch("DISC_0123456789ABCDEF0123") is not None
        and pattern.fullmatch("DISC_0123456789abcdef0123") is None
        and pattern.fullmatch("DISC_0123456789ABCDEF012") is None
    )


def _properties(schema: Mapping[str, Any]) -> Mapping[str, Any]:
    value = schema.get("properties")
    return value if isinstance(value, Mapping) else {}


def _definition(definitions: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    value = definitions.get(name)
    return value if isinstance(value, Mapping) else {}


def _load_records(path: Path, key: str) -> dict[str, Mapping[str, Any]]:
    try:
        data = tomllib.loads(path.read_text())
    except Exception:
        return {}
    return {
        row["id"]: row
        for row in data.get(key, [])
        if isinstance(row, Mapping) and isinstance(row.get("id"), str)
    }


def _is_git_worktree(root: Path) -> bool:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--is-inside-work-tree"],
        check=False,
        capture_output=True,
    )
    return result.returncode == 0


def _require_text(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, str) or not value.strip():
        failures.append(f"{label} must be a non-empty string")


def _require_date(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, str) or not DATE_RE.fullmatch(value):
        failures.append(f"{label} last_reviewed must use YYYY-MM-DD")


def _require_string_array(value: Any, label: str, failures: list[str]) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(item, str) and item.strip() for item in value)
    ):
        failures.append(f"{label} must be a non-empty string array")
        return []
    if len(value) != len(set(value)):
        failures.append(f"{label} must be unique")
    return list(value)


def _reject_claim_language(value: Any, label: str, failures: list[str]) -> None:
    if isinstance(value, str) and FORBIDDEN_CLAIM_RE.search(value):
        failures.append(f"{label} contains forbidden proof/coverage language")


def _hashable_strings(values: list[Any]) -> list[str]:
    return [value for value in values if isinstance(value, str)]
