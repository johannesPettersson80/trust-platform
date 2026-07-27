"""Generate reviewed runtime-memory assertion assignments from semantic rules."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import generate_path_plans


INVARIANT = "RT_VARIABLE_STORAGE_REFERENCE_001"
MEMORY_TEST_PATH = "crates/trust-runtime/src/memory/tests.rs"
BEHAVIOR_BY_TEST: Mapping[str, int] = {
    "variable_storage_clone_recovers_from_poisoned_cache_lock": 0,
    "instance_field_cache_is_scoped_per_instance": 0,
    "direct_instance_field_miss_cache_invalidates_on_new_insert": 0,
    "recursive_instance_field_cache_invalidates_when_child_adds_shadowing_field": 0,
    "recursive_lookup_does_not_cache_parent_chain_miss": 0,
    "declared_instance_field_offset_reuses_type_layout_for_declared_fields": 1,
    "declared_instance_field_offset_skips_inherited_fields": 1,
    "resolved_instance_field_ref_prefers_direct_field_before_parent_fallback": 1,
    "direct_instance_field_offset_reads_and_writes_without_value_ref": 1,
    "direct_slot_helpers_cover_global_local_and_instance_locations": 2,
    "direct_slot_helpers_match_empty_path_ref_helpers": 2,
    "borrowed_value_ref_helpers_match_owned_helpers": 2,
    "write_by_ref_path_preserves_struct_copy_on_write_isolation": 3,
    "read_and_write_by_ref_handle_extreme_array_bounds_without_overflow": 4,
}


def select_live_memory_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    """Select exactly the live memory mapping-debt rows from a saved inventory."""

    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("assertion inventory document lacks inventories")
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") == MEMORY_TEST_PATH
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
    }
    if set(expected.values()) != {
        (MEMORY_TEST_PATH, name) for name in BEHAVIOR_BY_TEST
    }:
        raise ValueError("live memory denominator does not equal reviewed rules")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live memory mapping-debt rows")
    for discovery_id, raw in selected.items():
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        identity = (raw.get("path"), raw.get("test_name"))
        if identity != expected[discovery_id]:
            raise ValueError(f"{discovery_id} inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    """Assign every selected assertion to the reviewed storage behavior."""

    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("memory assertion document lacks inventories")
    behavior_count = _invariant_behavior_count(root)
    rows = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if path != MEMORY_TEST_PATH:
            raise ValueError(f"unexpected runtime-memory path {path}")
        if name in seen_names:
            raise ValueError(f"duplicate runtime-memory test {name}")
        seen_names.add(name)
        try:
            behavior_index = BEHAVIOR_BY_TEST[name]
        except KeyError as error:
            raise ValueError(f"no reviewed runtime-memory rule for {name}") from error
        if behavior_index >= behavior_count:
            raise ValueError(f"{name} selects missing invariant behavior")
        if raw.get("unresolved_assertion_helpers") != []:
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
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": INVARIANT,
                    "behavior_index": behavior_index,
                }
            )
            if _is_setup(name=name, expression=expression):
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
    if seen_names != set(BEHAVIOR_BY_TEST):
        raise ValueError("runtime-memory assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_memory_semantic_rules",
        "assignments": rows,
    }


def _is_setup(*, name: str, expression: str) -> bool:
    if name == "variable_storage_clone_recovers_from_poisoned_cache_lock":
        return "test cache lock" in expression or "poison instance field cache" in expression
    if "get_instance_mut (derived)" in expression:
        return True
    if name == "recursive_instance_field_cache_invalidates_when_child_adds_shadowing_field":
        return "set_instance_var (base" in expression
    if name == "declared_instance_field_offset_reuses_type_layout_for_declared_fields":
        return "set_instance_var" in expression
    if name == "declared_instance_field_offset_skips_inherited_fields":
        return "set_instance_var (base" in expression
    if name == "resolved_instance_field_ref_prefers_direct_field_before_parent_fallback":
        return "set_instance_var" in expression
    if name == "direct_instance_field_offset_reads_and_writes_without_value_ref":
        return "set_instance_var" in expression
    if name in {
        "direct_slot_helpers_cover_global_local_and_instance_locations",
        "direct_slot_helpers_match_empty_path_ref_helpers",
    }:
        return (
            "set_local" in expression
            or "set_instance_var" in expression
            or ". ref_for_" in expression
        )
    if name == "borrowed_value_ref_helpers_match_owned_helpers":
        return "set_instance_var" in expression or ". ref_for_instance" in expression
    if name == "recursive_lookup_does_not_cache_parent_chain_miss":
        return "set_instance_var (base" in expression
    if name == "write_by_ref_path_preserves_struct_copy_on_write_isolation":
        return ". get_global" in expression
    if name == "read_and_write_by_ref_handle_extreme_array_bounds_without_overflow":
        return ". ref_for_global" in expression
    return False


def _invariant_behavior_count(root: Path) -> int:
    matches = list(
        (root / "verification/invariants").glob(f"**/{INVARIANT}.toml")
    )
    if len(matches) != 1:
        raise ValueError(f"expected one {INVARIANT} invariant")
    record = tomllib.loads(matches[0].read_text())
    behaviors = record.get("behavior")
    if not isinstance(behaviors, list):
        raise ValueError(f"{INVARIANT} lacks behaviors")
    return len(behaviors)


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    inventory = json.loads(args.inventory.read_text())
    selected = select_live_memory_inventories(
        root=root,
        inventory_document=inventory,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(args.selected_inventory_out, selected)
    _write_json(args.assignments_out, assignments)
    print(
        "runtime memory assignments generated: "
        f"{len(assignments['assignments'])} mapped"
    )
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError("memory plan generation arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=args.execution_dir,
            output_dir=args.plans_out,
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
        )
        print(
            "runtime memory catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
