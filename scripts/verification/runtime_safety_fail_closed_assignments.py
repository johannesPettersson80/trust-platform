"""Generate reviewed runtime fail-closed assertion assignments."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import generate_path_plans


MODBUS = "RT_MODBUS_DRIVER_001"
SAFE_STOP = "RT_SAFE_STOP_001"
SAFE_DEADLINE = "RT_SAFE_DEADLINE_001"
SAFE_RETAIN = "RT_SAFE_RETAIN_001"
SAFE_PANIC = "RT_SAFE_PANIC_001"
RESTART_BOUND = "RT_AUTOMATIC_RESTART_BOUND_001"

BEHAVIOR_BY_TEST: Mapping[str, tuple[str, int]] = {
    "modbus_warn_and_ignore_policy_do_not_fault_resource_state": (MODBUS, 5),
    "safe_state_write_failure_is_reported_without_losing_root_fault": (
        SAFE_DEADLINE,
        1,
    ),
    "deliberate_stop_and_fault_safe_state_paths_are_explicit": (SAFE_STOP, 1),
    "watchdog_restart_action_escalates_to_visible_fault": (RESTART_BOUND, 1),
    "watchdog_deadline_breach_before_commit_prevents_output_write": (
        SAFE_DEADLINE,
        0,
    ),
    "retain_save_failure_prevents_output_commit_when_due": (SAFE_RETAIN, 1),
    "panic_does_not_follow_ordinary_restart_policy": (SAFE_PANIC, 3),
    "requested_stop_retain_save_failure_is_visible": (SAFE_RETAIN, 2),
    "modbus_fault_policy_faults_resource_state": (MODBUS, 4),
    "panic_in_shared_resource_faults_without_poison_detection": (SAFE_PANIC, 0),
    "persistent_fault_restart_policy_escalates_to_visible_fault": (
        RESTART_BOUND,
        0,
    ),
    "requested_stop_applies_safe_outputs_before_thread_exits": (SAFE_STOP, 1),
}


def select_live_inventories(
    *,
    root: Path,
    inventory_documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
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
        raise ValueError(
            "live runtime fail-closed denominator does not equal reviewed rules"
        )
    selected = {
        discovery_id: merged[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in merged
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventories lack live runtime fail-closed mapping-debt rows"
        )
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
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("runtime fail-closed assertion document lacks inventories")
    behavior_counts = _invariant_behavior_counts(root)
    rows = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if name in seen_names:
            raise ValueError(f"duplicate runtime fail-closed test {name}")
        seen_names.add(name)
        try:
            default = BEHAVIOR_BY_TEST[name]
        except KeyError as error:
            raise ValueError(
                f"no reviewed runtime fail-closed rule for {path}:{name}"
            ) from error
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
        phase = default
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            if (
                name == "deliberate_stop_and_fault_safe_state_paths_are_explicit"
                and "fault-safe-state" in expression
            ):
                phase = (SAFE_PANIC, 1)
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": phase[0],
                    "behavior_index": phase[1],
                }
            )
            if _is_setup(expression=expression):
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
        raise ValueError("runtime fail-closed assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_safety_fail_closed_semantic_rules",
        "assignments": rows,
    }


def _is_setup(*, expression: str) -> bool:
    return (
        "IoAddress :: parse" in expression
        or "toml :: from_str" in expression
        or "ModbusTcpDriver :: from_params" in expression
        or "TestHarness :: from_source" in expression
        or "SharedGlobals :: from_runtime" in expression
        or ". spawn (" in expression
        or ". spawn_with_shared (" in expression
        or ". join () . expect" in expression
        or (
            ". is_empty ()" in expression
            and "must prevent physical output writes" not in expression
            and "watchdog breach before output commit" not in expression
        )
        or (
            "Instant :: now () < deadline" in expression
            and "resource did not fault in time" in expression
        )
    )


def _is_non_oracle(*, name: str, expression: str) -> bool:
    weak_message_tests = {
        "safe_state_write_failure_is_reported_without_losing_root_fault",
        "retain_save_failure_prevents_output_commit_when_due",
        "requested_stop_retain_save_failure_is_visible",
        "persistent_fault_restart_policy_escalates_to_visible_fault",
    }
    return name in weak_message_tests and ". to_string () . contains" in expression


def _invariant_behavior_counts(root: Path) -> dict[str, int]:
    counts = {}
    for invariant_id in {value[0] for value in BEHAVIOR_BY_TEST.values()} | {
        SAFE_PANIC
    }:
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
    selected = select_live_inventories(
        root=root,
        inventory_documents=documents,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(
        "runtime fail-closed assignments generated: "
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
            raise ValueError(
                "runtime fail-closed plan generation arguments must be complete"
            )
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
            "runtime fail-closed catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
