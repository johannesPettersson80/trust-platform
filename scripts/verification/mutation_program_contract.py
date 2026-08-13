"""Closed static contract for the focused Phase 10 mutation program."""

from __future__ import annotations

import hashlib
import json
import subprocess
import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .focused_mutation_artifact import (
    canonical_json,
    validate_execution_artifact,
    validate_execution_artifact_source,
)
from .metadata_validator.mutation_contracts import load_mutation_contract
from .metadata_validator.mutation_reports import validate_mutation_report
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_rust import scan_rust_file
from .test_catalog_surfaces import scan_conformance
from .test_catalog_validation import check_supported_schema_keywords


MUTATION_PROGRAM_PATH = "verification/mutation-program.toml"
MUTATION_PROGRAM_SCHEMA_PATH = "verification/schemas/mutation-program.schema.json"
MUTATION_PROGRAM_SCHEMA_SEMANTIC_DIGEST = (
    "5aa5e3fb5871e52337746f0679f9ad30cc2a3e15b154dc8a572523b4cdeacc43"
)
LEGACY_TEST_ID = "TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"
LEGACY_REPORT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-08/"
    "p1b-bytecode-validator-mutation-report.json"
)
REQUIRED_SHARD_IDS = (
    "MUTATION_SHARD_BYTECODE_VALIDATOR_001",
    "MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001",
    "MUTATION_SHARD_HIR_DIAGNOSTICS_001",
    "MUTATION_SHARD_PARSER_RECOVERY_001",
    "MUTATION_SHARD_RETAIN_RESTART_001",
    "MUTATION_SHARD_CONNECTOR_STATUS_PROJECTION_001",
)
ROOT_CONSTS = {
    "schema_version": 1,
    "id": "MUTATION_PROGRAM_V1",
    "title": "Focused mutation adequacy program",
    "status": "mapped",
    "owner": "verification",
    "proof_posture": "adequacy_only_no_proof",
    "coverage_posture": "adequacy_signal_not_safety_proof",
    "execution_posture": "report_only",
    "max_mutants_per_shard": 2,
    "last_reviewed": "2026-07-16",
}
ROOT_FIELDS = {*ROOT_CONSTS, "survivor_policy", "survivor_resolutions", "shards"}
SURVIVOR_POLICY = {
    "allowed_actions": [
        "add_test",
        "unreachable_defensive_rationale",
        "dead_code_removal",
    ],
    "resolution_status_required": "resolved",
    "durable_resolution_ref_required": True,
}
SHARD_FIELDS = {
    "id",
    "title",
    "area",
    "invariant_ids",
    "execution_status",
    "association_semantics",
    "delivered_build_requirement",
    "owner",
    "result_artifact_path",
    "legacy_catalog_test_id",
    "legacy_report_path",
    "delivered_binary_path",
    "delivered_confirmation",
    "mutations",
    "associated_tests",
}
RESULT_ARTIFACT_PATHS = {
    REQUIRED_SHARD_IDS[1]: "docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-runtime-value-conversion-mutation.json",
    REQUIRED_SHARD_IDS[2]: "docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-hir-subrange-diagnostics-mutation.json",
    REQUIRED_SHARD_IDS[3]: "docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-parser-recovery-mutation.json",
    REQUIRED_SHARD_IDS[4]: "docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-retain-restart-mutation.json",
}
MUTATION_FIELDS = {
    "id",
    "source_file",
    "source_digest",
    "function",
    "genre",
    "replacement",
    "selector_name",
    "build_command",
    "test_command",
    "association_ids",
}
GENERATED_MUTATION_FIELDS = {"source_digest", "selector_name"}
ASSOCIATION_FIELDS = {"id_kind", "id", "source_kind", "path", "name", "ignore_state"}
SURVIVOR_RESOLUTION_FIELDS = {
    "shard_id",
    "mutation_id",
    "owner",
    "action",
    "resolution_status",
    "rationale",
    "resolution_ref",
}


def _mutation(
    mutation_id: str,
    source_file: str,
    function: str,
    genre: str,
    replacement: str,
    build_command: list[str],
    test_command: list[str],
    association_ids: list[str],
) -> dict[str, Any]:
    return {
        "id": mutation_id,
        "source_file": source_file,
        "function": function,
        "genre": genre,
        "replacement": replacement,
        "build_command": build_command,
        "test_command": test_command,
        "association_ids": association_ids,
    }


