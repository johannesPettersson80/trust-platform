"""Generate reviewed bytecode assertion assignments from semantic rules."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tomllib
from collections import defaultdict
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
INSTRUCTION_BOUNDARY = "RT_BYTECODE_INSTRUCTION_BOUNDARY_001"

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


def generate_path_plans(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    assignments_document: Mapping[str, Any],
    execution_dir: Path,
    output_dir: Path,
    reviewed_date: str,
    timestamp: str,
) -> dict[str, Any]:
    """Project reviewed bytecode assignments onto existing path proof."""

    if not reviewed_date or not timestamp:
        raise ValueError("plan review date and timestamp must be non-empty")
    inventories = inventory_document.get("inventories")
    assignments = assignments_document.get("assignments")
    if not isinstance(inventories, Mapping) or not isinstance(assignments, list):
        raise ValueError("bytecode plan inputs are invalid")
    assignment_by_key = {
        (_string(row, "path", "assignment"), _string(row, "name", "assignment")): row
        for row in assignments
        if isinstance(row, Mapping)
    }
    inventory_by_key = {
        (
            _string(row, "path", str(discovery_id)),
            _string(row, "test_name", str(discovery_id)),
        ): (str(discovery_id), row)
        for discovery_id, row in inventories.items()
        if isinstance(row, Mapping)
    }
    if (
        len(assignment_by_key) != len(assignments)
        or len(inventory_by_key) != len(inventories)
        or set(assignment_by_key) != set(inventory_by_key)
    ):
        raise ValueError("bytecode assignments and inventories are not bijective")

    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    review_by_key = {
        (str(row.get("path")), str(row.get("name"))): row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and (str(row.get("path")), str(row.get("name")))
        in assignment_by_key
    }
    if set(review_by_key) != set(assignment_by_key):
        raise ValueError(
            "bytecode assignments do not equal live mapping-debt rows"
        )

    evidence_by_path: dict[str, tuple[Mapping[str, Any], Path]] = {}
    for metadata_path in sorted((execution_dir / "meta").glob("*.json")):
        metadata = json.loads(metadata_path.read_text())
        if not isinstance(metadata, Mapping):
            raise ValueError(f"{metadata_path} is not an object")
        path = _string(metadata, "path", metadata_path.as_posix())
        results_path = execution_dir / "results" / metadata_path.name
        if path in evidence_by_path or not results_path.is_file():
            raise ValueError(f"duplicate or missing path evidence for {path}")
        evidence_by_path[path] = (metadata, results_path)

    assignments_by_path: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for (path, _name), row in assignment_by_key.items():
        assignments_by_path[path].append(row)
    if set(evidence_by_path) != set(assignments_by_path):
        raise ValueError("bytecode execution paths do not equal assignment paths")

    invariants = _load_invariants(root)
    output_dir.mkdir(parents=True, exist_ok=True)
    path_records = []
    for path in sorted(assignments_by_path):
        metadata, results_path = evidence_by_path[path]
        results = json.loads(results_path.read_text())
        execution = results.get("execution") if isinstance(results, Mapping) else None
        if not isinstance(execution, Mapping):
            raise ValueError(f"{path} results lack execution provenance")
        path_assignments = sorted(
            assignments_by_path[path],
            key=lambda row: str(row["name"]),
        )
        names = {str(row["name"]) for row in path_assignments}
        passed_names = {
            str(row.get("title"))
            for row in results.get("tests", [])
            if isinstance(row, Mapping) and row.get("state") == "passed"
        }
        if not names <= passed_names:
            raise ValueError(f"{path} focused proof lacks assigned tests")
        path_reviews = [review_by_key[(path, name)] for name in sorted(names)]
        setup_by_name = {
            str(row["name"]): set(row["setup_assertion_ids"])
            for row in path_assignments
        }
        non_oracle_by_name = {
            str(row["name"]): set(row["non_oracle_assertion_ids"])
            for row in path_assignments
        }
        invariant_ids = sorted(
            {
                str(assertion["invariant_id"])
                for row in path_assignments
                for assertion in row["assertion_assignments"]
                if assertion["assertion_id"]
                not in setup_by_name[str(row["name"])]
                and assertion["assertion_id"]
                not in non_oracle_by_name[str(row["name"])]
            }
        )
        for invariant_id in invariant_ids:
            if invariant_id not in invariants:
                raise ValueError(f"{path} selects unknown invariant {invariant_id}")
        key = _path_key(path)
        plan_path = output_dir / f"{key}-plan.json"
        inventory_path = output_dir / f"{key}-assertions.json"
        plan = {
            "schema_version": 1,
            "execution_kind": "rust_nextest_jsonl",
            "target_profile": "runtime-remaining",
            "path": path,
            "package": _string(execution, "package", path),
            "target_kind": _string(execution, "target_kind", path),
            "binary_id": _string(execution, "binary_id", path),
            "test_prefix": execution.get("test_prefix"),
            "command": _string(execution, "batch_command", path),
            "runner_path": _string(execution, "runner_path", path),
            "raw_results_path": _string(
                execution, "raw_results_path", path
            ),
            "reviewed_date": reviewed_date,
            "timestamp": timestamp,
            "reviewer_ids": ["codex-main", "codex-assertion-review"],
            "expected_count": len(path_reviews),
            "selection_digest": _selection_digest(path_reviews),
            "invariant_ids": invariant_ids,
            "invariant_behavior_digests": {
                invariant_id: _digest_document(
                    {"behavior": invariants[invariant_id].get("behavior")}
                )
                for invariant_id in invariant_ids
            },
            "assertion_assignments": [
                {
                    "name": row["name"],
                    "assertions": row["assertion_assignments"],
                }
                for row in path_assignments
            ],
            "assertion_dispositions": [
                {
                    "name": row["name"],
                    "setup_assertion_ids": row["setup_assertion_ids"],
                    "non_oracle_assertion_ids": row["non_oracle_assertion_ids"],
                }
                for row in path_assignments
                if row["setup_assertion_ids"]
                or row["non_oracle_assertion_ids"]
            ],
        }
        if not isinstance(plan["test_prefix"], str):
            raise ValueError(f"{path} execution test prefix is invalid")
        projected_inventory = {
            "schema_version": inventory_document.get("schema_version", 2),
            "inventories": {
                inventory_by_key[(path, name)][0]: inventory_by_key[
                    (path, name)
                ][1]
                for name in sorted(names)
            },
        }
        _write_json(plan_path, plan)
        _write_json(inventory_path, projected_inventory)
        path_records.append(
            {
                "path": path,
                "expected_count": len(path_reviews),
                "plan_path": plan_path.relative_to(root).as_posix(),
                "inventory_path": inventory_path.relative_to(root).as_posix(),
                "results_path": results_path.relative_to(root).as_posix(),
            }
        )
    index = {
        "schema_version": 1,
        "source_commit": subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip(),
        "expected_count": len(assignment_by_key),
        "paths": path_records,
    }
    _write_json(output_dir / "index.json", index)
    return index


def _default_behavior(path: str, name: str) -> BehaviorRef:
    if path.endswith("/bytecode/validate/resource_limits.rs"):
        if name == "stack_limit_accepts_boundary_and_rejects_next_value":
            return (STACK_BOUNDARY, 0)
        if name == (
            "instruction_limit_accepts_boundary_and_rejects_next_instruction"
        ):
            return (INSTRUCTION_BOUNDARY, 0)
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
    if (
        path.endswith("/bytecode/validate/resource_limits.rs")
        and name
        == "instruction_limit_accepts_boundary_and_rejects_next_instruction"
        and "is_err" in expression
    ):
        return (INSTRUCTION_BOUNDARY, 1)
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


def _digest_document(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _selection_digest(reviews: Sequence[Mapping[str, Any]]) -> str:
    rows = [
        {
            field: _string(row, field, "denominator review")
            for field in (
                "discovery_id",
                "discovery_source_kind",
                "path",
                "name",
                "ignore_state",
            )
        }
        for row in reviews
    ]
    rows.sort(key=lambda row: row["discovery_id"])
    return _digest_document({"reviews": rows})


def _path_key(path: str) -> str:
    return (
        Path(path).stem.replace("_", "-")
        + "-"
        + hashlib.sha256(path.encode()).hexdigest()[:12]
    )


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
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
        if (args.execution_dir is None) != (args.plans_out is None):
            raise ValueError(
                "--execution-dir and --plans-out must be provided together"
            )
        if args.execution_dir is not None and args.plans_out is not None:
            execution_dir = (
                args.execution_dir
                if args.execution_dir.is_absolute()
                else root / args.execution_dir
            )
            plans_out = (
                args.plans_out
                if args.plans_out.is_absolute()
                else root / args.plans_out
            )
            index = generate_path_plans(
                root=root,
                inventory_document=document,
                assignments_document=assignments,
                execution_dir=execution_dir,
                output_dir=plans_out,
                reviewed_date=args.reviewed_date or "2026-07-27",
                timestamp=args.timestamp or "2026-07-27T00:00:00Z",
            )
            print(
                "runtime bytecode catalog plans generated: "
                f"{index['expected_count']} tests across "
                f"{len(index['paths'])} paths"
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
