"""Generate reviewed assignments for the authority-ready debugger batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


CONTROL = "DEBUG_CONTROL_EXECUTION_001"
DAP = "DEBUG_DAP_PROTOCOL_SESSION_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/debug-authority-ready-8-semantic-lock.json"
)

BehaviorSelector = tuple[str, str]
TestKey = tuple[str, str]

TEST_BINDINGS: Mapping[TestKey, BehaviorSelector] = {
    (
        "crates/trust-runtime/tests/debug_stepping.rs",
        "step_over_stops_in_caller_after_call",
    ): (CONTROL, "STEP_EXECUTION"),
    (
        "crates/trust-runtime/tests/debug_stepping.rs",
        "vm_breakpoint_populates_debug_snapshot_for_stack_queries",
    ): (DAP, "NONBLOCKING_SNAPSHOT_AND_SYNTHETIC_MAIN_FRAME"),
    (
        "crates/trust-runtime/tests/debug_stepping.rs",
        "breakpoint_triggers_for_executed_branch",
    ): (CONTROL, "BREAKPOINT_RESOLUTION_AND_STOP_EVENT"),
    (
        "crates/trust-runtime/tests/debug_stepping.rs",
        "breakpoint_rehits_each_cycle_after_continue",
    ): (CONTROL, "CYCLIC_BREAKPOINT_REHIT"),
    (
        "crates/trust-runtime/tests/debug_stepping.rs",
        "breakpoint_set_after_launch_hits_next_cycle",
    ): (CONTROL, "LIVE_BREAKPOINT_MUTATION"),
    (
        "crates/trust-runtime/tests/debug_stepping.rs",
        "breakpoint_set_while_running_hits_on_subsequent_cycle",
    ): (CONTROL, "LIVE_BREAKPOINT_MUTATION"),
    (
        "crates/trust-runtime/src/host/debug/control/tests.rs",
        "pause_after_continue_while_waiting_emits_pause_stop",
    ): (DAP, "RUN_CONTROL_AND_TASK_THREAD_PROJECTION"),
    (
        "crates/trust-runtime/src/host/debug/resolve.rs",
        "resolve_breakpoint_prefers_statement_on_line",
    ): (CONTROL, "BREAKPOINT_RESOLUTION_AND_STOP_EVENT"),
}


def select_live_inventories(
    *,
    root: Path,
    inventory_documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    merged: dict[str, Any] = {}
    for document in inventory_documents:
        for discovery_id, inventory in _canonical_inventories(
            root, document
        ).items():
            if discovery_id in merged and merged[discovery_id] != inventory:
                raise ValueError(f"conflicting debugger inventory {discovery_id}")
            merged[discovery_id] = inventory

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
        and (str(row.get("path")), str(row.get("name"))) in TEST_BINDINGS
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError(
            "live debugger denominator does not equal reviewed bindings"
        )
    selected = {
        discovery_id: merged[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in merged
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventories lack authority-ready debugger debt rows"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} debugger inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    behavior_indexes = _behavior_indexes(root)
    rows = []
    seen: set[TestKey] = set()
    dispositions: Counter[str] = Counter()
    kinds: Counter[str] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        key = (path, name)
        if key not in TEST_BINDINGS or key in seen:
            raise ValueError(f"unexpected or duplicate debugger test {path}:{name}")
        seen.add(key)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        invariant_id, behavior_label = TEST_BINDINGS[key]
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            kind = _string(assertion, "kind", assertion_id)
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_indexes[
                        (invariant_id, behavior_label)
                    ],
                }
            )
            disposition = _disposition(kind)
            dispositions[disposition] += 1
            kinds[kind] += 1
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
        if len(setup_ids) == len(assertion_rows):
            raise ValueError(f"{path}:{name} has no direct product oracle")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("debugger assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_debugger_semantic_group_and_assertion_rules",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions["setup_or_precondition"],
            "non_oracle_observation": 0,
            "assertion_kinds": dict(sorted(kinds.items())),
        },
        "assignments": rows,
    }


def _disposition(kind: str) -> str:
    if kind.startswith("method:") or kind == "macro:panic":
        return "setup_or_precondition"
    if kind in {"macro:assert", "macro:assert_eq", "macro:assert_ne"}:
        return "oracle_bound"
    raise ValueError(f"unreviewed debugger assertion kind {kind!r}")


def _canonical_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("debugger assertion document fields drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("debugger assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("debugger inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append({"discovery_id": discovery_id, "path": path, "name": name})
        batch_rows.append(
            {key: value for key, value in raw.items() if key != "inventory_digest"}
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError("saved debugger assertion inventory is not canonical")
    return canonical


def _behavior_indexes(root: Path) -> dict[BehaviorSelector, int]:
    result: dict[BehaviorSelector, int] = {}
    for invariant_id in {value[0] for value in TEST_BINDINGS.values()}:
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant_id}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition")
            label = partition.get("equals") if isinstance(partition, Mapping) else None
            if isinstance(label, str) and label:
                key = (invariant_id, label)
                if key in result:
                    raise ValueError(
                        f"duplicate behavior selector {invariant_id}:{label}"
                    )
                result[key] = index
    missing = set(TEST_BINDINGS.values()) - set(result)
    if missing:
        raise ValueError(f"missing debugger behavior selectors: {sorted(missing)}")
    return result


def _review_lock_document(inventories: Mapping[str, Any]) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        invariant_id, behavior_label = TEST_BINDINGS[(path, name)]
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            locked_assertions.append(
                {
                    "kind": _string(assertion, "kind", name),
                    "function": _string(assertion, "function", name),
                    "expression": _string(assertion, "expression", name),
                    "disposition": _disposition(
                        _string(assertion, "kind", name)
                    ),
                    "invariant_id": invariant_id,
                    "behavior_label": behavior_label,
                }
            )
        tests.append({"path": path, "name": name, "assertions": locked_assertions})
    return {
        "schema_version": 1,
        "id": "DEBUG_AUTHORITY_READY_8_SEMANTIC_LOCK",
        "tests": tests,
    }


def _validate_review_lock(
    root: Path,
    inventories: Mapping[str, Any],
    review_lock_path: Path,
) -> None:
    path = (
        review_lock_path
        if review_lock_path.is_absolute()
        else root / review_lock_path
    )
    try:
        actual = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise ValueError(
            f"reviewed debugger semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "debugger reviewed expressions, dispositions, or bindings "
            "drifted from semantic lock"
        )


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
    parser.add_argument("--review-lock", type=Path, default=REVIEW_LOCK_PATH)
    parser.add_argument("--emit-review-lock", action="store_true")
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
    if args.emit_review_lock:
        _write_json(
            _rooted(root, args.review_lock),
            _review_lock_document(_canonical_inventories(root, selected)),
        )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
        review_lock_path=args.review_lock,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    summary = assignments["summary"]
    print(
        "debugger assignments generated: "
        f"{summary['tests']} tests, {summary['assertions']} assertions, "
        f"{summary['direct_oracle']} direct oracles, "
        f"{summary['setup_or_precondition']} setup"
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
                "debugger plan generation arguments must be complete"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="runtime-remaining",
        )
        print(
            "debugger catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