REVIEWED_SHARDS: dict[str, dict[str, Any]] = {
    REQUIRED_SHARD_IDS[0]: {
        "title": "Bytecode validator",
        "area": "bytecode_vm",
        "invariant_ids": ["VM_SEAM_VALID_001"],
        "execution_status": "measured",
        "owner": "trust-runtime",
        "delivered_build_requirement": "not_applicable_source_mutation",
        "mutations": [
            _mutation(
                "MUTANT_VALIDATE_INSTRUCTION_STREAM_BYPASS",
                "crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs",
                "validate_instruction_stream",
                "FnValue",
                "Ok(())",
                ["cargo", "test", "-p", "trust-runtime", "--test", "bytecode_validation", "--no-run"],
                ["cargo", "test", "-p", "trust-runtime", "--test", "bytecode_validation"],
                [
                    "VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_FF_3A193633",
                    "VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_80_36D182BA",
                    "VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND_100_DF169D85",
                    "VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND__100_4555B79F",
                ],
            ),
            _mutation(
                "MUTANT_VALIDATE_STACK_SHAPE_BYPASS",
                "crates/trust-runtime/src/bytecode/validate/stack_shape.rs",
                "validate_stack_shape",
                "FnValue",
                "Ok(())",
                ["cargo", "test", "-p", "trust-runtime", "--test", "bytecode_vm_core", "--no-run"],
                [
                    "cargo", "test", "-p", "trust-runtime", "--test", "bytecode_vm_core",
                    "vm_rejects_stack_underflow", "--", "--exact",
                ],
                ["VM_SEAM_VALID_001_STACK_UNDERFLOW_POU_BODY_POP_EMPTY_STACK_443E654C"],
            ),
        ],
    },
    REQUIRED_SHARD_IDS[1]: {
        "title": "Runtime value and type conversion",
        "area": "bytecode_vm",
        "invariant_ids": ["VM_SEAM_DECLARED_TYPE_001"],
        "execution_status": "measured",
        "owner": "trust-runtime",
        "result_artifact_path": RESULT_ARTIFACT_PATHS[REQUIRED_SHARD_IDS[1]],
        "delivered_build_requirement": "not_applicable_source_mutation",
        "mutations": [
            _mutation(
                "MUTANT_RUNTIME_CONVERT_VALUE_IDENTITY_COMPARISON",
                "crates/trust-runtime/src/stdlib/conversions/dispatch.rs",
                "convert_value",
                "BinaryOperator",
                "!=",
                ["cargo", "test", "-p", "trust-runtime", "--test", "stdlib_conv", "--no-run"],
                [
                    "cargo", "test", "-p", "trust-runtime", "--test", "stdlib_conv",
                    "conversion_functions", "--", "--exact",
                ],
                ["DISC_11144C19050BB255758E"],
            )
        ],
    },
    REQUIRED_SHARD_IDS[2]: {
        "title": "HIR subrange diagnostics",
        "area": "compiler_iec",
        "invariant_ids": ["IEC_SUBRANGE_001"],
        "execution_status": "measured",
        "owner": "trust-hir",
        "result_artifact_path": RESULT_ARTIFACT_PATHS[REQUIRED_SHARD_IDS[2]],
        "delivered_build_requirement": "not_applicable_source_mutation",
        "mutations": [
            _mutation(
                "MUTANT_HIR_SUBRANGE_DIAGNOSTIC_NOOP",
                "crates/trust-hir/src/type_check/stmt_impl_part_04.rs",
                "StmtChecker<'a, 'b>::check_subrange_assignment",
                "FnValue",
                "()",
                ["cargo", "test", "-p", "trust-hir", "--test", "semantic_type_checking", "--no-run"],
                [
                    "cargo", "test", "-p", "trust-hir", "--test", "semantic_type_checking",
                    "basics_and_warnings::test_subrange_assignment_out_of_range", "--", "--exact",
                ],
                ["DISC_44D2AECEE13EB41E3AA0"],
            )
        ],
    },
    REQUIRED_SHARD_IDS[3]: {
        "title": "Parser recovery",
        "area": "compiler_iec",
        "invariant_ids": ["IEC_PARSE_RECOVER_001"],
        "execution_status": "measured",
        "owner": "trust-syntax",
        "result_artifact_path": RESULT_ARTIFACT_PATHS[REQUIRED_SHARD_IDS[3]],
        "delivered_build_requirement": "not_applicable_source_mutation",
        "mutations": [
            _mutation(
                "MUTANT_PARSER_RECOVERY_EOF_COMPARISON",
                "crates/trust-syntax/src/parser/parser.rs",
                "Parser<'t, 'src>::recover_top_level_until",
                "BinaryOperator",
                "!=",
                ["cargo", "test", "-p", "trust-syntax", "--lib", "--no-run"],
                [
                    "cargo", "test", "-p", "trust-syntax", "--lib",
                    "parser::parser::tests::test_bounded_recovery_does_not_close_on_rparen_inside_unclosed_bracket",
                    "--", "--exact",
                ],
                ["DISC_3C5D1667AD9E1B86A142"],
            )
        ],
    },
    REQUIRED_SHARD_IDS[4]: {
        "title": "Retain and restart",
        "area": "runtime_safety",
        "invariant_ids": ["RT_SAFE_RESTART_001", "RT_SAFE_RETAIN_001"],
        "execution_status": "measured",
        "owner": "trust-runtime",
        "result_artifact_path": RESULT_ARTIFACT_PATHS[REQUIRED_SHARD_IDS[4]],
        "delivered_build_requirement": "not_applicable_source_mutation",
        "mutations": [
            _mutation(
                "MUTANT_RETAIN_ON_WARM_FALSE",
                "crates/trust-runtime/src/runtime/retain_snapshot.rs",
                "retain_on_warm",
                "FnValue",
                "false",
                ["cargo", "test", "-p", "trust-runtime", "--test", "runtime_restart", "--no-run"],
                [
                    "cargo", "test", "-p", "trust-runtime", "--test", "runtime_restart",
                    "warm_restart_preserves_retain_and_initializes_nonretain", "--", "--exact",
                ],
                ["DISC_83FE96819CB25F21CA77"],
            )
        ],
    },
    REQUIRED_SHARD_IDS[5]: {
        "title": "Connector status projection",
        "area": "protocols",
        "invariant_ids": ["PROTO_STATUS_TRUTH_001"],
        "execution_status": "planned",
        "owner": "trust-runtime",
        "delivered_build_requirement": "required_before_execution",
        "delivered_binary_path": "{target_dir}/debug/trust-runtime",
        "delivered_confirmation": ["artifact_sha256", "direct_execution"],
        "mutations": [
            _mutation(
                "MUTANT_ADS_STATUS_DEGRADED_COMPARISON",
                "crates/trust-runtime/src/connectors/mapping.rs",
                "ads_connection_status_state",
                "BinaryOperator",
                "!=",
                ["cargo", "build", "-p", "trust-runtime", "--bin", "trust-runtime"],
                [
                    "{delivered_binary}", "conformance", "--suite-root", "conformance", "--filter",
                    "cfm_comms_determinism_connector_projection_001", "--output",
                    "target/gate-artifacts/verification/p10-connector-status-projection.json",
                ],
                ["DISC_FA437CAA77C99B81B753"],
            )
        ],
    },
}

