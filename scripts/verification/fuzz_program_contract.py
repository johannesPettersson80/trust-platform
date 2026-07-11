"""Closed static contract for the Phase 9 fuzz-program registry."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import tomllib
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from .gate_inventory import GateInventoryError, load_gate_inventory
from .fuzz_program_source_contract import validate_execution_source_bindings
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords


FUZZ_PROGRAM_PATH = "verification/fuzz-program.toml"
FUZZ_PROGRAM_SCHEMA_PATH = "verification/schemas/fuzz-program.schema.json"
FUZZ_PROGRAM_SCHEMA_SEMANTIC_DIGEST = (
    "0cc7ff88a59d0d4fca3e7d6c38f01075c5c6b74bec16184043f63552191f1a72"
)
REVIEWED_PROGRAM_TITLE = "PLC fuzz and malformed-input program inventory"
REQUIRED_SURFACE_IDS = (
    "st_lexer_parser",
    "hir_lowering_input",
    "plcopen_xml",
    "bytecode_container_instructions",
    "protocol_payloads",
    "config_files",
    "lsp_incremental_edits",
    "hmi_schema_payloads",
)
SURFACE_AREAS = {
    "st_lexer_parser": "compiler_iec",
    "hir_lowering_input": "compiler_iec",
    "plcopen_xml": "plcopen_devtools",
    "bytecode_container_instructions": "bytecode_vm",
    "protocol_payloads": "protocols",
    "config_files": "runtime_safety",
    "lsp_incremental_edits": "editor_safety",
    "hmi_schema_payloads": "hmi_ui",
}
REVIEWED_SURFACE_ROWS = (
    (
        "st_lexer_parser",
        "Structured Text lexer and parser",
        "compiler_iec",
        "Generated source text exercises lexical handling, parser recovery, and bounded diagnostics.",
    ),
    (
        "hir_lowering_input",
        "HIR and lowering input",
        "compiler_iec",
        "Generated parsed programs should exercise semantic queries and lowering inputs without treating parser survival as lowering evidence.",
    ),
    (
        "plcopen_xml",
        "PLCopen XML",
        "plcopen_devtools",
        "Malformed and vendor-shaped XML needs a dedicated import/export target; deterministic negative tests alone are not a fuzz target.",
    ),
    (
        "bytecode_container_instructions",
        "Bytecode container and instruction streams",
        "bytecode_vm",
        "Container structure and instruction streams need generated malformed-input pressure at the validator boundary.",
    ),
    (
        "protocol_payloads",
        "Protocol payloads",
        "protocols",
        "Framing, dispatch, and decoded payload boundaries need generated input independent of live hardware.",
    ),
    (
        "config_files",
        "Configuration files",
        "runtime_safety",
        "Configuration parsers need generated malformed documents; generated policy strings are not a substitute for file parsing.",
    ),
    (
        "lsp_incremental_edits",
        "LSP incremental edits",
        "editor_safety",
        "Protocol-level edit sequences need generated range, encoding, and lifecycle variation beyond lower-level HIR edit cycles.",
    ),
    (
        "hmi_schema_payloads",
        "HMI schema payloads",
        "hmi_ui",
        "HMI schema and value payloads need generated malformed shapes without implying browser or authorization behavior.",
    ),
)
TIER_IDS = ("pr_smoke", "nightly", "manual_extended")
TARGET_KINDS = ("cargo_fuzz", "bounded_rust_smoke")
ASSOCIATION_STRENGTHS = ("direct", "partial")
ENFORCEMENT_STATUSES = ("wired", "planned", "manual_only")
AREA_IDS = (
    "compiler_iec",
    "plcopen_devtools",
    "bytecode_vm",
    "protocols",
    "runtime_safety",
    "editor_safety",
    "hmi_ui",
)
ROOT_CONSTS = {
    "schema_version": 1,
    "id": "FUZZ_PROGRAM_V1",
    "status": "mapped",
    "owner": "verification",
    "proof_posture": "association_only",
    "execution_posture": "inventory_only",
    "mapping_basis": "explicit_live_identity_and_reviewed_surface_association_only",
    "last_reviewed": "2026-07-11",
}
REVIEWED_CORPUS_POLICY = {
    "working_corpus_storage": "machine_local_ignored",
    "raw_crash_storage": "machine_local_ignored",
    "durable_evidence_status": "not_durable",
    "contents_assessed": False,
    "tracked_generated_corpus_allowed": False,
    "rationale": "Generated working corpora and raw crash artifacts are local discovery inputs, not durable verification evidence. Their contents and counts are intentionally excluded from this reproducible inventory.",
}
REVIEWED_CRASH_HANDOFF = {
    "enforcement_status": "not_enforced",
    "required_disposition": "deterministic_regression",
    "p9_005_row_remains_open": True,
    "rationale": "The written program requires a minimized crash to become a deterministic regression, but no machine registry or exhaustive crash-to-regression join enforces that handoff yet.",
}
TARGET_ID_ORDER = (
    "FUZZ_TARGET_SYNTAX_PARSE",
    "FUZZ_TARGET_HIR_SEMANTIC",
    "FUZZ_TARGET_ADS_AMS_FRAME",
    "FUZZ_TARGET_ADS_BOUNDARY_NOOP",
    "FUZZ_TARGET_ADS_COMMAND_DISPATCH",
    "FUZZ_SMOKE_VM_MALFORMED_BYTECODE",
    "FUZZ_SMOKE_MESH_PAYLOAD",
    "FUZZ_SMOKE_SHM_HEADER",
    "FUZZ_SMOKE_RUNTIME_CLOUD_API",
    "FUZZ_SMOKE_WAN_ALLOWLIST",
    "FUZZ_SMOKE_PARSER_INITIALIZER_RECOVERY_PROPERTY",
)
ROOT_FIELDS = {
    "schema_version",
    "id",
    "title",
    "status",
    "owner",
    "proof_posture",
    "execution_posture",
    "mapping_basis",
    "last_reviewed",
    "corpus_policy",
    "crash_regression_handoff",
    "surfaces",
    "targets",
}
COMMON_TARGET_FIELDS = {
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
    "last_reviewed",
    "surface_associations",
}
CARGO_TARGET_FIELDS = COMMON_TARGET_FIELDS | {
    "manifest_path",
    "corpus_path",
    "artifact_path",
    "corpus_storage",
    "artifact_storage",
}
SMOKE_TARGET_FIELDS = COMMON_TARGET_FIELDS | {
    "discovery_id",
    "discovery_source_kind",
}
SURFACE_FIELDS = {"id", "title", "area", "rationale"}
ASSOCIATION_FIELDS = {"surface_id", "strength", "rationale"}
SCHEMA_TARGET_PROPERTIES = CARGO_TARGET_FIELDS | SMOKE_TARGET_FIELDS
FORBIDDEN_CLAIM_RE = re.compile(
    r"\b(?:proves?|proof of|validated|complete coverage|crash[- ]free)\b",
    re.IGNORECASE,
)


def _target_contract(
    kind: str,
    primary: str,
    additional: tuple[str, ...],
    enforcement: str,
    basis: tuple[str, ...],
    associations: tuple[tuple[str, str], ...],
) -> dict[str, Any]:
    return {
        "target_kind": kind,
        "primary_tier": primary,
        "additional_tiers": additional,
        "enforcement_status": enforcement,
        "execution_basis_ids": basis,
        "surface_associations": associations,
    }


REVIEWED_TARGET_CONTRACTS = {
    "FUZZ_TARGET_SYNTAX_PARSE": _target_contract(
        "cargo_fuzz",
        "pr_smoke",
        ("nightly",),
        "wired",
        (
            "GATE_SCRIPT_SALSA_FUZZ",
            "GATE_JOB_SALSA_FUZZ_SMOKE",
            "GATE_JOB_SALSA_FUZZ_EXTENDED_NIGHTLY",
        ),
        (("st_lexer_parser", "direct"),),
    ),
    "FUZZ_TARGET_HIR_SEMANTIC": _target_contract(
        "cargo_fuzz",
        "pr_smoke",
        ("nightly",),
        "wired",
        (
            "GATE_SCRIPT_SALSA_FUZZ",
            "GATE_JOB_SALSA_FUZZ_SMOKE",
            "GATE_JOB_SALSA_FUZZ_EXTENDED_NIGHTLY",
        ),
        (("hir_lowering_input", "partial"), ("lsp_incremental_edits", "partial")),
    ),
    "FUZZ_TARGET_ADS_AMS_FRAME": _target_contract(
        "cargo_fuzz", "manual_extended", (), "manual_only", (), (("protocol_payloads", "direct"),)
    ),
    "FUZZ_TARGET_ADS_BOUNDARY_NOOP": _target_contract(
        "cargo_fuzz", "manual_extended", (), "manual_only", (), (("protocol_payloads", "direct"),)
    ),
    "FUZZ_TARGET_ADS_COMMAND_DISPATCH": _target_contract(
        "cargo_fuzz", "manual_extended", (), "manual_only", (), (("protocol_payloads", "direct"),)
    ),
    "FUZZ_SMOKE_VM_MALFORMED_BYTECODE": _target_contract(
        "bounded_rust_smoke",
        "nightly",
        (),
        "planned",
        ("GATE_SCRIPT_RUNTIME_VM_MALFORMED_BYTECODE_FUZZ",),
        (("bytecode_container_instructions", "direct"),),
    ),
    "FUZZ_SMOKE_MESH_PAYLOAD": _target_contract(
        "bounded_rust_smoke",
        "pr_smoke",
        (),
        "wired",
        ("GATE_SCRIPT_RUNTIME_COMMS_FUZZ", "GATE_JOB_CI_CONFORMANCE"),
        (("protocol_payloads", "direct"),),
    ),
    "FUZZ_SMOKE_SHM_HEADER": _target_contract(
        "bounded_rust_smoke",
        "pr_smoke",
        (),
        "wired",
        ("GATE_SCRIPT_RUNTIME_COMMS_FUZZ", "GATE_JOB_CI_CONFORMANCE"),
        (("protocol_payloads", "direct"),),
    ),
    "FUZZ_SMOKE_RUNTIME_CLOUD_API": _target_contract(
        "bounded_rust_smoke",
        "pr_smoke",
        (),
        "wired",
        ("GATE_SCRIPT_RUNTIME_COMMS_FUZZ", "GATE_JOB_CI_CONFORMANCE"),
        (("protocol_payloads", "direct"),),
    ),
    "FUZZ_SMOKE_WAN_ALLOWLIST": _target_contract(
        "bounded_rust_smoke",
        "pr_smoke",
        (),
        "wired",
        ("GATE_SCRIPT_RUNTIME_COMMS_FUZZ", "GATE_JOB_CI_CONFORMANCE"),
        (("protocol_payloads", "partial"),),
    ),
    "FUZZ_SMOKE_PARSER_INITIALIZER_RECOVERY_PROPERTY": _target_contract(
        "bounded_rust_smoke",
        "pr_smoke",
        (),
        "wired",
        ("GATE_JOB_CI_TEST",),
        (("st_lexer_parser", "direct"),),
    ),
}
REVIEWED_TARGET_IDENTITIES = {
    "FUZZ_TARGET_SYNTAX_PARSE": {
        "name": "syntax_parse",
        "path": "fuzz/fuzz_targets/syntax_parse.rs",
        "manifest_path": "fuzz/Cargo.toml",
        "command": "cd fuzz && cargo fuzz run syntax_parse",
        "owner": "trust-syntax",
        "corpus_path": "fuzz/corpus/syntax_parse",
        "artifact_path": "fuzz/artifacts/syntax_parse",
        "corpus_storage": "machine_local_ignored",
        "artifact_storage": "machine_local_ignored",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_TARGET_HIR_SEMANTIC": {
        "name": "hir_semantic",
        "path": "fuzz/fuzz_targets/hir_semantic.rs",
        "manifest_path": "fuzz/Cargo.toml",
        "command": "cd fuzz && cargo fuzz run hir_semantic",
        "owner": "trust-hir",
        "corpus_path": "fuzz/corpus/hir_semantic",
        "artifact_path": "fuzz/artifacts/hir_semantic",
        "corpus_storage": "machine_local_ignored",
        "artifact_storage": "machine_local_ignored",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_TARGET_ADS_AMS_FRAME": {
        "name": "ams_frame",
        "path": "crates/trust-ads-server/fuzz/fuzz_targets/ams_frame.rs",
        "manifest_path": "crates/trust-ads-server/fuzz/Cargo.toml",
        "command": "cd crates/trust-ads-server/fuzz && cargo fuzz run ams_frame",
        "owner": "trust-ads-server",
        "corpus_path": "crates/trust-ads-server/fuzz/corpus/ams_frame",
        "artifact_path": "crates/trust-ads-server/fuzz/artifacts/ams_frame",
        "corpus_storage": "machine_local_ignored",
        "artifact_storage": "machine_local_ignored",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_TARGET_ADS_BOUNDARY_NOOP": {
        "name": "boundary_noop",
        "path": "crates/trust-ads-server/fuzz/fuzz_targets/boundary_noop.rs",
        "manifest_path": "crates/trust-ads-server/fuzz/Cargo.toml",
        "command": "cd crates/trust-ads-server/fuzz && cargo fuzz run boundary_noop",
        "owner": "trust-ads-server",
        "corpus_path": "crates/trust-ads-server/fuzz/corpus/boundary_noop",
        "artifact_path": "crates/trust-ads-server/fuzz/artifacts/boundary_noop",
        "corpus_storage": "machine_local_ignored",
        "artifact_storage": "machine_local_ignored",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_TARGET_ADS_COMMAND_DISPATCH": {
        "name": "command_dispatch",
        "path": "crates/trust-ads-server/fuzz/fuzz_targets/command_dispatch.rs",
        "manifest_path": "crates/trust-ads-server/fuzz/Cargo.toml",
        "command": "cd crates/trust-ads-server/fuzz && cargo fuzz run command_dispatch",
        "owner": "trust-ads-server",
        "corpus_path": "crates/trust-ads-server/fuzz/corpus/command_dispatch",
        "artifact_path": "crates/trust-ads-server/fuzz/artifacts/command_dispatch",
        "corpus_storage": "machine_local_ignored",
        "artifact_storage": "machine_local_ignored",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_SMOKE_VM_MALFORMED_BYTECODE": {
        "name": "vm_malformed_bytecode_fuzz_smoke_budget",
        "path": "crates/trust-runtime/tests/bytecode_vm_core/fuzz_stack_call.rs",
        "discovery_id": "DISC_FB4371C17A9F9FB83CA9",
        "discovery_source_kind": "rust_integration_test",
        "command": "cargo test -p trust-runtime --test bytecode_vm_core vm_malformed_bytecode_fuzz_smoke_budget",
        "owner": "trust-runtime",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_SMOKE_MESH_PAYLOAD": {
        "name": "mesh_payload_encode_decode_fuzz_smoke_budget",
        "path": "crates/trust-runtime/src/host/mesh/tests.rs",
        "discovery_id": "DISC_A6037EE5CFAA0C4994D2",
        "discovery_source_kind": "rust_unit_test",
        "command": "cargo test -p trust-runtime mesh_payload_encode_decode_fuzz_smoke_budget",
        "owner": "trust-runtime",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_SMOKE_SHM_HEADER": {
        "name": "t0_shm_header_fuzz_rejects_corruption_budget",
        "path": "crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs",
        "discovery_id": "DISC_E97822CE4B2200DD8928",
        "discovery_source_kind": "rust_unit_test",
        "command": "cargo test -p trust-runtime t0_shm_header_fuzz_rejects_corruption_budget",
        "owner": "trust-runtime",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_SMOKE_RUNTIME_CLOUD_API": {
        "name": "runtime_cloud_api_payload_fuzz_smoke_budget",
        "path": "crates/trust-runtime/src/runtime_cloud/routing.rs",
        "discovery_id": "DISC_49D6842FE830D483460D",
        "discovery_source_kind": "rust_unit_test",
        "command": "cargo test -p trust-runtime runtime_cloud_api_payload_fuzz_smoke_budget",
        "owner": "trust-runtime",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_SMOKE_WAN_ALLOWLIST": {
        "name": "wan_allowlist_parser_fuzz_smoke_budget",
        "path": "crates/trust-runtime/src/runtime_cloud/profile_policy.rs",
        "discovery_id": "DISC_794F59E9A339F867023D",
        "discovery_source_kind": "rust_unit_test",
        "command": "cargo test -p trust-runtime wan_allowlist_parser_fuzz_smoke_budget",
        "owner": "trust-runtime",
        "last_reviewed": "2026-07-11",
    },
    "FUZZ_SMOKE_PARSER_INITIALIZER_RECOVERY_PROPERTY": {
        "name": "test_initializer_recovery_property_smoke_for_generated_positional_shapes",
        "path": "crates/trust-syntax/tests/parser_variables.rs",
        "discovery_id": "DISC_21449A3BBD5F3F55D531",
        "discovery_source_kind": "rust_integration_test",
        "command": "cargo test -p trust-syntax --test parser_variables test_initializer_recovery_property_smoke_for_generated_positional_shapes",
        "owner": "trust-syntax",
        "last_reviewed": "2026-07-11",
    },
}
REVIEWED_ASSOCIATION_RATIONALES = {
    ("FUZZ_TARGET_SYNTAX_PARSE", "st_lexer_parser"): "The target sends every valid UTF-8 string decoded from generated bytes through the production Structured Text parser; invalid UTF-8 byte sequences are skipped.",
    ("FUZZ_TARGET_HIR_SEMANTIC", "hir_lowering_input"): "The target exercises HIR semantic queries and invalidation, but it does not establish a complete source-to-runtime lowering path.",
    ("FUZZ_TARGET_HIR_SEMANTIC", "lsp_incremental_edits"): "The target exercises lower-level database edits and re-queries, not LSP ranges, position encoding, or protocol lifecycle.",
    ("FUZZ_TARGET_ADS_AMS_FRAME", "protocol_payloads"): "The target feeds arbitrary byte slices into AMS frame decoding.",
    ("FUZZ_TARGET_ADS_BOUNDARY_NOOP", "protocol_payloads"): "The target exercises ADS Net ID text/binary conversion and notification payload construction with generated bytes.",
    ("FUZZ_TARGET_ADS_COMMAND_DISPATCH", "protocol_payloads"): "The target sends generated ADS command frames through command dispatch.",
    ("FUZZ_SMOKE_VM_MALFORMED_BYTECODE", "bytecode_container_instructions"): "The bounded mutation table exercises container metadata, opcode, operand, jump, reference, and encoding rejection paths.",
    ("FUZZ_SMOKE_MESH_PAYLOAD", "protocol_payloads"): "The bounded generator round-trips valid mesh payloads and feeds generated malformed byte buffers to the decoder.",
    ("FUZZ_SMOKE_SHM_HEADER", "protocol_payloads"): "The bounded generator mutates shared-memory transport headers and requires corruption rejection.",
    ("FUZZ_SMOKE_RUNTIME_CLOUD_API", "protocol_payloads"): "The bounded generator varies runtime-cloud request envelopes, actions, targets, and JSON payload shapes at the API boundary.",
    ("FUZZ_SMOKE_WAN_ALLOWLIST", "protocol_payloads"): "The bounded generator exercises target-pattern policy strings used by runtime-cloud requests, but it does not generate complete protocol payloads.",
    ("FUZZ_SMOKE_PARSER_INITIALIZER_RECOVERY_PROPERTY", "st_lexer_parser"): "The deterministic 7-by-7 input table checks the locked parser diagnostic, bounded error count, and following-declaration recovery.",
}


def load_fuzz_program(root: Path) -> dict[str, Any]:
    path = root.resolve() / FUZZ_PROGRAM_PATH
    return tomllib.loads(path.read_text())


def validate_fuzz_program_contract(root: Path, program: object) -> list[str]:
    """Validate schema, reviewed semantics, gate bindings, and storage posture."""

    root = root.resolve()
    failures: list[str] = []
    try:
        schema = json.loads((root / FUZZ_PROGRAM_SCHEMA_PATH).read_text())
    except (OSError, json.JSONDecodeError) as exc:
        return [f"fuzz-program schema cannot be read: {exc}"]
    failures.extend(validate_fuzz_program_schema_contract(schema))
    failures.extend(validate_json_schema_instance(program, schema))
    if not isinstance(program, Mapping):
        return sorted(set([*failures, "fuzz program root must be an object"]))
    if set(program) != ROOT_FIELDS:
        failures.append("fuzz program root fields drift from the closed contract")
    for field, expected in ROOT_CONSTS.items():
        if program.get(field) != expected:
            failures.append(f"fuzz program {field} drifted from the reviewed contract")
    if program.get("title") != REVIEWED_PROGRAM_TITLE:
        failures.append("fuzz program reviewed program title drifted")
    if program.get("corpus_policy") != REVIEWED_CORPUS_POLICY:
        failures.append("fuzz program corpus policy drifted from the reviewed contract")
    if program.get("crash_regression_handoff") != REVIEWED_CRASH_HANDOFF:
        failures.append("fuzz program crash-regression handoff drifted from the reviewed contract")
    try:
        _validate_surfaces(program.get("surfaces"), failures)
        _validate_targets(program.get("targets"), failures)
        _validate_claim_language(program, failures)
        _validate_corpus_storage(root, failures)
        _validate_gate_bindings(root, program.get("targets"), failures)
        validate_execution_source_bindings(
            root, program.get("targets"), TARGET_ID_ORDER, failures
        )
    except (AttributeError, KeyError, TypeError, ValueError) as exc:
        failures.append(f"fuzz program semantic validation rejected malformed shape: {type(exc).__name__}")
    return sorted(set(failures))


def validate_fuzz_program_schema_contract(schema: object) -> list[str]:
    if not isinstance(schema, dict):
        return ["fuzz-program schema root must be an object"]
    failures: list[str] = []
    semantic_digest = hashlib.sha256(
        json.dumps(schema, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    if semantic_digest != FUZZ_PROGRAM_SCHEMA_SEMANTIC_DIGEST:
        failures.append("fuzz-program schema semantic digest drifted")
    check_supported_schema_keywords(schema, "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("fuzz-program schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("fuzz-program schema root required fields drift")
    root_properties = schema.get("properties")
    if not isinstance(root_properties, Mapping):
        failures.append("fuzz-program schema root properties are missing")
    else:
        if set(root_properties) != ROOT_FIELDS:
            failures.append("fuzz-program schema root properties drifted")
        for field, expected in ROOT_CONSTS.items():
            definition = root_properties.get(field)
            if not isinstance(definition, Mapping) or definition.get("const") != expected:
                failures.append(f"fuzz-program schema {field} const drifted")
        if root_properties.get("title") != {"type": "string", "minLength": 1}:
            failures.append("fuzz-program schema root title contract drifted")
        if root_properties.get("surfaces") != {
            "type": "array",
            "items": {"$ref": "#/$defs/surface"},
            "minItems": 8,
            "maxItems": 8,
        }:
            failures.append("fuzz-program schema surfaces array contract drifted")
        if root_properties.get("targets") != {
            "type": "array",
            "items": {"$ref": "#/$defs/target"},
            "minItems": 11,
            "maxItems": 11,
        }:
            failures.append("fuzz-program schema targets array contract drifted")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        return [*failures, "fuzz-program schema definitions are missing"]
    expected_enums = (
        ("surface_id", REQUIRED_SURFACE_IDS),
        ("area", AREA_IDS),
        ("tier", TIER_IDS),
    )
    for name, expected in expected_enums:
        definition = definitions.get(name)
        if not isinstance(definition, Mapping) or definition.get("enum") != list(expected):
            failures.append(f"fuzz-program schema {name} enum drifted")
    target = definitions.get("target")
    if not isinstance(target, Mapping) or target.get("additionalProperties") is not False:
        failures.append("fuzz-program schema target must be a closed object")
    elif isinstance(target.get("properties"), Mapping):
        properties = target["properties"]
        if set(target.get("required", [])) != COMMON_TARGET_FIELDS:
            failures.append("fuzz-program schema target required fields drifted")
        if set(properties) != SCHEMA_TARGET_PROPERTIES:
            failures.append("fuzz-program schema target properties drifted")
        expected_target_enums = {
            "target_kind": TARGET_KINDS,
            "enforcement_status": ENFORCEMENT_STATUSES,
        }
        for field, expected in expected_target_enums.items():
            definition = properties.get(field)
            if not isinstance(definition, Mapping) or definition.get("enum") != list(expected):
                failures.append(f"fuzz-program schema target {field} enum drifted")
        expected_target_properties = {
            "path": {"type": "string", "pattern": "^[A-Za-z0-9_./-]+$"},
            "command": {"type": "string", "minLength": 1},
            "additional_tiers": {
                "type": "array",
                "items": {"$ref": "#/$defs/tier"},
                "maxItems": 1,
                "uniqueItems": True,
            },
            "surface_associations": {
                "type": "array",
                "items": {"$ref": "#/$defs/surface_association"},
                "minItems": 1,
                "uniqueItems": True,
            },
        }
        for field, expected in expected_target_properties.items():
            if properties.get(field) != expected:
                failures.append(f"fuzz-program schema target {field} contract drifted")
    association = definitions.get("surface_association")
    if not isinstance(association, Mapping) or association.get("additionalProperties") is not False:
        failures.append("fuzz-program schema surface association must be a closed object")
    elif isinstance(association.get("properties"), Mapping):
        if set(association.get("required", [])) != ASSOCIATION_FIELDS:
            failures.append("fuzz-program schema surface association required fields drifted")
        if set(association["properties"]) != ASSOCIATION_FIELDS:
            failures.append("fuzz-program schema surface association properties drifted")
        strength = association["properties"].get("strength")
        if not isinstance(strength, Mapping) or strength.get("enum") != list(ASSOCIATION_STRENGTHS):
            failures.append("fuzz-program schema association strength enum drifted")
        if association["properties"].get("surface_id") != {"$ref": "#/$defs/surface_id"}:
            failures.append("fuzz-program schema association surface_id binding drifted")
    _validate_closed_definition_shape(
        definitions.get("surface"),
        SURFACE_FIELDS,
        SURFACE_FIELDS,
        "surface",
        failures,
    )
    surface = definitions.get("surface")
    surface_properties = surface.get("properties") if isinstance(surface, Mapping) else None
    if not isinstance(surface_properties, Mapping) or surface_properties.get("id") != {
        "$ref": "#/$defs/surface_id"
    }:
        failures.append("fuzz-program schema surface id binding drifted")
    if not isinstance(surface_properties, Mapping) or surface_properties.get("area") != {
        "$ref": "#/$defs/area"
    }:
        failures.append("fuzz-program schema surface area binding drifted")
    _validate_closed_definition_shape(
        definitions.get("corpus_policy"),
        set(REVIEWED_CORPUS_POLICY),
        set(REVIEWED_CORPUS_POLICY),
        "corpus_policy",
        failures,
    )
    _validate_closed_definition_shape(
        definitions.get("crash_regression_handoff"),
        set(REVIEWED_CRASH_HANDOFF),
        set(REVIEWED_CRASH_HANDOFF),
        "crash_regression_handoff",
        failures,
    )
    _validate_definition_consts(
        definitions.get("corpus_policy"),
        REVIEWED_CORPUS_POLICY,
        "corpus_policy",
        failures,
    )
    _validate_definition_consts(
        definitions.get("crash_regression_handoff"),
        REVIEWED_CRASH_HANDOFF,
        "crash_regression_handoff",
        failures,
    )
    return failures


def _validate_definition_consts(
    definition: object,
    expected: Mapping[str, Any],
    name: str,
    failures: list[str],
) -> None:
    properties = definition.get("properties") if isinstance(definition, Mapping) else None
    if not isinstance(properties, Mapping):
        failures.append(f"fuzz-program schema {name} properties are missing")
        return
    actual = {
        field: value.get("const") if isinstance(value, Mapping) else None
        for field, value in properties.items()
    }
    if actual != dict(expected):
        failures.append(f"fuzz-program schema {name} consts drifted")


def _validate_closed_definition_shape(
    definition: object,
    required: set[str],
    properties: set[str],
    name: str,
    failures: list[str],
) -> None:
    if not isinstance(definition, Mapping) or definition.get("additionalProperties") is not False:
        failures.append(f"fuzz-program schema {name} must be a closed object")
        return
    if set(definition.get("required", [])) != required:
        failures.append(f"fuzz-program schema {name} required fields drifted")
    actual_properties = definition.get("properties")
    if not isinstance(actual_properties, Mapping) or set(actual_properties) != properties:
        failures.append(f"fuzz-program schema {name} properties drifted")


def _validate_surfaces(value: object, failures: list[str]) -> None:
    if not isinstance(value, list):
        failures.append("fuzz program surfaces must be an array")
        return
    ids = [row.get("id") for row in value if isinstance(row, Mapping)]
    if ids != list(REQUIRED_SURFACE_IDS):
        failures.append("fuzz program surfaces must use the exact reviewed Phase 9 order")
    expected_rows = [
        {"id": surface_id, "title": title, "area": area, "rationale": rationale}
        for surface_id, title, area, rationale in REVIEWED_SURFACE_ROWS
    ]
    if value != expected_rows:
        failures.append("fuzz program reviewed surface rows drifted")
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"surfaces[{index}] must be an object")
            continue
        surface_id = row.get("id")
        if isinstance(surface_id, str) and row.get("area") != SURFACE_AREAS.get(surface_id):
            failures.append(f"surface {surface_id} area drifted from the reviewed contract")


def _validate_targets(value: object, failures: list[str]) -> None:
    if not isinstance(value, list):
        failures.append("fuzz program targets must be an array")
        return
    ids = [row.get("id") for row in value if isinstance(row, Mapping)]
    if ids != list(TARGET_ID_ORDER):
        failures.append("fuzz program targets must use the exact reviewed identity order")
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"targets[{index}] must be an object")
            continue
        target_id = row.get("id")
        if not isinstance(target_id, str):
            continue
        expected = REVIEWED_TARGET_CONTRACTS.get(target_id)
        if expected is None:
            failures.append(f"unknown fuzz target id {target_id}")
            continue
        identity = REVIEWED_TARGET_IDENTITIES.get(target_id)
        if identity is None or any(row.get(field) != value for field, value in identity.items()):
            failures.append(f"{target_id} identity fields drift from reviewed live target contract")
        kind = row.get("target_kind")
        expected_fields = CARGO_TARGET_FIELDS if kind == "cargo_fuzz" else SMOKE_TARGET_FIELDS
        if set(row) != expected_fields:
            failures.append(f"{target_id} fields drift from the {kind} union contract")
        if kind != expected["target_kind"]:
            failures.append(f"{target_id} target kind drifted from reviewed live target contract")
        additional_value = row.get("additional_tiers")
        if not isinstance(additional_value, list) or not all(
            isinstance(item, str) for item in additional_value
        ):
            failures.append(f"{target_id} additional_tiers must be a string array")
            additional: tuple[str, ...] = ()
        else:
            additional = tuple(additional_value)
        basis_value = row.get("execution_basis_ids")
        if not isinstance(basis_value, list) or not all(
            isinstance(item, str) for item in basis_value
        ):
            failures.append(f"{target_id} execution_basis_ids must be a string array")
            basis: tuple[str, ...] = ()
        else:
            basis = tuple(basis_value)
        if (
            row.get("primary_tier") != expected["primary_tier"]
            or additional != expected["additional_tiers"]
            or row.get("enforcement_status") != expected["enforcement_status"]
        ):
            failures.append(f"{target_id} tier fields drift from reviewed live target contract")
        if basis != expected["execution_basis_ids"]:
            failures.append(f"{target_id} execution basis drifted from reviewed live target contract")
        associations = row.get("surface_associations")
        actual_associations = tuple(
            (item.get("surface_id"), item.get("strength"))
            for item in associations
            if isinstance(item, Mapping)
        ) if isinstance(associations, list) else ()
        if actual_associations != expected["surface_associations"]:
            failures.append(f"{target_id} surface associations drift from reviewed live target contract")
        if isinstance(associations, list):
            for association in associations:
                if not isinstance(association, Mapping):
                    continue
                key = (target_id, association.get("surface_id"))
                if association.get("rationale") != REVIEWED_ASSOCIATION_RATIONALES.get(key):
                    failures.append(f"{target_id} reviewed association rationale drifted")
        if row.get("primary_tier") in additional:
            failures.append(f"{target_id} repeats its primary tier as an additional tier")
        for field in ("path", "manifest_path", "corpus_path", "artifact_path"):
            if field in row and not _safe_relative_path(row.get(field)):
                failures.append(f"{target_id} {field} must be normalized and workspace-relative")


def _validate_claim_language(program: Mapping[str, Any], failures: list[str]) -> None:
    def walk(value: object, where: str) -> None:
        if isinstance(value, str) and FORBIDDEN_CLAIM_RE.search(value):
            failures.append(f"{where} uses forbidden proof or completed-coverage claim language")
        elif isinstance(value, list):
            for index, item in enumerate(value):
                walk(item, f"{where}[{index}]")
        elif isinstance(value, Mapping):
            for key, item in value.items():
                if key not in {"proof_posture"}:
                    walk(item, f"{where}.{key}")
    walk(program, "$fuzz_program")


def _validate_corpus_storage(root: Path, failures: list[str]) -> None:
    required_patterns = {"artifacts/", "corpus/", "coverage/", "target/"}
    for relative in ("fuzz/.gitignore", "crates/trust-ads-server/fuzz/.gitignore"):
        path = root / relative
        try:
            if path.is_symlink():
                raise OSError("symlink is not allowed")
            lines = {line.strip() for line in path.read_text().splitlines() if line.strip() and not line.lstrip().startswith("#")}
        except OSError as exc:
            failures.append(f"{relative} cannot be read as a regular ignore contract: {exc}")
            continue
        if lines != required_patterns:
            failures.append(
                f"{relative} must contain exactly artifacts/, corpus/, coverage/, and target/"
            )
        workspace = Path(relative).parent.as_posix()
        for generated_root in sorted(required_patterns):
            probe = f"{workspace}/{generated_root}__phase9_probe__"
            ignored = subprocess.run(
                ["git", "-C", str(root), "check-ignore", "--no-index", "-q", "--", probe],
                check=False,
                capture_output=True,
            )
            if ignored.returncode != 0:
                failures.append(f"{relative} does not effectively ignore {generated_root}")
    try:
        result = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            check=False,
            capture_output=True,
        )
    except OSError as exc:
        failures.append(f"could not inspect tracked corpus paths: {exc}")
        return
    if result.returncode != 0:
        failures.append("could not inspect tracked corpus paths")
        return
    tracked_generated = []
    for item in result.stdout.split(b"\0"):
        if not item:
            continue
        value = item.decode()
        parts = PurePosixPath(value).parts
        if "fuzz" in parts and any(
            part in {"corpus", "artifacts", "coverage", "crashes", "target"}
            for part in parts
        ):
            tracked_generated.append(value)
    if tracked_generated:
        failures.append("generated fuzz corpus/crash paths must not be tracked: " + ", ".join(sorted(tracked_generated)))


def _validate_gate_bindings(root: Path, targets: object, failures: list[str]) -> None:
    try:
        inventory = load_gate_inventory(root)
    except GateInventoryError as exc:
        failures.append(f"gate inventory cannot be loaded for fuzz tiers: {exc}")
        return
    if not isinstance(targets, list):
        return
    tier_to_suite = {"pr_smoke": "pr", "nightly": "nightly", "manual_extended": None}
    for row in targets:
        if not isinstance(row, Mapping) or not isinstance(row.get("id"), str):
            continue
        target_id = row["id"]
        basis_ids = row.get("execution_basis_ids")
        if not isinstance(basis_ids, list):
            continue
        records = []
        for basis_id in basis_ids:
            record = inventory.get(basis_id) if isinstance(basis_id, str) else None
            if record is None:
                failures.append(f"{target_id} names unknown execution basis {basis_id!r}")
            else:
                records.append(record)
        additional = row.get("additional_tiers")
        if not isinstance(additional, list):
            additional = []
        expected_suites = {
            tier_to_suite[tier]
            for tier in [row.get("primary_tier"), *additional]
            if tier in tier_to_suite and tier_to_suite[tier] is not None
        }
        actual_suites = {
            suite
            for record in records
            for suite in record.get("suite_ids", [])
            if isinstance(suite, str)
        }
        if not expected_suites.issubset(actual_suites):
            failures.append(f"{target_id} execution bases do not support its reviewed tiers")
        enforcement = row.get("enforcement_status")
        if enforcement == "manual_only" and records:
            failures.append(f"{target_id} manual-only target must not claim an execution basis")
        if enforcement == "wired" and not any(record.get("enforcement") == "required" for record in records):
            failures.append(f"{target_id} wired target lacks a required live execution basis")
        if enforcement == "planned" and not records:
            failures.append(f"{target_id} planned target lacks its planned inventory basis")
        if enforcement == "planned" and any(record.get("enforcement") != "planned" for record in records):
            failures.append(f"{target_id} planned target basis must remain planned")


def _safe_relative_path(value: object) -> bool:
    if not isinstance(value, str) or not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts
