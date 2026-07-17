"""At-rest semantic contract for Phase 9 fuzz-program audit payloads."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from collections.abc import Mapping
from pathlib import PurePosixPath
from typing import Any

from .fuzz_program_analysis import GAP_REASONS, SURFACE_STATES
from .fuzz_program_contract import (
    AREA_IDS,
    REQUIRED_SURFACE_IDS,
    REVIEWED_CORPUS_POLICY,
    REVIEWED_CRASH_HANDOFF,
    TARGET_ID_ORDER,
    TIER_IDS,
)
from .fuzz_program_live import LiveFuzzProgramState, TIMESTAMP_RE, validate_timestamp
from .fuzz_program_report import BOUNDARIES, GENERATOR, GENERATOR_VERSION, LIMITATIONS, SCOPE
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
    "corpus_policy",
    "crash_regression_handoff",
    "targets",
    "surfaces",
    "gap_rows",
    "summary",
    "limitations",
}
REPORT_SCHEMA_SEMANTIC_DIGEST = (
    "1fe95e46ffbb2b1ac803282b258f97352e33dd19950abbb72068b2ee8fc6f181"
)
OUTPUT_PATH_FIELDS = {"json", "markdown"}
TARGET_REQUIRED_FIELDS = {
    "id",
    "target_kind",
    "name",
    "path",
    "command",
    "owner",
    "primary_tier",
    "additional_tiers",
    "enforcement_status",
    "execution_basis_ids",
    "surface_associations",
    "last_reviewed",
    "corpus_contents_assessed",
}
TARGET_PROPERTY_FIELDS = TARGET_REQUIRED_FIELDS | {
    "manifest_path",
    "corpus_path",
    "artifact_path",
    "discovery_id",
    "discovery_source_kind",
    "ignore_state",
}
SURFACE_FIELDS = {
    "surface_id",
    "title",
    "area",
    "state",
    "target_ids",
    "direct_target_ids",
    "partial_target_ids",
}
GAP_FIELDS = {"surface_id", "state", "reason", "target_ids"}
SUMMARY_FIELDS = {
    "inventory_targets",
    "cargo_fuzz_targets",
    "bounded_rust_smokes",
    "required_surfaces",
    "gap_surfaces",
    "by_surface_state",
    "by_primary_tier",
    "by_additional_tier",
}


def validate_schema_contract(schema: object) -> list[str]:
    if not isinstance(schema, dict):
        return ["fuzz-program report schema root must be an object"]
    failures: list[str] = []
    semantic_digest = hashlib.sha256(
        json.dumps(schema, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    if semantic_digest != REPORT_SCHEMA_SEMANTIC_DIGEST:
        failures.append("fuzz-program report schema semantic digest drifted")
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("fuzz-program report schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("fuzz-program report schema root required fields drift")
    root_properties = schema.get("properties")
    if not isinstance(root_properties, Mapping) or set(root_properties) != ROOT_FIELDS:
        failures.append("fuzz-program report schema root properties drift")
        root_properties = {}
    for field, expected in {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
    }.items():
        definition = root_properties.get(field)
        if not isinstance(definition, Mapping) or definition.get("const") != expected:
            failures.append(f"fuzz-program report schema {field} const drifted")
    if root_properties.get("input_digest") != {"$ref": "#/$defs/digest"}:
        failures.append("fuzz-program report schema input_digest binding drifted")
    commit = root_properties.get("commit")
    if not isinstance(commit, Mapping) or commit.get("pattern") != "^[0-9a-f]{40}$":
        failures.append("fuzz-program report schema commit pattern drifted")
    timestamp = root_properties.get("timestamp")
    if not isinstance(timestamp, Mapping) or timestamp.get("pattern") != TIMESTAMP_RE.pattern:
        failures.append("fuzz-program report schema timestamp pattern drifted")
    if root_properties.get("platform") != {"type": "string", "minLength": 1}:
        failures.append("fuzz-program report schema platform contract drifted")
    expected_root_arrays = {
        "command": {
            "type": "array",
            "items": {"type": "string", "minLength": 1},
            "minItems": 1,
        },
        "targets": {
            "type": "array",
            "items": {"$ref": "#/$defs/target"},
            "minItems": 17,
            "maxItems": 17,
        },
        "surfaces": {
            "type": "array",
            "items": {"$ref": "#/$defs/surface"},
            "minItems": 8,
            "maxItems": 8,
        },
        "gap_rows": {
            "type": "array",
            "items": {"$ref": "#/$defs/gap"},
            "minItems": 0,
            "maxItems": 0,
        },
    }
    for field, expected in expected_root_arrays.items():
        if root_properties.get(field) != expected:
            failures.append(f"fuzz-program report schema {field} array contract drifted")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        return [*failures, "fuzz-program report schema definitions are missing"]
    expected_enums = (
        ("surface_id", REQUIRED_SURFACE_IDS),
        ("target_id", TARGET_ID_ORDER),
        ("tier", TIER_IDS),
        ("surface_state", SURFACE_STATES),
        ("target_kind", ("cargo_fuzz", "bounded_rust_smoke")),
        ("enforcement_status", ("wired", "planned", "manual_only")),
        ("association_strength", ("direct", "partial")),
        ("area", AREA_IDS),
        ("gap_state", ("smoke_only", "partial_only", "unmapped")),
        (
            "gap_reason",
            ("no_cargo_fuzz_target", "no_direct_surface_target", "no_associated_target"),
        ),
    )
    for name, expected in expected_enums:
        definition = definitions.get(name)
        if not isinstance(definition, Mapping) or definition.get("enum") != list(expected):
            failures.append(f"fuzz-program report schema {name} enum drifted")
    for name in (
        "output_paths",
        "scope",
        "boundaries",
        "corpus_policy",
        "crash_regression_handoff",
        "target",
        "surface",
        "gap",
        "summary",
    ):
        definition = definitions.get(name)
        if not isinstance(definition, Mapping) or definition.get("additionalProperties") is not False:
            failures.append(f"fuzz-program report schema {name} must be a closed object")
    expected_shapes = {
        "output_paths": (OUTPUT_PATH_FIELDS, OUTPUT_PATH_FIELDS),
        "scope": (set(SCOPE), set(SCOPE)),
        "boundaries": (set(BOUNDARIES), set(BOUNDARIES)),
        "corpus_policy": (set(REVIEWED_CORPUS_POLICY), set(REVIEWED_CORPUS_POLICY)),
        "crash_regression_handoff": (
            set(REVIEWED_CRASH_HANDOFF),
            set(REVIEWED_CRASH_HANDOFF),
        ),
        "target": (TARGET_REQUIRED_FIELDS, TARGET_PROPERTY_FIELDS),
        "surface": (SURFACE_FIELDS, SURFACE_FIELDS),
        "gap": (GAP_FIELDS, GAP_FIELDS),
        "summary": (SUMMARY_FIELDS, SUMMARY_FIELDS),
    }
    for name, (required, properties) in expected_shapes.items():
        _validate_closed_definition_shape(
            definitions.get(name), required, properties, name, failures
        )
    digest = definitions.get("digest")
    if (
        not isinstance(digest, Mapping)
        or digest.get("type") != "string"
        or digest.get("pattern") != "^sha256:[0-9a-f]{64}$"
    ):
        failures.append("fuzz-program report schema digest contract drifted")
    output_definition = definitions.get("output_paths")
    output_properties = (
        output_definition.get("properties")
        if isinstance(output_definition, Mapping)
        else None
    )
    if not isinstance(output_properties, Mapping) or any(
        output_properties.get(field) != {"type": "string", "minLength": 1}
        for field in OUTPUT_PATH_FIELDS
    ):
        failures.append("fuzz-program report schema output path types drifted")
    target_definition = definitions.get("target")
    target_properties = (
        target_definition.get("properties")
        if isinstance(target_definition, Mapping)
        else None
    )
    expected_target_bindings = {
        "id": {"$ref": "#/$defs/target_id"},
        "target_kind": {"$ref": "#/$defs/target_kind"},
        "name": {"type": "string", "minLength": 1},
        "path": {"type": "string", "minLength": 1},
        "command": {"type": "string", "minLength": 1},
        "owner": {"type": "string", "minLength": 1},
        "ignore_state": {"const": "not_ignored"},
        "primary_tier": {"$ref": "#/$defs/tier"},
        "additional_tiers": {"$ref": "#/$defs/tier_array"},
        "enforcement_status": {"$ref": "#/$defs/enforcement_status"},
        "execution_basis_ids": {"$ref": "#/$defs/string_array"},
    }
    if not isinstance(target_properties, Mapping):
        failures.append("fuzz-program report schema target properties are missing")
    else:
        for field, expected in expected_target_bindings.items():
            if target_properties.get(field) != expected:
                failures.append(f"fuzz-program report schema target {field} binding drifted")
        expected_associations = {
            "type": "array",
            "items": {"$ref": "#/$defs/surface_association"},
            "minItems": 1,
            "uniqueItems": True,
        }
        if target_properties.get("surface_associations") != expected_associations:
            failures.append("fuzz-program report schema target surface_associations binding drifted")
    surface_definition = definitions.get("surface")
    surface_properties = (
        surface_definition.get("properties")
        if isinstance(surface_definition, Mapping)
        else None
    )
    expected_surface_bindings = {
        "surface_id": {"$ref": "#/$defs/surface_id"},
        "title": {"type": "string", "minLength": 1},
        "area": {"$ref": "#/$defs/area"},
        "state": {"$ref": "#/$defs/surface_state"},
        "target_ids": {"$ref": "#/$defs/target_id_array"},
        "direct_target_ids": {"$ref": "#/$defs/target_id_array"},
        "partial_target_ids": {"$ref": "#/$defs/target_id_array"},
    }
    if not isinstance(surface_properties, Mapping):
        failures.append("fuzz-program report schema surface properties are missing")
    else:
        for field, expected in expected_surface_bindings.items():
            if surface_properties.get(field) != expected:
                failures.append(f"fuzz-program report schema surface {field} binding drifted")
    for name, fields in (
        ("count_map_tier", set(TIER_IDS)),
        ("count_map_surface_state", set(SURFACE_STATES)),
    ):
        _validate_closed_definition_shape(
            definitions.get(name), fields, fields, name, failures
        )
        definition = definitions.get(name)
        properties = definition.get("properties") if isinstance(definition, Mapping) else None
        if not isinstance(properties, Mapping) or any(
            properties.get(field) != {"type": "integer", "minimum": 0}
            for field in fields
        ):
            failures.append(f"fuzz-program report schema {name} count fields drifted")
    expected_array_definitions = {
        "string_array": {
            "type": "array",
            "items": {"type": "string", "minLength": 1},
            "uniqueItems": True,
        },
        "tier_array": {
            "type": "array",
            "items": {"$ref": "#/$defs/tier"},
            "uniqueItems": True,
        },
        "target_id_array": {
            "type": "array",
            "items": {"$ref": "#/$defs/target_id"},
            "uniqueItems": True,
        },
    }
    for name, expected in expected_array_definitions.items():
        if definitions.get(name) != expected:
            failures.append(f"fuzz-program report schema {name} array contract drifted")
    surface_association = definitions.get("surface_association")
    _validate_closed_definition_shape(
        surface_association,
        {"surface_id", "strength", "rationale"},
        {"surface_id", "strength", "rationale"},
        "surface_association",
        failures,
    )
    association_properties = (
        surface_association.get("properties")
        if isinstance(surface_association, Mapping)
        else None
    )
    if not isinstance(association_properties, Mapping) or (
        association_properties.get("surface_id") != {"$ref": "#/$defs/surface_id"}
        or association_properties.get("strength")
        != {"$ref": "#/$defs/association_strength"}
        or association_properties.get("rationale")
        != {"type": "string", "minLength": 1}
    ):
        failures.append("fuzz-program report schema surface association bindings drifted")
    gap_definition = definitions.get("gap")
    gap_properties = gap_definition.get("properties") if isinstance(gap_definition, Mapping) else None
    expected_gap_bindings = {
        "surface_id": {"$ref": "#/$defs/surface_id"},
        "state": {"$ref": "#/$defs/gap_state"},
        "reason": {"$ref": "#/$defs/gap_reason"},
        "target_ids": {"$ref": "#/$defs/target_id_array"},
    }
    if not isinstance(gap_properties, Mapping) or any(
        gap_properties.get(field) != expected
        for field, expected in expected_gap_bindings.items()
    ):
        failures.append("fuzz-program report schema gap bindings drifted")
    _validate_schema_consts(definitions.get("scope"), SCOPE, "scope", failures)
    _validate_schema_consts(definitions.get("boundaries"), BOUNDARIES, "boundaries", failures)
    _validate_schema_consts(
        definitions.get("corpus_policy"),
        REVIEWED_CORPUS_POLICY,
        "corpus_policy",
        failures,
    )
    _validate_schema_consts(
        definitions.get("crash_regression_handoff"),
        REVIEWED_CRASH_HANDOFF,
        "crash_regression_handoff",
        failures,
    )
    summary = definitions.get("summary")
    summary_properties = summary.get("properties") if isinstance(summary, Mapping) else None
    expected_summary_consts = {
        "inventory_targets": 17,
        "cargo_fuzz_targets": 11,
        "bounded_rust_smokes": 6,
        "required_surfaces": 8,
        "gap_surfaces": 0,
    }
    if not isinstance(summary_properties, Mapping) or any(
        not isinstance(summary_properties.get(field), Mapping)
        or summary_properties[field].get("const") != expected
        for field, expected in expected_summary_consts.items()
    ):
        failures.append("fuzz-program report schema summary consts drifted")
    return failures


def _validate_schema_consts(
    definition: object,
    expected: Mapping[str, Any],
    name: str,
    failures: list[str],
) -> None:
    properties = definition.get("properties") if isinstance(definition, Mapping) else None
    if not isinstance(properties, Mapping):
        failures.append(f"fuzz-program report schema {name} properties are missing")
        return
    actual = {
        field: value.get("const") if isinstance(value, Mapping) else None
        for field, value in properties.items()
    }
    if actual != dict(expected):
        failures.append(f"fuzz-program report schema {name} consts drifted")


def _validate_closed_definition_shape(
    definition: object,
    required: set[str],
    properties: set[str],
    name: str,
    failures: list[str],
) -> None:
    if not isinstance(definition, Mapping) or definition.get("additionalProperties") is not False:
        failures.append(f"fuzz-program report schema {name} must be a closed object")
        return
    if set(definition.get("required", [])) != required:
        failures.append(f"fuzz-program report schema {name} required fields drifted")
    actual_properties = definition.get("properties")
    if not isinstance(actual_properties, Mapping) or set(actual_properties) != properties:
        failures.append(f"fuzz-program report schema {name} properties drifted")


def validate_report_payload(
    payload: object,
    *,
    expected_state: LiveFuzzProgramState | None = None,
) -> list[str]:
    """Return failures for hostile payloads instead of raising on bad shapes."""

    failures: list[str] = []
    if not isinstance(payload, Mapping):
        return ["fuzz-program report root must be an object"]
    if set(payload) != ROOT_FIELDS:
        failures.append("fuzz-program report root fields drift from the closed contract")
    expected_scalars = {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
    }
    for field, expected in expected_scalars.items():
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected!r}")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")
    if payload.get("scope") != SCOPE:
        failures.append("scope drifted from the Phase 9 report contract")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries drifted from the Phase 9 report contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations drifted from the Phase 9 report contract")
    try:
        validate_timestamp(payload.get("timestamp"))
    except ValueError as exc:
        failures.append(str(exc))
    targets = payload.get("targets")
    surfaces = payload.get("surfaces")
    gaps = payload.get("gap_rows")
    summary = payload.get("summary")
    try:
        _validate_paths_and_command(payload, failures)
        _validate_rows(targets, surfaces, gaps, failures)
        _validate_summary(targets, surfaces, gaps, summary, failures)
    except (AttributeError, KeyError, TypeError, ValueError) as exc:
        failures.append(f"semantic validation rejected malformed payload shape: {type(exc).__name__}")
    if expected_state is not None:
        expected = expected_state.analysis
        if targets != expected["targets"]:
            failures.append("report targets do not match current live Phase 9 inventory")
        if surfaces != expected["surfaces"]:
            failures.append("report surfaces do not match current live Phase 9 analysis")
        if gaps != expected["gap_rows"]:
            failures.append("report gap rows do not match current live Phase 9 analysis")
        if summary != expected["summary"]:
            failures.append("report summary does not match current live Phase 9 analysis")
        if payload.get("corpus_policy") != expected_state.corpus_policy:
            failures.append("corpus policy does not match current live Phase 9 metadata")
        if payload.get("crash_regression_handoff") != expected_state.crash_regression_handoff:
            failures.append("crash-regression handoff does not match current live Phase 9 metadata")
        if payload.get("input_paths") != list(expected_state.input_paths):
            failures.append("input_paths do not match the complete live Phase 9 closure")
        if payload.get("input_digest") != expected_state.input_digest:
            failures.append("input_digest does not match the complete live Phase 9 closure")
    return sorted(set(failures))


def _validate_paths_and_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    output_paths = payload.get("output_paths")
    if not isinstance(output_paths, Mapping):
        failures.append("output_paths must be an object")
        return
    json_path = output_paths.get("json")
    markdown_path = output_paths.get("markdown")
    if not isinstance(json_path, str) or not isinstance(markdown_path, str):
        failures.append("output paths must be strings")
        return
    if not _safe_relative_path(json_path) or not _safe_relative_path(markdown_path):
        failures.append("output paths must be normalized and workspace-relative")
    if json_path == markdown_path:
        failures.append("JSON and Markdown output paths must be distinct")
    expected_command = [
        "python3",
        "scripts/report_fuzz_program_audit.py",
        "--json-out",
        json_path,
        "--markdown-out",
        markdown_path,
        "--timestamp",
        payload.get("timestamp"),
    ]
    if payload.get("command") != expected_command:
        failures.append("command does not match the canonical Phase 9 generator invocation")
    input_paths = payload.get("input_paths")
    if not isinstance(input_paths, list) or not input_paths or not all(
        isinstance(item, str) and item for item in input_paths
    ):
        failures.append("input_paths must be a non-empty string array")
    elif input_paths != sorted(set(input_paths)):
        failures.append("input_paths must be unique and canonical-ordered")
    elif not all(_safe_relative_path(item) for item in input_paths):
        failures.append("input_paths must be normalized and workspace-relative")


def _validate_rows(
    targets: object,
    surfaces: object,
    gaps: object,
    failures: list[str],
) -> None:
    if not isinstance(targets, list):
        failures.append("targets must be an array")
        targets = []
    if not isinstance(surfaces, list):
        failures.append("surfaces must be an array")
        surfaces = []
    if not isinstance(gaps, list):
        failures.append("gap_rows must be an array")
        gaps = []
    surface_ids = [row.get("surface_id") for row in surfaces if isinstance(row, Mapping)]
    if surface_ids != list(REQUIRED_SURFACE_IDS):
        failures.append("surface rows must use the exact Phase 9 order")
    expected_gap_ids = [
        row.get("surface_id")
        for row in surfaces
        if isinstance(row, Mapping) and row.get("state") != "cargo_fuzz_target"
    ]
    actual_gap_ids = [row.get("surface_id") for row in gaps if isinstance(row, Mapping)]
    if actual_gap_ids != expected_gap_ids:
        failures.append("gap rows must be the exhaustive non-cargo-target surface partition")
    target_ids = {row.get("id") for row in targets if isinstance(row, Mapping)}
    for index, row in enumerate(surfaces):
        if not isinstance(row, Mapping):
            failures.append(f"surfaces[{index}] must be an object")
            continue
        state = row.get("state")
        if state not in SURFACE_STATES:
            failures.append(f"surfaces[{index}] uses unknown state {state!r}")
        for field in ("target_ids", "direct_target_ids", "partial_target_ids"):
            values = row.get(field)
            if (
                not isinstance(values, list)
                or not all(isinstance(item, str) for item in values)
                or values != sorted(set(values))
            ):
                failures.append(f"surfaces[{index}].{field} must be a canonical unique array")
            elif not set(values).issubset(target_ids):
                failures.append(f"surfaces[{index}].{field} names an unknown target")
    for index, row in enumerate(gaps):
        if not isinstance(row, Mapping):
            failures.append(f"gap_rows[{index}] must be an object")
            continue
        state = row.get("state")
        if state not in GAP_REASONS or row.get("reason") != GAP_REASONS.get(state):
            failures.append(f"gap_rows[{index}] reason does not match its state")


def _validate_summary(
    targets: object,
    surfaces: object,
    gaps: object,
    summary: object,
    failures: list[str],
) -> None:
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
        return
    target_rows = [row for row in targets if isinstance(row, Mapping)] if isinstance(targets, list) else []
    surface_rows = [row for row in surfaces if isinstance(row, Mapping)] if isinstance(surfaces, list) else []
    gap_rows = [row for row in gaps if isinstance(row, Mapping)] if isinstance(gaps, list) else []
    states = Counter(row.get("state") for row in surface_rows)
    primary = Counter(row.get("primary_tier") for row in target_rows)
    additional: Counter[str] = Counter()
    for row in target_rows:
        values = row.get("additional_tiers")
        if isinstance(values, list):
            additional.update(tier for tier in values if isinstance(tier, str))
    expected = {
        "inventory_targets": len(target_rows),
        "cargo_fuzz_targets": sum(row.get("target_kind") == "cargo_fuzz" for row in target_rows),
        "bounded_rust_smokes": sum(row.get("target_kind") == "bounded_rust_smoke" for row in target_rows),
        "required_surfaces": len(surface_rows),
        "gap_surfaces": len(gap_rows),
        "by_surface_state": {state: states[state] for state in SURFACE_STATES},
        "by_primary_tier": {tier: primary[tier] for tier in TIER_IDS},
        "by_additional_tier": {tier: additional[tier] for tier in TIER_IDS},
    }
    if summary != expected:
        failures.append("summary does not equal the values recomputed from report rows")


def _safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return bool(path.parts) and not path.is_absolute() and ".." not in path.parts and "." not in path.parts