REVIEWED_SCANNER_ASSOCIATIONS = {
    REQUIRED_SHARD_IDS[1]: (
        "DISC_11144C19050BB255758E", "rust_integration_test",
        "crates/trust-runtime/tests/stdlib_conv.rs", "conversion_functions",
    ),
    REQUIRED_SHARD_IDS[2]: (
        "DISC_44D2AECEE13EB41E3AA0", "rust_integration_test",
        "crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs",
        "test_subrange_assignment_out_of_range",
    ),
    REQUIRED_SHARD_IDS[3]: (
        "DISC_3C5D1667AD9E1B86A142", "rust_unit_test",
        "crates/trust-syntax/src/parser/parser.rs",
        "test_bounded_recovery_does_not_close_on_rparen_inside_unclosed_bracket",
    ),
    REQUIRED_SHARD_IDS[4]: (
        "DISC_83FE96819CB25F21CA77", "rust_integration_test",
        "crates/trust-runtime/tests/runtime_restart.rs",
        "warm_restart_preserves_retain_and_initializes_nonretain",
    ),
    REQUIRED_SHARD_IDS[5]: (
        "DISC_FA437CAA77C99B81B753", "conformance_case",
        "conformance/cases/comms_determinism/cfm_comms_determinism_connector_projection_001/manifest.toml",
        "cfm_comms_determinism_connector_projection_001",
    ),
}


def load_mutation_program(root: Path) -> dict[str, Any]:
    """Load the reviewed program manifest."""

    return tomllib.loads((root / MUTATION_PROGRAM_PATH).read_text())


def validate_mutation_program_contract(root: Path, program: Any) -> list[str]:
    """Validate the closed manifest without invoking cargo-mutants."""

    return _validate_mutation_program_contract(root, program)


def validate_mutation_program_for_shard_execution(
    root: Path,
    program: Any,
    shard_id: str,
) -> list[str]:
    """Validate reviewed inputs without requiring the artifacts being refreshed."""

    if shard_id not in REQUIRED_SHARD_IDS[1:5]:
        return [f"mutation shard {shard_id} is outside the reviewed source-runner set"]
    return _validate_mutation_program_contract(
        root,
        program,
        skip_execution_artifacts=True,
    )


def _validate_mutation_program_contract(
    root: Path,
    program: Any,
    *,
    artifact_exempt_shard_id: str | None = None,
    skip_execution_artifacts: bool = False,
) -> list[str]:
    failures: list[str] = []
    if not isinstance(program, Mapping):
        return ["mutation program root must be a table"]
    try:
        schema = json.loads((root / MUTATION_PROGRAM_SCHEMA_PATH).read_text())
    except Exception as exc:
        return [f"mutation program schema cannot be read: {exc}"]
    failures.extend(_validate_schema(schema))
    try:
        failures.extend(validate_json_schema_instance(dict(program), schema))
    except Exception as exc:
        failures.append(f"mutation program schema validation failed safely: {exc}")

    if set(program) != ROOT_FIELDS:
        failures.append("mutation program root fields drift from contract")
    for field, expected in ROOT_CONSTS.items():
        if program.get(field) != expected:
            label = field.replace("_", " ")
            failures.append(f"mutation program {label} must equal {expected!r}")
    if program.get("survivor_policy") != SURVIVOR_POLICY:
        failures.append("mutation survivor policy must require a resolved allowed action and durable ref")

    shards = program.get("shards")
    if not isinstance(shards, list):
        failures.append("mutation program shards must be an array")
        return sorted(set(failures))
    shard_ids = [row.get("id") if isinstance(row, Mapping) else None for row in shards]
    if shard_ids != list(REQUIRED_SHARD_IDS):
        failures.append("mutation program shards must match the exact reviewed order")
    if [len(row.get("mutations", [])) if isinstance(row, Mapping) and isinstance(row.get("mutations"), list) else 0 for row in shards] != [2, 1, 1, 1, 1, 1]:
        failures.append("mutation program exceeds the focused maximum or changes reviewed shard sizes")

    invariants = _load_invariants(root, failures)
    for index, row in enumerate(shards):
        _validate_shard(root, row, index, invariants, failures)
    try:
        json.dumps(program, sort_keys=True, separators=(",", ":"), allow_nan=False)
        artifact_inputs_usable = True
    except (TypeError, ValueError) as exc:
        failures.append(f"mutation program contains a non-JSON-compatible value: {exc}")
        artifact_inputs_usable = False
    compare_survivors_to_artifacts = not skip_execution_artifacts and artifact_inputs_usable
    if not compare_survivors_to_artifacts:
        legacy_survivor_ids: set[tuple[str, str]] = set()
        focused_survivor_ids: set[tuple[str, str]] = set()
    else:
        legacy_survivor_ids = _validate_legacy_shard(root, shards, failures)
        focused_survivor_ids = _validate_focused_shards(
            root,
            shards,
            failures,
            artifact_exempt_shard_id=artifact_exempt_shard_id,
        )
    _validate_survivor_resolutions(
        root,
        program.get("survivor_resolutions"),
        legacy_survivor_ids | focused_survivor_ids if compare_survivors_to_artifacts else None,
        {
            (shard.get("id"), mutation.get("id"))
            for shard in shards
            if isinstance(shard, Mapping)
            and isinstance(shard.get("id"), str)
            for mutation in (
                shard.get("mutations")
                if isinstance(shard.get("mutations"), list)
                else []
            )
            if isinstance(mutation, Mapping)
            and isinstance(mutation.get("id"), str)
        },
        failures,
    )
    _validate_scanner_associations(root, shards, failures)
    return sorted(set(failures))


