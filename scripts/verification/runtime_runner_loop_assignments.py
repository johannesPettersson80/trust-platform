"""Generate reviewed runtime runner-loop assertion assignments."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import generate_path_plans


SAFE_DEADLINE = "RT_SAFE_DEADLINE_001"
AUTOMATIC_RESTART = "RT_AUTOMATIC_RESTART_BOUND_001"
WATCHDOG_INSTALL = "RT_WATCHDOG_POLICY_INSTALL_001"

BEHAVIOR_BY_TEST: Mapping[str, tuple[str, int]] = {
    "scheduler_arms_execution_deadline_during_cycle": (SAFE_DEADLINE, 2),
    "cycle_deadline_from_zero_timeout_is_immediately_expired": (
        SAFE_DEADLINE,
        3,
    ),
    "cycle_deadline_guard_restores_previous_deadlines_after_success": (
        SAFE_DEADLINE,
        4,
    ),
    "cycle_deadline_guard_restores_after_runtime_error_and_contained_panic": (
        SAFE_DEADLINE,
        5,
    ),
    "disabled_watchdog_does_not_arm_cycle_deadlines": (SAFE_DEADLINE, 6),
    "automatic_restart_limiter_escalates_after_retry_budget": (
        AUTOMATIC_RESTART,
        2,
    ),
    "automatic_restart_backoff_is_bounded_and_monotonic": (
        AUTOMATIC_RESTART,
        3,
    ),
    "update_watchdog_zero_timeout_is_normalized_before_use": (
        WATCHDOG_INSTALL,
        1,
    ),
}


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
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
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and row.get("name") in BEHAVIOR_BY_TEST
    }
    if {name for _path, name in expected.values()} != set(BEHAVIOR_BY_TEST):
        raise ValueError(
            "live runner-loop denominator does not equal reviewed rules"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live runner-loop mapping debt")
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
        raise ValueError("runner-loop assertion document lacks inventories")
    behavior_counts = _invariant_behavior_counts(root)
    rows = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if name in seen_names:
            raise ValueError(f"duplicate runner-loop test {name}")
        seen_names.add(name)
        try:
            invariant_id, behavior_index = BEHAVIOR_BY_TEST[name]
        except KeyError as error:
            raise ValueError(
                f"no reviewed runner-loop rule for {path}:{name}"
            ) from error
        if behavior_index >= behavior_counts[invariant_id]:
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
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            assertion_behavior = _behavior_for_assertion(
                name=name,
                expression=expression,
                default=(invariant_id, behavior_index),
            )
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": assertion_behavior[0],
                    "behavior_index": assertion_behavior[1],
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
        raise ValueError("runner-loop assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_runner_loop_semantic_rules",
        "assignments": rows,
    }


def _is_setup(*, name: str, expression: str) -> bool:
    if name == (
        "cycle_deadline_guard_restores_after_runtime_error_and_contained_panic"
    ):
        return expression.startswith("panic !")
    if name == "scheduler_arms_execution_deadline_during_cycle":
        return "observed_deadline_for_cycle . lock" in expression
    return False


def _is_non_oracle(*, name: str, expression: str) -> bool:
    if (
        name == "scheduler_arms_execution_deadline_during_cycle"
        and expression.startswith("debug_assert ! (sentinel")
    ):
        return True
    return (
        name
        == "cycle_deadline_guard_restores_after_runtime_error_and_contained_panic"
        and "result . is_ok () || outcome == \"panic\"" in expression
    )


def _behavior_for_assertion(
    *,
    name: str,
    expression: str,
    default: tuple[str, int],
) -> tuple[str, int]:
    if (
        name
        == "cycle_deadline_guard_restores_after_runtime_error_and_contained_panic"
        and "runtime . execution_deadline () . is_some" in expression
    ):
        return (SAFE_DEADLINE, 2)
    return default


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
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    document = json.loads(_rooted(root, args.inventory).read_text())
    selected = select_live_inventories(
        root=root,
        inventory_document=document,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(
        "runtime runner-loop assignments generated: "
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
                "runner-loop plan generation arguments must be complete"
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
            "runtime runner-loop catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
