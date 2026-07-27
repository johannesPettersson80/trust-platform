"""Generate reviewed bytecode assertion assignments from semantic rules."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


BehaviorRef = tuple[str, int]
CODEC = "RT_BYTECODE_CONTAINER_CODEC_001"
ENCODER = "RT_BYTECODE_ENCODER_PROJECTION_001"
METADATA = "RT_BYTECODE_METADATA_APPLY_001"
VALIDATOR = "VM_SEAM_VALID_001"
LIMITS = "VM_SEAM_DETERMINISM_LIMITS_001"
REFERENCES = "VM_SEAM_REF_001"
STACK_BOUNDARY = "RT_BYTECODE_STACK_BOUNDARY_001"

ENCODER_BEHAVIORS: Mapping[str, int] = {
    "encoder_accepts_case_insensitive_names_in_call_heavy_if_blocks": 7,
    "encoder_bytes_roundtrip_from_source": 7,
    "encoder_emits_composite_types": 0,
    "encoder_emits_control_flow_jumps": 5,
    "encoder_emits_debug_map": 4,
    "encoder_emits_dynamic_instance_access": 6,
    "encoder_emits_if_with_string_literal_elsif_condition": 5,
    "encoder_emits_interface_methods": 0,
    "encoder_emits_io_map": 3,
    "encoder_emits_local_refs_for_functions_and_methods": 2,
    "encoder_emits_method_tables": 1,
    "encoder_emits_param_defaults": 1,
    "encoder_emits_program_field_var_meta_for_validation": 2,
    "encoder_emits_scoped_function_local_var_meta": 2,
    "encoder_emits_var_meta_and_retain_init": 2,
    "encoder_preserves_label_only_statement_as_explicit_nop": 5,
    "encoder_resource_meta_preserves_declared_resource_name": 3,
    "encoder_resource_meta_sizes_follow_io_bindings": 3,
    "encoder_roundtrip_validates": 7,
    "encoder_validates_enum_constant_payloads": 8,
}

METADATA_BEHAVIORS: Mapping[str, int] = {
    "apply_bytecode_bytes": 2,
    "fb_refs_from_container": 1,
    "pou_associations": 1,
    "resources_from_container": 0,
    "task_associations": 0,
    "tasks_from_metadata": 0,
    "version_gate": 3,
}

VALIDATOR_BEHAVIORS: Mapping[str, int] = {
    "debug_map_validation": 7,
    "jump_validation": 4,
    "opcode_validation": 2,
    "opcode_validation_extended": 6,
    "validator_rejects_unsupported_runtime_opcodes_before_dispatch": 2,
    "var_meta_rejects_duplicate_ref_idx": 8,
    "var_meta_rejects_duplicate_textual_name_at_different_string_indices": 8,
    "var_meta_rejects_local_ref_outside_every_pou_range": 0,
    "var_meta_rejects_local_retain_and_initializer_state": 8,
}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("bytecode assertion document lacks inventories")
    invariants = _load_invariants(root)
    rows = []
    seen: set[tuple[str, str]] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        key = (path, name)
        if key in seen:
            raise ValueError(f"duplicate bytecode test {path}:{name}")
        seen.add(key)
        unresolved = raw.get("unresolved_assertion_helpers")
        if unresolved != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            behavior = _behavior_for_assertion(
                path=path,
                name=name,
                expression=expression,
            )
            _validate_behavior(invariants, behavior, name)
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": behavior[0],
                    "behavior_index": behavior[1],
                }
            )
            if _is_setup(path=path, name=name, expression=expression):
                setup_ids.append(assertion_id)
        if len(setup_ids) == len(assertion_rows):
            raise ValueError(f"{path}:{name} has no product-oracle assertion")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_bytecode_semantic_test_and_assertion_rules",
        "assignments": rows,
    }


def _default_behavior(path: str, name: str) -> BehaviorRef:
    if path.endswith("/bytecode/validate/resource_limits.rs"):
        if name == "stack_limit_accepts_boundary_and_rejects_next_value":
            return (STACK_BOUNDARY, 0)
    if path.endswith("/bytecode_decode_resource_bounds.rs"):
        return (LIMITS, 0)
    if path.endswith("/bytecode_container.rs"):
        return (VALIDATOR, 7)
    if "/tests/bytecode_encoder/" in path:
        try:
            return (ENCODER, ENCODER_BEHAVIORS[name])
        except KeyError as error:
            raise ValueError(f"no reviewed encoder rule for {path}:{name}") from error
    if path.endswith("/bytecode_metadata.rs"):
        try:
            return (METADATA, METADATA_BEHAVIORS[name])
        except KeyError as error:
            raise ValueError(
                f"no reviewed bytecode metadata rule for {path}:{name}"
            ) from error
    if path.endswith("/bytecode_roundtrip.rs"):
        if name == "roundtrip":
            return (CODEC, 2)
        if name == "corruption_rejected":
            return (CODEC, 3)
    if path.endswith("/bytecode_sections.rs"):
        return (CODEC, 1 if name == "string_table_padding" else 0)
    if path.endswith("/bytecode_validation.rs"):
        if name == "var_meta_rejects_local_ref_outside_every_pou_range":
            return (REFERENCES, VALIDATOR_BEHAVIORS[name])
        try:
            return (VALIDATOR, VALIDATOR_BEHAVIORS[name])
        except KeyError as error:
            raise ValueError(
                f"no reviewed bytecode validation rule for {path}:{name}"
            ) from error
    raise ValueError(f"no reviewed bytecode rule for {path}:{name}")


def _behavior_for_assertion(
    *,
    path: str,
    name: str,
    expression: str,
) -> BehaviorRef:
    if (
        path.endswith("/bytecode/validate/resource_limits.rs")
        and name == "stack_limit_accepts_boundary_and_rejects_next_value"
        and "VM_MAX_OPERAND_STACK + 1" in expression
    ):
        return (STACK_BOUNDARY, 1)
    return _default_behavior(path, name)


def _is_setup(*, path: str, name: str, expression: str) -> bool:
    if "/tests/bytecode_encoder/" in path:
        return "bytecode_module_from_source" in expression
    if path.endswith("/bytecode_metadata.rs"):
        return "TestHarness :: from_source" in expression
    if path.endswith("/bytecode_container.rs"):
        return "module . encode" in expression
    if path.endswith("/bytecode_decode_resource_bounds.rs"):
        return "module . encode" in expression
    if path.endswith("/bytecode_validation.rs"):
        if name == "opcode_validation_extended":
            return (
                "module . encode" in expression
                or "BytecodeModule :: decode" in expression
            )
        product_tokens = ("unwrap_err", "expect_err", "matches !")
        return not any(token in expression for token in product_tokens)
    return False


def _load_invariants(root: Path) -> dict[str, Mapping[str, Any]]:
    result = {}
    for path in (root / "verification/invariants").rglob("*.toml"):
        record = tomllib.loads(path.read_text())
        invariant_id = record.get("id")
        if isinstance(invariant_id, str):
            result[invariant_id] = record
    return result


def _validate_behavior(
    invariants: Mapping[str, Mapping[str, Any]],
    behavior: BehaviorRef,
    name: str,
) -> None:
    invariant_id, index = behavior
    invariant = invariants.get(invariant_id)
    rows = invariant.get("behavior") if isinstance(invariant, Mapping) else None
    if (
        not isinstance(rows, Sequence)
        or isinstance(rows, (str, bytes))
        or index < 0
        or index >= len(rows)
    ):
        raise ValueError(f"{name} selects absent behavior {invariant_id}#{index}")


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    inventory_path = (
        args.inventory if args.inventory.is_absolute() else root / args.inventory
    )
    output_path = (
        args.assignments_out
        if args.assignments_out.is_absolute()
        else root / args.assignments_out
    )
    try:
        document = json.loads(inventory_path.read_text())
        if not isinstance(document, Mapping):
            raise ValueError("bytecode assertion inventory must be an object")
        assignments = generate_assignments(
            root=root,
            inventory_document=document,
        )
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            json.dumps(assignments, indent=2, sort_keys=True) + "\n"
        )
        print(
            "runtime bytecode assignments generated: "
            f"{len(assignments['assignments'])} mapped"
        )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(f"runtime bytecode assignment generation failed: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