def _validate_schema(schema: Any) -> list[str]:
    failures: list[str] = []
    if not isinstance(schema, Mapping):
        return ["mutation program schema root must be an object"]
    check_supported_schema_keywords(dict(schema), "$", failures)
    semantic = hashlib.sha256(
        json.dumps(schema, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    if semantic != MUTATION_PROGRAM_SCHEMA_SEMANTIC_DIGEST:
        failures.append("mutation program schema semantic digest drifts")
    if schema.get("additionalProperties") is not False:
        failures.append("mutation program schema root must be closed")
    properties = schema.get("properties")
    if not isinstance(properties, Mapping):
        return failures + ["mutation program schema properties drift"]
    shard_schema = properties.get("shards")
    if not isinstance(shard_schema, Mapping) or shard_schema.get("minItems") != 6 or shard_schema.get("maxItems") != 6:
        failures.append("mutation program schema shard cardinality drifts")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        failures.append("mutation program schema definitions drift")
    else:
        for name in (
            "survivor_policy",
            "survivor_resolution",
            "shard",
            "mutation",
            "associated_test",
        ):
            definition = definitions.get(name)
            if not isinstance(definition, Mapping) or definition.get("additionalProperties") is not False:
                failures.append(f"mutation program schema {name} must be a closed object")
    return failures


def _validate_shard(
    root: Path,
    value: Any,
    index: int,
    invariants: Mapping[str, str],
    failures: list[str],
) -> None:
    if not isinstance(value, Mapping):
        failures.append(f"mutation shard {index} must be a table")
        return
    shard_id = value.get("id")
    label = shard_id if isinstance(shard_id, str) else f"shards[{index}]"
    if not set(value).issubset(SHARD_FIELDS):
        failures.append(f"mutation shard {label} fields drift from contract")
    if not isinstance(shard_id, str):
        failures.append(f"mutation shard {label} id must be a string")
        return
    expected = REVIEWED_SHARDS.get(shard_id)
    if expected is None:
        failures.append(f"mutation shard {label} is not reviewed")
        return
    for field, expected_value in expected.items():
        if field == "mutations":
            _validate_reviewed_mutations(
                label,
                value.get(field),
                expected_value,
                failures,
            )
        elif value.get(field) != expected_value:
            failures.append(f"mutation shard {label} {field} drifts from reviewed contract")
    if value.get("association_semantics") != "association_only_not_execution_claim":
        failures.append(f"mutation shard {label} must remain association-only")
    if shard_id == REQUIRED_SHARD_IDS[0]:
        if value.get("legacy_catalog_test_id") != LEGACY_TEST_ID or value.get("legacy_report_path") != LEGACY_REPORT_PATH:
            failures.append("measured bytecode shard must bind the exact legacy catalog test and report")
    elif "legacy_catalog_test_id" in value or "legacy_report_path" in value:
        failures.append(f"planned mutation shard {label} cannot claim legacy measurement")
    if shard_id != REQUIRED_SHARD_IDS[5] and (
        "delivered_binary_path" in value or "delivered_confirmation" in value
    ):
        failures.append(f"mutation shard {label} cannot claim delivered-build confirmation")
    artifact_path = value.get("result_artifact_path")
    if shard_id in RESULT_ARTIFACT_PATHS:
        if artifact_path != RESULT_ARTIFACT_PATHS[shard_id]:
            failures.append(f"mutation shard {label} result artifact path drifts")
    elif artifact_path is not None:
        failures.append(f"mutation shard {label} cannot reserve a focused result artifact path")

    invariant_ids = value.get("invariant_ids")
    if isinstance(invariant_ids, list):
        for invariant_id in invariant_ids:
            if not isinstance(invariant_id, str) or invariant_id not in invariants:
                failures.append(f"mutation shard {label} references unknown invariant {invariant_id!r}")
            elif invariants[invariant_id] != value.get("area"):
                failures.append(f"mutation shard {label} invariant {invariant_id} area mismatch")
    mutations = value.get("mutations")
    if not isinstance(mutations, list):
        failures.append(f"mutation shard {label} mutations must be an array")
        return
    if len(mutations) > 2:
        failures.append(f"mutation shard {label} exceeds the focused maximum of two mutants")
    for mutation in mutations:
        _validate_mutation(root, label, mutation, failures)
    associations = value.get("associated_tests")
    if not isinstance(associations, list) or not associations:
        failures.append(f"mutation shard {label} must have associated tests")
    else:
        for association in associations:
            if not isinstance(association, Mapping) or set(association) != ASSOCIATION_FIELDS:
                failures.append(f"mutation shard {label} association fields drift")
        association_ids = [
            association.get("id")
            for association in associations
            if isinstance(association, Mapping)
        ]
        mutation_association_ids: list[str] = []
        for mutation in mutations:
            if not isinstance(mutation, Mapping):
                continue
            values = mutation.get("association_ids")
            if isinstance(values, list):
                mutation_association_ids.extend(
                    item for item in values if isinstance(item, str)
                )
        if mutation_association_ids != association_ids:
            failures.append(
                f"mutation shard {label} mutant association_ids must partition associated tests exactly"
            )


def _validate_reviewed_mutations(
    shard_id: str,
    actual: Any,
    expected: Any,
    failures: list[str],
) -> None:
    if not isinstance(actual, list) or not isinstance(expected, list):
        failures.append(f"mutation shard {shard_id} mutations drift from reviewed contract")
        return
    if len(actual) != len(expected):
        failures.append(f"mutation shard {shard_id} mutations drift from reviewed contract")
        return
    for index, (actual_row, expected_row) in enumerate(zip(actual, expected, strict=True)):
        if not isinstance(actual_row, Mapping) or not isinstance(expected_row, Mapping):
            failures.append(
                f"mutation shard {shard_id} mutation[{index}] drifts from reviewed contract"
            )
            continue
        actual_semantics = {
            key: value
            for key, value in actual_row.items()
            if key not in GENERATED_MUTATION_FIELDS
        }
        if actual_semantics != expected_row:
            mutation_id = actual_row.get("id", f"mutation[{index}]")
            failures.append(
                f"mutation shard {shard_id} {mutation_id} semantics drift from reviewed contract"
            )


def _validate_mutation(root: Path, shard_id: str, value: Any, failures: list[str]) -> None:
    if not isinstance(value, Mapping):
        failures.append(f"mutation shard {shard_id} has a non-table mutant")
        return
    mutation_id = value.get("id", "<unknown>")
    if set(value) != MUTATION_FIELDS:
        failures.append(f"mutation {mutation_id} fields drift from contract")
    source = _safe_tracked_file(root, value.get("source_file"), f"mutation {mutation_id}", failures)
    if source is not None:
        actual = "sha256:" + hashlib.sha256(source.read_bytes()).hexdigest()
        if value.get("source_digest") != actual:
            failures.append(f"mutation {mutation_id} source_digest mismatch")
    for field in ("build_command", "test_command"):
        command = value.get(field)
        if not isinstance(command, list) or not all(isinstance(arg, str) and arg for arg in command):
            failures.append(f"mutation {mutation_id} {field} must be a string array")
            continue
        joined = " ".join(command)
        if any(broad in joined for broad in ("just test", "test-all", "--workspace", "--all-targets")):
            failures.append(f"mutation {mutation_id} {field} must remain focused")
    association_ids = value.get("association_ids")
    if (
        not isinstance(association_ids, list)
        or not association_ids
        or not all(isinstance(item, str) and item for item in association_ids)
        or len(association_ids) != len(set(association_ids))
    ):
        failures.append(
            f"mutation {mutation_id} association_ids must be unique non-empty strings"
        )


def _validate_legacy_shard(
    root: Path,
    shards: list[Any],
    failures: list[str],
) -> set[tuple[str, str]]:
    if not shards or not isinstance(shards[0], Mapping):
        return set()
    try:
        contract = load_mutation_contract(LEGACY_TEST_ID, root=root)
        report = json.loads((root / LEGACY_REPORT_PATH).read_text())
    except Exception as exc:
        failures.append(f"measured bytecode mutation contract cannot be loaded: {exc}")
        return set()
    for message in validate_mutation_report(report, contract):
        failures.append(f"measured bytecode mutation report: {message}")
    if not isinstance(report, Mapping):
        failures.append("measured bytecode legacy report root must be an object")
        return set()
    row = shards[0]
    mutations = row.get("mutations")
    if isinstance(mutations, list):
        manifest_core = [
            {
                **{
                    field: item.get(field)
                    for field in (
                        "id",
                        "source_file",
                        "function",
                        "genre",
                        "replacement",
                        "build_command",
                        "test_command",
                    )
                },
                "association_ids": item.get("association_ids"),
            }
            for item in mutations if isinstance(item, Mapping)
        ]
        legacy_core = [
            {
                "id": item.id,
                "source_file": item.source_file,
                "function": item.function,
                "genre": item.genre,
                "replacement": item.replacement,
                "build_command": list(item.build_command),
                "test_command": list(item.test_command),
                "association_ids": list(item.related_case_ids),
            }
            for item in contract.mutations
        ]
        if manifest_core != legacy_core:
            failures.append("measured bytecode shard does not exactly match its catalog mutation selectors")
    associated = row.get("associated_tests")
    associated_ids = [item.get("id") for item in associated if isinstance(item, Mapping)] if isinstance(associated, list) else []
    legacy_case_ids = [case_id for mutation in contract.mutations for case_id in mutation.related_case_ids]
    if associated_ids != legacy_case_ids:
        failures.append("measured bytecode shard associations do not match committed case IDs")
    summary = report.get("summary")
    if (
        report.get("status") != "complete"
        or report.get("test_id") != LEGACY_TEST_ID
        or report.get("shard_id") != REQUIRED_SHARD_IDS[0]
        or not isinstance(summary, Mapping)
        or summary.get("total") != len(contract.mutations)
    ):
        failures.append("measured bytecode legacy report does not match its focused shard")
    survivors = report.get("survivors")
    if not isinstance(survivors, list):
        return set()
    return {
        (REQUIRED_SHARD_IDS[0], item.get("id"))
        for item in survivors
        if isinstance(item, Mapping) and isinstance(item.get("id"), str)
    }


def _validate_focused_shards(
    root: Path,
    shards: list[Any],
    failures: list[str],
    *,
    artifact_exempt_shard_id: str | None = None,
) -> set[tuple[str, str]]:
    survivors: set[tuple[str, str]] = set()
    for shard in shards[1:5]:
        if not isinstance(shard, Mapping) or shard.get("execution_status") != "measured":
            continue
        shard_id = shard.get("id")
        label = shard_id if isinstance(shard_id, str) else "unknown focused shard"
        if shard_id == artifact_exempt_shard_id:
            continue
        artifact = _safe_tracked_file(
            root,
            shard.get("result_artifact_path"),
            f"measured mutation shard {label} result artifact",
            failures,
        )
        if artifact is None:
            failures.append(f"{label} focused mutation artifact cannot be loaded")
            continue
        try:
            raw = artifact.read_bytes()
            payload = json.loads(raw)
        except (OSError, json.JSONDecodeError) as exc:
            failures.append(f"{label} focused mutation artifact cannot be loaded: {exc}")
            continue
        if not isinstance(payload, Mapping):
            failures.append(f"{label} focused mutation artifact root must be an object")
            continue
        if canonical_json(payload).encode() != raw:
            failures.append(f"{label} focused mutation artifact must use canonical JSON")
        failures.extend(
            f"{label} focused mutation artifact: {message}"
            for message in validate_execution_artifact(root, payload, shard)
        )
        failures.extend(
            f"{label} focused mutation artifact: {message}"
            for message in validate_execution_artifact_source(root, payload, shard)
        )
        outcomes = payload.get("mutations")
        if isinstance(outcomes, list):
            survivors.update(
                (label, row["id"])
                for row in outcomes
                if isinstance(row, Mapping)
                and isinstance(row.get("id"), str)
                and row.get("result") == "survived"
            )
    return survivors


def _validate_survivor_resolutions(
    root: Path,
    value: Any,
    expected_ids: set[tuple[str, str]] | None,
    known_ids: set[tuple[Any, Any]],
    failures: list[str],
) -> None:
    if not isinstance(value, list):
        failures.append("mutation survivor resolutions must be an array")
        return
    actual_ids: set[tuple[Any, Any]] = set()
    for index, item in enumerate(value):
        label = f"mutation survivor_resolutions[{index}]"
        if not isinstance(item, Mapping):
            failures.append(f"{label} must be a table")
            continue
        if set(item) != SURVIVOR_RESOLUTION_FIELDS:
            failures.append(f"{label} fields drift from contract")
        shard_id = item.get("shard_id")
        mutation_id = item.get("mutation_id")
        if not isinstance(shard_id, str) or not isinstance(mutation_id, str):
            failures.append(f"{label} shard_id and mutation_id must be strings")
        else:
            key = (shard_id, mutation_id)
            if key in actual_ids:
                failures.append(f"{label} duplicates a survivor resolution")
            actual_ids.add(key)
            if key not in known_ids:
                failures.append(f"{label} references an unknown mutation")
        action = item.get("action")
        if not isinstance(action, str) or action not in SURVIVOR_POLICY["allowed_actions"]:
            failures.append(f"{label} action is not allowed")
        if item.get("resolution_status") != "resolved":
            failures.append(f"{label} must be resolved")
        for field in ("owner", "rationale"):
            if not isinstance(item.get(field), str) or not item.get(field):
                failures.append(f"{label} {field} must be non-empty text")
        resolution_ref = item.get("resolution_ref")
        if _safe_tracked_file(root, resolution_ref, label, failures) is not None:
            ignored = subprocess.run(
                [
                    "git",
                    "-C",
                    str(root),
                    "check-ignore",
                    "--no-index",
                    "-q",
                    "--",
                    str(resolution_ref),
                ],
                check=False,
                capture_output=True,
            )
            if ignored.returncode == 0:
                failures.append(f"{label} resolution_ref must not match an ignore rule")
            elif ignored.returncode != 1:
                failures.append(f"{label} resolution_ref ignore status could not be checked")
    if expected_ids is not None and actual_ids != expected_ids:
        failures.append(
            "mutation survivor resolutions must match measured survivors exactly"
        )


def _validate_scanner_associations(root: Path, shards: list[Any], failures: list[str]) -> None:
    facts = _scan_reviewed_facts(root, failures)
    for row in shards[1:]:
        if not isinstance(row, Mapping):
            continue
        shard_id = row.get("id")
        if not isinstance(shard_id, str):
            failures.append("planned mutation shard id must be a string")
            continue
        expected = REVIEWED_SCANNER_ASSOCIATIONS.get(shard_id)
        associations = row.get("associated_tests")
        if expected is None or not isinstance(associations, list) or len(associations) != 1:
            failures.append(f"planned mutation shard {shard_id} must have one reviewed scanner association")
            continue
        association = associations[0]
        if not isinstance(association, Mapping):
            failures.append(f"planned mutation shard {shard_id} association must be a table")
            continue
        expected_row = {
            "id_kind": "scanner_discovery_id",
            "id": expected[0],
            "source_kind": expected[1],
            "path": expected[2],
            "name": expected[3],
            "ignore_state": "not_ignored",
        }
        if dict(association) != expected_row:
            failures.append(f"planned mutation shard {shard_id} scanner association drifts")
        fact = facts.get(expected[0])
        if fact is None:
            failures.append(f"planned mutation shard {shard_id} discovery ID is absent from live scanner facts")
        elif any(fact.get(field) != expected_row[field] for field in ("source_kind", "path", "name", "ignore_state")):
            failures.append(f"planned mutation shard {shard_id} live scanner identity mismatch")


def _scan_reviewed_facts(root: Path, failures: list[str]) -> dict[str, dict[str, Any]]:
    specs = (
        ("crates/trust-runtime/tests/stdlib_conv.rs", "trust-runtime", "rust_integration_test", "cargo test -p trust-runtime --test stdlib_conv"),
        ("crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs", "trust-hir", "rust_integration_test", "cargo test -p trust-hir --test semantic_type_checking"),
        ("crates/trust-syntax/src/parser/parser.rs", "trust-syntax", "rust_unit_test", "cargo test -p trust-syntax"),
        ("crates/trust-runtime/tests/runtime_restart.rs", "trust-runtime", "rust_integration_test", "cargo test -p trust-runtime --test runtime_restart"),
    )
    rows: list[dict[str, Any]] = []
    for relative, package, source_kind, command in specs:
        batch = scan_rust_file(
            root,
            root / relative,
            package=package,
            source_kind=source_kind,
            command_prefix=command,
            command_authority="conservative" if source_kind == "rust_integration_test" else "package_only",
        )
        failures.extend(
            f"mutation association scanner diagnostic: {item.message}"
            for item in batch.diagnostics if item.severity == "error"
        )
        rows.extend(fact.to_dict() for fact in batch.facts)
    conformance = scan_conformance(root)
    failures.extend(
        f"mutation association scanner diagnostic: {item.message}"
        for item in conformance.diagnostics if item.severity == "error"
    )
    rows.extend(fact.to_dict() for fact in conformance.facts)
    result: dict[str, dict[str, Any]] = {}
    for row in rows:
        stable_id = row.get("stable_id")
        if not isinstance(stable_id, str):
            failures.append("mutation association scanner emitted an invalid discovery ID")
            continue
        if stable_id in result:
            failures.append(
                f"mutation association scanner emitted duplicate discovery ID {stable_id}"
            )
            continue
        result[stable_id] = row
    return result


def _load_invariants(root: Path, failures: list[str]) -> dict[str, str]:
    result: dict[str, str] = {}
    base = root / "verification/invariants"
    try:
        paths = sorted(base.glob("*/*.toml"))
    except OSError as exc:
        failures.append(f"mutation invariant registry cannot be scanned: {exc}")
        return result
    for path in paths:
        try:
            row = tomllib.loads(path.read_text())
        except Exception as exc:
            failures.append(f"mutation invariant cannot be read at {path.relative_to(root)}: {exc}")
            continue
        invariant_id = row.get("id")
        area = row.get("area")
        if isinstance(invariant_id, str) and isinstance(area, str):
            result[invariant_id] = area
    return result


def _safe_tracked_file(root: Path, value: Any, label: str, failures: list[str]) -> Path | None:
    if not isinstance(value, str) or not value or "\\" in value:
        failures.append(f"{label} path must be a normalized workspace-relative string")
        return None
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        failures.append(f"{label} path escapes the workspace")
        return None
    candidate = root / relative
    try:
        if any(part.is_symlink() for part in (candidate, *candidate.parents) if part != root.parent):
            failures.append(f"{label} path cannot contain symlinks")
            return None
        resolved = candidate.resolve()
        resolved.relative_to(root.resolve())
    except (OSError, ValueError):
        failures.append(f"{label} path escapes the workspace")
        return None
    if not resolved.is_file():
        failures.append(f"{label} path is not a file: {value}")
        return None
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", value],
        check=False,
        capture_output=True,
        text=True,
    )
    if tracked.returncode != 0:
        failures.append(f"{label} path is not tracked: {value}")
        return None
    return resolved
