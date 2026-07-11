"""Semantic contract for Phase 10 mutation-program reports."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from pathlib import PurePosixPath
from typing import Any

from .metadata_validator.mutation_reports import (
    derive_reported_result,
    has_infrastructure_failure,
)
from .mutation_program_contract import REQUIRED_SHARD_IDS
from .mutation_program_live import LiveMutationProgramState, validate_timestamp
from .mutation_program_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    SCOPE,
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
    "tool",
    "shards",
    "survivors",
    "coverage",
    "summary",
    "limitations",
}
SHARD_FIELDS = {
    "id",
    "title",
    "area",
    "invariant_ids",
    "association_semantics",
    "owner",
    "execution_status",
    "delivered_build_requirement",
    "delivered_binary_path",
    "delivered_confirmation_requirements",
    "delivered_build_confirmation",
    "associated_tests",
    "mutations",
    "result_artifact",
    "results",
}
MUTATION_FIELDS = {
    "id",
    "source_file",
    "source_digest",
    "function",
    "genre",
    "replacement",
    "generated_mutant_name",
    "build_command",
    "test_command",
    "association_ids",
}
RESULT_FIELDS = {
    "id",
    "source_file",
    "function",
    "genre",
    "replacement",
    "generated_mutant_name",
    "build_command",
    "build_exit_status",
    "build_output_tail",
    "build_timed_out",
    "test_command",
    "test_exit_status",
    "test_output_tail",
    "test_timed_out",
    "duration_seconds",
    "result",
    "association_ids",
}
SURVIVOR_FIELDS = {
    "shard_id",
    "mutation_id",
    "owner",
    "action",
    "resolution_status",
    "rationale",
    "resolution_ref",
}
SUMMARY_FIELDS = {
    "shards",
    "measured_shards",
    "planned_shards",
    "defined_mutants",
    "measured_mutants",
    "caught",
    "survived",
    "unviable",
    "timeout",
    "error",
}
RESULTS = ("caught", "survived", "unviable", "timeout", "error")
ALLOWED_SURVIVOR_ACTIONS = {
    "add_test",
    "unreachable_defensive_rationale",
    "dead_code_removal",
}
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
ASSOCIATED_TEST_FIELDS = {"id_kind", "id", "source_kind", "path", "name", "ignore_state"}
REPORT_SCHEMA_SEMANTIC_DIGEST = (
    "3711954bcbfffc04a9d2a905191865cbc18ec1cace320106cac10164d704c49f"
)


def validate_schema_contract(schema: object) -> list[str]:
    if not isinstance(schema, Mapping):
        return ["mutation survivor report schema root must be an object"]
    failures: list[str] = []
    semantic_digest = hashlib.sha256(
        json.dumps(schema, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    if semantic_digest != REPORT_SCHEMA_SEMANTIC_DIGEST:
        failures.append("mutation survivor report schema semantic digest drifted")
    check_supported_schema_keywords(dict(schema), "$", failures)
    _closed_schema(schema, ROOT_FIELDS, "root", failures)
    properties = _properties(schema)
    for field, expected in {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
    }.items():
        if _property_schema(properties, field).get("const") != expected:
            failures.append(f"mutation survivor report schema {field} const drifted")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        return sorted(set([*failures, "mutation survivor report schema definitions missing"]))
    for name in (
        "output_paths",
        "scope",
        "boundaries",
        "tool",
        "associated_test",
        "mutation",
        "result_artifact",
        "delivered_confirmation",
        "result",
        "shard",
        "survivor",
        "coverage",
        "summary",
    ):
        definition = definitions.get(name)
        if isinstance(definition, Mapping) and definition.get("type") == "object":
            expected_fields = set(_properties(definition))
            _closed_schema(definition, expected_fields, name, failures)
        elif name not in {"result_artifact", "delivered_confirmation"}:
            failures.append(f"mutation survivor report schema {name} definition missing")
    shard_definition = definitions.get("shard")
    mutation_definition = definitions.get("mutation")
    survivor_definition = definitions.get("survivor")
    result_definition = definitions.get("result")
    summary_definition = definitions.get("summary")
    if isinstance(shard_definition, Mapping):
        if set(_properties(shard_definition)) != SHARD_FIELDS:
            failures.append("mutation survivor report schema shard fields drifted")
    if isinstance(mutation_definition, Mapping):
        if set(_properties(mutation_definition)) != MUTATION_FIELDS:
            failures.append("mutation survivor report schema mutation fields drifted")
    if isinstance(result_definition, Mapping):
        if set(_properties(result_definition)) != RESULT_FIELDS:
            failures.append("mutation survivor report schema result fields drifted")
    if isinstance(survivor_definition, Mapping):
        if set(_properties(survivor_definition)) != SURVIVOR_FIELDS:
            failures.append("mutation survivor report schema survivor fields drifted")
        actions = _property_schema(_properties(survivor_definition), "action").get("enum")
        action_set = _string_set(actions)
        if action_set != ALLOWED_SURVIVOR_ACTIONS:
            failures.append("mutation survivor report schema survivor action enum drifted")
    if isinstance(summary_definition, Mapping):
        summary_properties = _properties(summary_definition)
        if set(summary_properties) != SUMMARY_FIELDS:
            failures.append("mutation survivor report schema summary fields drifted")
        if any(
            _property_schema(summary_properties, key).get("type") != "integer"
            or _property_schema(summary_properties, key).get("minimum") != 0
            for key in SUMMARY_FIELDS
        ):
            failures.append(
                "mutation survivor report schema summary counters must be non-negative integers"
            )
    _schema_consts(definitions.get("scope"), SCOPE, "scope", failures)
    _schema_consts(definitions.get("boundaries"), BOUNDARIES, "boundaries", failures)
    tool = _properties(definitions.get("tool"))
    for field, expected in {
        "name": "cargo-mutants",
        "version": "cargo-mutants 27.0.0",
        "selection_mode": "single_file_list_only",
    }.items():
        if _property_schema(tool, field).get("const") != expected:
            failures.append(f"mutation survivor report schema tool {field} const drifted")
    coverage = _properties(definitions.get("coverage"))
    if _property_schema(coverage, "runs").get("const") != 0:
        failures.append("mutation survivor report schema coverage runs const drifted")
    root_shards = _property_schema(properties, "shards")
    if root_shards.get("minItems") != 6 or root_shards.get("maxItems") != 6:
        failures.append("mutation survivor report schema shard cardinality drifted")
    return sorted(set(failures))


def validate_report_payload(
    payload: object,
    *,
    expected_state: LiveMutationProgramState | None = None,
) -> list[str]:
    """Validate hostile payloads without raising on malformed shapes."""

    if not isinstance(payload, Mapping):
        return ["mutation survivor report root must be an object"]
    failures: list[str] = []
    if set(payload) != ROOT_FIELDS:
        failures.append("mutation survivor report root fields drift from the closed contract")
    for field, expected in {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
        "scope": SCOPE,
        "boundaries": BOUNDARIES,
        "limitations": list(LIMITATIONS),
        "tool": {
            "name": "cargo-mutants",
            "version": "cargo-mutants 27.0.0",
            "selection_mode": "single_file_list_only",
        },
    }.items():
        if payload.get(field) != expected:
            failures.append(f"mutation survivor report {field} drifted from contract")
    try:
        validate_timestamp(payload.get("timestamp"))
    except ValueError as exc:
        failures.append(str(exc))
    _validate_provenance(payload, failures)
    shards = _validate_shards(payload.get("shards"), failures)
    survivors = _validate_survivors(payload.get("survivors"), shards, failures)
    _validate_coverage(payload.get("coverage"), failures)
    _validate_summary(payload.get("summary"), shards, survivors, failures)
    if expected_state is not None:
        if shards != list(expected_state.shards):
            failures.append("report shard rows do not match current live Phase 10 mutation state")
        if survivors != list(expected_state.survivors):
            failures.append("report survivors do not match current live Phase 10 mutation state")
        if payload.get("coverage") != expected_state.coverage:
            failures.append("report coverage does not match current live Phase 10 mutation state")
        if payload.get("summary") != expected_state.summary:
            failures.append("report summary does not match current live Phase 10 mutation state")
        if payload.get("input_paths") != list(expected_state.input_paths):
            failures.append("input_paths do not match the complete live Phase 10 closure")
        if payload.get("input_digest") != expected_state.input_digest:
            failures.append("input_digest does not match the complete live Phase 10 closure")
    return sorted(set(failures))


def _validate_provenance(payload: Mapping[str, Any], failures: list[str]) -> None:
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")
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
        "scripts/report_mutation_program.py",
        "--json-out",
        json_path,
        "--markdown-out",
        markdown_path,
        "--timestamp",
        payload.get("timestamp"),
    ]
    if payload.get("command") != expected_command:
        failures.append("command does not match the canonical Phase 10 generator invocation")
    input_paths = payload.get("input_paths")
    if not isinstance(input_paths, list) or not input_paths or not all(
        isinstance(item, str) and item for item in input_paths
    ):
        failures.append("input_paths must be a non-empty string array")
    elif input_paths != sorted(set(input_paths)):
        failures.append("input_paths must be unique and canonical-ordered")
    elif not all(_safe_relative_path(item) for item in input_paths):
        failures.append("input_paths must be normalized and workspace-relative")
    if isinstance(input_paths, list):
        collisions = {
            path
            for path in (json_path, markdown_path)
            if isinstance(path, str) and path in input_paths
        }
        if collisions:
            failures.append(
                "report output paths cannot overwrite bound inputs: "
                + ", ".join(sorted(collisions))
            )


def _validate_shards(value: object, failures: list[str]) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append("shards must be an array")
        return []
    rows = [dict(row) for row in value if isinstance(row, Mapping)]
    if len(rows) != len(value):
        failures.append("every shard must be an object")
    if [row.get("id") for row in rows] != list(REQUIRED_SHARD_IDS):
        failures.append("shards must use the exact ordered Phase 10 shard IDs")
    for index, row in enumerate(rows):
        label = f"shards[{index}]"
        if set(row) != SHARD_FIELDS:
            failures.append(f"{label} fields drift from the closed contract")
        status = row.get("execution_status")
        if not isinstance(status, str) or status not in {"measured", "planned"}:
            failures.append(f"{label} execution_status is invalid")
        if row.get("association_semantics") != "association_only_not_execution_claim":
            failures.append(f"{label} must preserve association-only semantics")
        invariant_ids = row.get("invariant_ids")
        if not _is_unique_nonempty_string_list(invariant_ids):
            failures.append(f"{label} invariant_ids must be unique non-empty strings")
        if not isinstance(row.get("owner"), str) or not row.get("owner"):
            failures.append(f"{label} owner must be non-empty text")
        definitions = _validate_mutation_definitions(row.get("mutations"), label, failures)
        results = _validate_results(row.get("results"), definitions, label, failures)
        if status == "planned" and results:
            failures.append(f"{label} planned shard must not fabricate mutation results")
        if status == "measured" and set(results) != set(definitions):
            failures.append(f"{label} measured outcome set does not match defined mutants")
        artifact = row.get("result_artifact")
        if status == "measured":
            if not _valid_artifact(artifact):
                failures.append(f"{label} measured shard requires a digest-bound result artifact")
        elif artifact is not None:
            failures.append(f"{label} planned shard must not claim a result artifact")
        _validate_delivered_confirmation(row, index, failures)
        associated_tests = row.get("associated_tests")
        _validate_associated_tests(associated_tests, label, failures)
        if isinstance(associated_tests, list):
            expected_association_ids = [
                item.get("id")
                for item in associated_tests
                if isinstance(item, Mapping)
            ]
            actual_association_ids: list[str] = []
            for definition in definitions.values():
                values = definition.get("association_ids")
                if isinstance(values, list):
                    actual_association_ids.extend(
                        item for item in values if isinstance(item, str)
                    )
            if actual_association_ids != expected_association_ids:
                failures.append(
                    f"{label} mutant association_ids must partition associated tests exactly"
                )
    return rows


def _validate_associated_tests(value: object, label: str, failures: list[str]) -> None:
    if not isinstance(value, list) or not value:
        failures.append(f"{label} associated_tests must be a non-empty object array")
        return
    seen: set[tuple[object, object]] = set()
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            failures.append(f"{label}.associated_tests[{index}] must be an object")
            continue
        if set(item) != ASSOCIATED_TEST_FIELDS:
            failures.append(f"{label}.associated_tests[{index}] fields drift from contract")
        id_kind = item.get("id_kind")
        identity = item.get("id")
        if isinstance(id_kind, str) and isinstance(identity, str):
            key = (id_kind, identity)
            if key in seen:
                failures.append(f"{label} duplicates an associated test identity")
            seen.add(key)
        if not isinstance(id_kind, str) or id_kind not in {
            "committed_case_id",
            "scanner_discovery_id",
        }:
            failures.append(f"{label}.associated_tests[{index}] id_kind is invalid")
        if not all(
            isinstance(item.get(field), str) and item.get(field)
            for field in ("id", "source_kind", "path", "name", "ignore_state")
        ):
            failures.append(f"{label}.associated_tests[{index}] text fields must be non-empty")
        elif not _safe_relative_path(item["path"]):
            failures.append(f"{label}.associated_tests[{index}] path is unsafe")


def _validate_mutation_definitions(
    value: object,
    label: str,
    failures: list[str],
) -> dict[str, dict[str, Any]]:
    if not isinstance(value, list) or not value or len(value) > 2:
        failures.append(f"{label} must define one or two focused mutants")
        return {}
    definitions: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            failures.append(f"{label}.mutations[{index}] must be an object")
            continue
        row = dict(item)
        if set(row) != MUTATION_FIELDS:
            failures.append(f"{label}.mutations[{index}] fields drift from contract")
        mutation_id = row.get("id")
        if not isinstance(mutation_id, str) or not mutation_id:
            failures.append(f"{label}.mutations[{index}] has invalid id")
            continue
        if mutation_id in definitions:
            failures.append(f"{label} duplicates mutation id {mutation_id}")
        definitions[mutation_id] = row
        association_ids = row.get("association_ids")
        if (
            not isinstance(association_ids, list)
            or not association_ids
            or not all(isinstance(item, str) and item for item in association_ids)
            or len(association_ids) != len(set(association_ids))
        ):
            failures.append(
                f"{label} {mutation_id} association_ids must be unique non-empty strings"
            )
    return definitions


def _validate_results(
    value: object,
    definitions: Mapping[str, Mapping[str, Any]],
    label: str,
    failures: list[str],
) -> dict[str, dict[str, Any]]:
    if not isinstance(value, list):
        failures.append(f"{label}.results must be an array")
        return {}
    results: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            failures.append(f"{label}.results[{index}] must be an object")
            continue
        row = dict(item)
        if set(row) != RESULT_FIELDS:
            failures.append(f"{label}.results[{index}] fields drift from the closed outcome contract")
        mutation_id = row.get("id")
        if not isinstance(mutation_id, str) or not mutation_id:
            failures.append(f"{label}.results[{index}] has invalid id")
            continue
        if mutation_id in results:
            failures.append(f"{label} duplicates measured mutation {mutation_id}")
        results[mutation_id] = row
        definition = definitions.get(mutation_id)
        if definition is None:
            failures.append(f"{label} result names undefined mutation {mutation_id}")
            continue
        for field in (
            "source_file",
            "function",
            "genre",
            "replacement",
            "generated_mutant_name",
            "build_command",
            "test_command",
            "association_ids",
        ):
            if row.get(field) != definition.get(field):
                failures.append(f"{label} {mutation_id} result {field} drifts from selector")
        duration = row.get("duration_seconds")
        if (
            not isinstance(duration, (int, float))
            or isinstance(duration, bool)
            or duration < 0
        ):
            failures.append(f"{label} {mutation_id} duration_seconds must be non-negative")
        for field in ("build_output_tail", "test_output_tail"):
            if not isinstance(row.get(field), str):
                failures.append(f"{label} {mutation_id} {field} must be text")
        if _outcome_has_infrastructure_failure(row):
            failures.append(f"{label} {mutation_id} contains an infrastructure failure")
        try:
            derived, derivation_failure = derive_reported_result(row)
        except (KeyError, TypeError, ValueError) as exc:
            failures.append(f"{label} {mutation_id} outcome shape is invalid: {type(exc).__name__}")
            continue
        if derivation_failure:
            failures.append(f"{label} {mutation_id} {derivation_failure}")
        elif row.get("result") != derived:
            failures.append(
                f"{label} {mutation_id} declared result does not match derived result {derived}"
            )
        if derived == "error":
            failures.append(f"{label} {mutation_id} complete reports cannot contain error outcomes")
    return results


def _validate_delivered_confirmation(
    row: Mapping[str, Any],
    index: int,
    failures: list[str],
) -> None:
    confirmation = row.get("delivered_build_confirmation")
    requirement = row.get("delivered_build_requirement")
    measured = row.get("execution_status") == "measured"
    delivered_required = requirement == "required_before_execution" or index == 5
    if not isinstance(requirement, str) or requirement not in {
        "not_applicable_source_mutation",
        "required_before_execution",
    }:
        failures.append(f"shards[{index}] delivered build requirement is invalid")
    binary_path = row.get("delivered_binary_path")
    confirmation_requirements = row.get("delivered_confirmation_requirements")
    if requirement == "required_before_execution":
        if not isinstance(binary_path, str) or not binary_path:
            failures.append(f"shards[{index}] delivered binary path is required")
        if confirmation_requirements != ["artifact_sha256", "direct_execution"]:
            failures.append(f"shards[{index}] delivered confirmation requirements drifted")
    elif binary_path is not None or confirmation_requirements != []:
        failures.append(f"shards[{index}] source mutation forbids delivered binary fields")
    if measured and delivered_required:
        if not isinstance(confirmation, Mapping):
            failures.append("delivered connector mutation requires artifact digest and direct execution confirmation")
            return
        if not DIGEST_RE.fullmatch(str(confirmation.get("artifact_sha256", ""))):
            failures.append("delivered connector mutation requires an artifact SHA-256")
        if confirmation.get("direct_execution_confirmed") is not True:
            failures.append("delivered connector mutation requires direct execution confirmation")
        if not _safe_relative_path(str(confirmation.get("artifact_path", ""))):
            failures.append("delivered connector mutation artifact path is unsafe")
    elif confirmation is not None:
        failures.append("delivered build confirmation is forbidden when no delivered run is measured")


def _validate_survivors(
    value: object,
    shards: list[dict[str, Any]],
    failures: list[str],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append("survivors must be an array")
        return []
    rows = [dict(row) for row in value if isinstance(row, Mapping)]
    if len(rows) != len(value):
        failures.append("every survivor must be an object")
    expected: set[tuple[str, str]] = set()
    for shard in shards:
        shard_id = shard.get("id")
        for result in _mapping_rows(shard.get("results")):
            mutation_id = result.get("id")
            if (
                result.get("result") == "survived"
                and isinstance(shard_id, str)
                and isinstance(mutation_id, str)
            ):
                expected.add((shard_id, mutation_id))
    actual: set[tuple[Any, Any]] = set()
    for index, row in enumerate(rows):
        label = f"survivors[{index}]"
        if set(row) != SURVIVOR_FIELDS:
            failures.append(f"{label} fields drift from the closed survivor contract")
        shard_id = row.get("shard_id")
        mutation_id = row.get("mutation_id")
        if not isinstance(shard_id, str) or not isinstance(mutation_id, str):
            failures.append(f"{label} shard_id and mutation_id must be strings")
        else:
            key = (shard_id, mutation_id)
            if key in actual:
                failures.append(f"{label} duplicates a survivor disposition")
            actual.add(key)
        action = row.get("action")
        if not isinstance(action, str) or action not in ALLOWED_SURVIVOR_ACTIONS:
            failures.append(f"{label} survivor action is not allowed")
        if row.get("resolution_status") != "resolved":
            failures.append(f"{label} survivor resolution must be resolved")
        if not isinstance(row.get("owner"), str) or not row.get("owner"):
            failures.append(f"{label} survivor owner is required")
        if not isinstance(row.get("rationale"), str) or not row.get("rationale"):
            failures.append(f"{label} survivor rationale is required")
        resolution_ref = row.get("resolution_ref")
        if not isinstance(resolution_ref, str) or not _safe_relative_path(resolution_ref):
            failures.append(f"{label} survivor requires a durable workspace-relative resolution_ref")
    if actual != expected:
        failures.append("survivor rows must be derived exactly from survived mutation outcomes")
    return rows


def _validate_coverage(value: object, failures: list[str]) -> None:
    expected = {
        "runs": 0,
        "line_percent": None,
        "branch_percent": None,
        "posture": "adequacy_signal_not_release_safety_proof",
    }
    if value != expected:
        failures.append("coverage must record zero runs and no fabricated percentages")


def _validate_summary(
    value: object,
    shards: list[dict[str, Any]],
    survivors: list[dict[str, Any]],
    failures: list[str],
) -> None:
    if not isinstance(value, Mapping):
        failures.append("summary must be an object")
        return
    results = [result for shard in shards for result in _mapping_rows(shard.get("results"))]
    expected = {
        "shards": len(shards),
        "measured_shards": sum(row.get("execution_status") == "measured" for row in shards),
        "planned_shards": sum(row.get("execution_status") == "planned" for row in shards),
        "defined_mutants": sum(
            len(row.get("mutations", [])) if isinstance(row.get("mutations"), list) else 0
            for row in shards
        ),
        "measured_mutants": len(results),
        **{name: sum(row.get("result") == name for row in results) for name in RESULTS},
    }
    if value != expected:
        failures.append("summary does not equal values recomputed from shard outcomes")
    if expected["survived"] != len(survivors):
        failures.append("summary survivor count does not match resolved survivor rows")


def _outcome_has_infrastructure_failure(row: Mapping[str, Any]) -> bool:
    return has_infrastructure_failure(
        row.get("build_exit_status"), str(row.get("build_output_tail", ""))
    ) or has_infrastructure_failure(
        row.get("test_exit_status"), str(row.get("test_output_tail", ""))
    )


def _valid_artifact(value: object) -> bool:
    return (
        isinstance(value, Mapping)
        and _safe_relative_path(str(value.get("path", "")))
        and DIGEST_RE.fullmatch(str(value.get("sha256", ""))) is not None
    )


def _safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return bool(path.parts) and not path.is_absolute() and ".." not in path.parts and "." not in path.parts


def _closed_schema(
    schema: Mapping[str, Any],
    fields: set[str],
    label: str,
    failures: list[str],
) -> None:
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append(f"mutation survivor report schema {label} must be a closed object")
    required = schema.get("required")
    required_set = _string_set(required)
    if required_set != fields:
        failures.append(f"mutation survivor report schema {label} required fields drifted")
    if set(_properties(schema)) != fields:
        failures.append(f"mutation survivor report schema {label} properties drifted")


def _properties(schema: object) -> Mapping[str, Any]:
    if not isinstance(schema, Mapping):
        return {}
    value = schema.get("properties")
    return value if isinstance(value, Mapping) else {}


def _property_schema(properties: Mapping[str, Any], key: str) -> Mapping[str, Any]:
    value = properties.get(key)
    return value if isinstance(value, Mapping) else {}


def _mapping_rows(value: object) -> list[Mapping[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, Mapping)]


def _string_set(value: object) -> set[str] | None:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        return None
    return set(value)


def _is_unique_nonempty_string_list(value: object) -> bool:
    return (
        isinstance(value, list)
        and bool(value)
        and all(isinstance(item, str) and item for item in value)
        and len(value) == len(set(value))
    )


def _schema_consts(
    schema: object,
    expected: Mapping[str, Any],
    label: str,
    failures: list[str],
) -> None:
    properties = _properties(schema)
    actual = {
        field: value.get("const") if isinstance(value, Mapping) else None
        for field, value in properties.items()
    }
    if actual != dict(expected):
        failures.append(f"mutation survivor report schema {label} consts drifted")
