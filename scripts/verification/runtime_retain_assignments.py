"""Generate reviewed retain assertion assignments from semantic rules."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import generate_path_plans


PERSISTENCE = "RT_RETAIN_PERSISTENCE_CODEC_001"
MIGRATION = "RT_RETAIN_MIGRATION_001"
SAFE_RETAIN = "RT_SAFE_RETAIN_001"
SAFE_RESTART = "RT_SAFE_RESTART_001"

BEHAVIOR_BY_TEST: Mapping[str, tuple[str, int]] = {
    "retain_store_missing_file_returns_default": (PERSISTENCE, 0),
    "retain_store_reports_real_path_failures_without_silent_defaults": (
        PERSISTENCE,
        1,
    ),
    "file_retain_store_open_and_read_failures_are_visible": (PERSISTENCE, 1),
    "file_retain_store_replaces_snapshot_atomically": (PERSISTENCE, 2),
    "file_retain_store_parent_sync_failure_is_visible_after_publish": (
        PERSISTENCE,
        3,
    ),
    "retain_manager_retries_failed_save_without_a_new_dirty_mark": (
        PERSISTENCE,
        4,
    ),
    "retain_store_roundtrip": (PERSISTENCE, 5),
    "retain_store_loads_legacy_v1_snapshot": (PERSISTENCE, 6),
    "retain_scalar_widening_migrates_with_runtime_event": (MIGRATION, 0),
    "retain_bounded_string_is_canonicalized_on_load": (MIGRATION, 1),
    "retain_struct_added_field_uses_declared_default_with_migration_event": (
        MIGRATION,
        2,
    ),
    "retain_struct_removed_field_drops_with_migration_event": (MIGRATION, 3),
    "retain_orphan_global_emits_runtime_event": (MIGRATION, 4),
    "retain_snapshot_migration_failure_does_not_partially_apply_earlier_values": (
        SAFE_RETAIN,
        0,
    ),
    "iec_6_5_6": (SAFE_RESTART, 0),
}


def select_live_retain_inventories(
    *,
    root: Path,
    inventory_documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Select the exact live retain mapping-debt rows from saved inventories."""

    merged: dict[str, Any] = {}
    for document in inventory_documents:
        inventories = document.get("inventories")
        if not isinstance(inventories, Mapping):
            raise ValueError("assertion inventory document lacks inventories")
        for discovery_id, inventory in inventories.items():
            if discovery_id in merged and merged[discovery_id] != inventory:
                raise ValueError(f"conflicting inventory {discovery_id}")
            merged[str(discovery_id)] = inventory
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and row.get("name") in BEHAVIOR_BY_TEST
    }
    if {name for _path, name in expected.values()} != set(BEHAVIOR_BY_TEST):
        raise ValueError("live retain denominator does not equal reviewed rules")
    selected = {
        discovery_id: merged[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in merged
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventories lack live retain mapping-debt rows")
    for discovery_id, raw in selected.items():
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    """Assign every selected retain assertion to reviewed behavior authority."""

    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("retain assertion document lacks inventories")
    behavior_counts = _invariant_behavior_counts(root)
    rows = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if name in seen_names:
            raise ValueError(f"duplicate retain test {name}")
        seen_names.add(name)
        try:
            default = BEHAVIOR_BY_TEST[name]
        except KeyError as error:
            raise ValueError(f"no reviewed retain rule for {path}:{name}") from error
        if default[1] >= behavior_counts[default[0]]:
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
        non_oracle_ids = []
        restart_phase = 0
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            if name == "iec_6_5_6":
                if "RestartMode :: Warm" in expression:
                    restart_phase = 0
                elif "RestartMode :: Cold" in expression:
                    restart_phase = 1
                behavior = (SAFE_RESTART, restart_phase)
            else:
                behavior = default
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": behavior[0],
                    "behavior_index": behavior[1],
                }
            )
            if _is_setup(name=name, expression=expression):
                setup_ids.append(assertion_id)
            elif _is_non_oracle(name=name, expression=expression):
                non_oracle_ids.append(assertion_id)
        if len(setup_ids) + len(non_oracle_ids) == len(assertion_rows):
            raise ValueError(f"{path}:{name} has no product-oracle assertion")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": non_oracle_ids,
            }
        )
    if seen_names != set(BEHAVIOR_BY_TEST):
        raise ValueError("retain assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_retain_semantic_rules",
        "assignments": rows,
    }


def _is_setup(*, name: str, expression: str) -> bool:
    if "TestHarness :: from_source" in expression:
        return True
    if name == "retain_store_reports_real_path_failures_without_silent_defaults":
        return (
            "std :: fs :: create_dir_all" in expression
            or "std :: fs :: write" in expression
            or "std :: fs :: remove_" in expression
        )
    if name == "file_retain_store_open_and_read_failures_are_visible":
        return "seed readable snapshot" in expression
    if name == "file_retain_store_parent_sync_failure_is_visible_after_publish":
        return "seed last good snapshot" in expression
    if name == "file_retain_store_replaces_snapshot_atomically":
        return "store initial snapshot" in expression
    if name == "retain_store_loads_legacy_v1_snapshot":
        return "std :: fs :: write" in expression
    if name == "retain_store_roundtrip":
        return (
            "ArrayValue :: from_untyped_parts" in expression
            or "EnumValue :: new" in expression
        )
    if "retain test parent" in expression or "read retain test " in expression:
        return True
    return False


def _is_non_oracle(*, name: str, expression: str) -> bool:
    weak_message_tests = {
        "retain_store_reports_real_path_failures_without_silent_defaults",
        "file_retain_store_open_and_read_failures_are_visible",
        "file_retain_store_parent_sync_failure_is_visible_after_publish",
        "retain_snapshot_migration_failure_does_not_partially_apply_earlier_values",
    }
    return name in weak_message_tests and ". to_string () . contains" in expression


def _invariant_behavior_counts(root: Path) -> dict[str, int]:
    counts = {}
    for invariant_id in {value[0] for value in BEHAVIOR_BY_TEST.values()}:
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant_id}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        counts[invariant_id] = len(behaviors)
    return counts


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _rooted(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--inventory", type=Path, action="append", required=True)
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    documents = [
        json.loads(_rooted(root, path).read_text()) for path in args.inventory
    ]
    selected = select_live_retain_inventories(
        root=root,
        inventory_documents=documents,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(f"runtime retain assignments generated: {len(assignments['assignments'])} mapped")
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError("retain plan generation arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
        )
        print(
            "runtime retain catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
