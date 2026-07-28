"""Generate reviewed assignments for the frozen Realtime T0 batch."""

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


TRANSPORT = "RT_REALTIME_T0_TRANSPORT_001"
CYCLE = "RT_REALTIME_T0_CYCLE_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/realtime-t0-15-semantic-lock.json"
)

BehaviorSelector = tuple[str, str]
TestKey = tuple[str, str]

TEST_BINDINGS: Mapping[TestKey, BehaviorSelector] = {
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs",
        "t0_shm_registration_fails_fast_when_root_path_is_not_directory",
    ): (
        TRANSPORT,
        "SHM_ROOT_DIRECTORY_FAILURE",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs",
        "t0_shm_registration_fails_fast_when_required_pinning_is_unavailable",
    ): (
        TRANSPORT,
        "REQUIRED_PAGE_PINNING_FAILURE",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs",
        "t0_shm_contract_mismatch_is_rejected_before_run",
    ): (
        TRANSPORT,
        "REGISTRATION_SCHEMA_HANDSHAKE_MISMATCH",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
        "qos_tier_route_legality_matrix_matches_contract",
    ): (
        TRANSPORT,
        "QOS_ROUTE_LEGALITY_MATRIX",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
        "t0_bind_rejects_non_t0_route_and_denies_fallback",
    ): (
        TRANSPORT,
        "T0_ROUTE_REJECTS_MESH_FALLBACK",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
        "t0_read_surfaces_stale_after_bounded_misses_and_spin_limit",
    ): (
        TRANSPORT,
        "STALE_MISS_AND_SPIN_EXHAUSTION",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
        "t0_error_codes_map_to_canonical_comms_contract_codes",
    ): (
        TRANSPORT,
        "T0_TO_COMMS_ERROR_MAPPING",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
        "t0_publish_and_read_track_overrun_and_latest_payload",
    ): (
        TRANSPORT,
        "LATEST_PAYLOAD_AND_OVERRUN_ACCOUNTING",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
        "t0_bind_enforces_schema_hash_and_fixed_layout_contract",
    ): (
        TRANSPORT,
        "BIND_SCHEMA_AND_FIXED_LAYOUT",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs",
        "t0_publish_and_read_reject_uninitialized_or_unpinned_channel_state",
    ): (
        TRANSPORT,
        "UNPINNED_IO_REJECTION",
    ),
    (
        "crates/trust-runtime/tests/realtime_t0_integration.rs",
        "realtime_t0_route_does_not_fallback_to_mesh_ip_path",
    ): (
        TRANSPORT,
        "SAME_PROCESS_FIXED_PAYLOAD_EXCHANGE",
    ),
    (
        "crates/trust-runtime/tests/realtime_t0_integration.rs",
        "realtime_t0_multi_process_shm_exchange_succeeds",
    ): (
        TRANSPORT,
        "CROSS_PROCESS_FIXED_PAYLOAD_VISIBILITY",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs",
        "t0_scheduler_budget_isolated_under_cloud_stress_across_cycles",
    ): (
        CYCLE,
        "PER_CYCLE_CLOUD_BUDGET",
    ),
    (
        "crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs",
        "t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
    ): (
        CYCLE,
        "PRE_TASK_POST_TASK_ORDER",
    ),
    (
        "crates/trust-runtime/tests/realtime_t0_integration.rs",
        "realtime_t0_determinism_holds_under_cloud_budget_pressure",
    ): (
        CYCLE,
        "FUNCTIONAL_SEQUENCE_UNDER_CLOUD_PRESSURE",
    ),
}


def select_live_inventories(
    *,
    root: Path,
    inventory_documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    merged: dict[str, Any] = {}
    for document in inventory_documents:
        rows = _canonical_inventories(root, document)
        for discovery_id, inventory in rows.items():
            if discovery_id in merged and merged[discovery_id] != inventory:
                raise ValueError(f"conflicting Realtime T0 inventory {discovery_id}")
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
            "live Realtime T0 denominator does not equal reviewed bindings"
        )
    selected = {
        discovery_id: merged[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in merged
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventories lack live Realtime T0 mapping-debt rows"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} Realtime T0 inventory identity drifted")
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
    seen_keys: set[TestKey] = set()
    dispositions: Counter[str] = Counter()
    kinds: Counter[str] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        key = (path, name)
        if key not in TEST_BINDINGS or key in seen_keys:
            raise ValueError(f"unexpected or duplicate Realtime T0 test {path}:{name}")
        seen_keys.add(key)
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
            kind = _string(assertion, "kind", assertion_id)
            function = _string(assertion, "function", assertion_id)
            selector = _behavior_for_assertion(
                path=path,
                name=name,
                expression=expression,
                function=function,
            )
            invariant_id, behavior_label = selector
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_indexes[
                        (invariant_id, behavior_label)
                    ],
                }
            )
            disposition = _disposition(
                name=name,
                expression=expression,
                kind=kind,
                function=function,
            )
            dispositions[disposition] += 1
            kinds[kind] += 1
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            elif disposition == "non_oracle_observation":
                non_oracle_ids.append(assertion_id)
        if len(setup_ids) + len(non_oracle_ids) == len(assertion_rows):
            raise ValueError(f"{path}:{name} has no direct product oracle")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": non_oracle_ids,
            }
        )
    if seen_keys != set(TEST_BINDINGS):
        raise ValueError("Realtime T0 assignments are not exhaustive")
    if sum(dispositions.values()) != sum(kinds.values()):
        raise ValueError("Realtime T0 disposition accounting drifted")
    return {
        "schema_version": 3,
        "basis": "reviewed_realtime_t0_semantic_group_and_assertion_rules",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions["setup_or_precondition"],
            "non_oracle_observation": dispositions["non_oracle_observation"],
            "assertion_kinds": dict(sorted(kinds.items())),
        },
        "assignments": rows,
    }


def _behavior_for_assertion(
    *,
    path: str,
    name: str,
    expression: str,
    function: str,
) -> BehaviorSelector:
    default = TEST_BINDINGS[(path, name)]
    if name == "realtime_t0_route_does_not_fallback_to_mesh_ip_path":
        if function != name:
            return TRANSPORT, "CROSS_PROCESS_FIXED_PAYLOAD_VISIBILITY"
        if any(
            marker in expression
            for marker in (
                "expect_err (\"mesh/ip fallback route",
                "T0ErrorCode :: ContractViolation",
                "fallback_denied_total",
            )
        ):
            return TRANSPORT, "T0_ROUTE_REJECTS_MESH_FALLBACK"
        return TRANSPORT, "SAME_PROCESS_FIXED_PAYLOAD_EXCHANGE"
    if name == "t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget":
        if any(
            marker in expression
            for marker in (
                "expect_err (\"post-task before pre-task",
                "out_of_order . code",
            )
        ):
            return CYCLE, "POST_BEFORE_PRE_REJECTION"
        if any(
            marker in expression
            for marker in (
                "assert_eq ! (first",
                "assert_eq ! (second",
                "denied_cloud_ops_total",
            )
        ):
            return CYCLE, "PER_CYCLE_CLOUD_BUDGET"
        return CYCLE, "PRE_TASK_POST_TASK_ORDER"
    return default


def _disposition(
    *,
    name: str,
    expression: str,
    kind: str,
    function: str,
) -> str:
    if kind == "method:expect":
        return "setup_or_precondition"
    if (
        name
        in {
            "realtime_t0_route_does_not_fallback_to_mesh_ip_path",
            "realtime_t0_determinism_holds_under_cloud_budget_pressure",
        }
        and function != name
    ):
        return "setup_or_precondition"
    return "oracle_bound"


def _canonical_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("Realtime T0 assertion document fields drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("Realtime T0 assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("Realtime T0 inventory is invalid")
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
        raise ValueError("saved Realtime T0 assertion inventory is not canonical")
    return canonical


def _behavior_indexes(root: Path) -> dict[tuple[str, str], int]:
    result: dict[tuple[str, str], int] = {}
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
            if not isinstance(label, str) or not label:
                raise ValueError(
                    f"{invariant_id} behavior {index} lacks named equals partition"
                )
            key = (invariant_id, label)
            if key in result:
                raise ValueError(f"duplicate behavior selector {invariant_id}:{label}")
            result[key] = index
    missing = set(TEST_BINDINGS.values()) - set(result)
    if missing:
        raise ValueError(f"missing Realtime T0 behavior selectors: {sorted(missing)}")
    return result


def _review_lock_document(inventories: Mapping[str, Any]) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            expression = _string(assertion, "expression", name)
            kind = _string(assertion, "kind", name)
            function = _string(assertion, "function", name)
            invariant_id, behavior_label = _behavior_for_assertion(
                path=path,
                name=name,
                expression=expression,
                function=function,
            )
            locked_assertions.append(
                {
                    "kind": kind,
                    "function": function,
                    "expression": expression,
                    "disposition": _disposition(
                        name=name,
                        expression=expression,
                        kind=kind,
                        function=function,
                    ),
                    "invariant_id": invariant_id,
                    "behavior_label": behavior_label,
                }
            )
        tests.append(
            {
                "path": path,
                "name": name,
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "REALTIME_T0_15_SEMANTIC_LOCK",
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
            f"reviewed Realtime T0 semantic lock is missing: {review_lock_path}"
        ) from error
    expected = _review_lock_document(inventories)
    if actual != expected:
        raise ValueError(
            "Realtime T0 reviewed expressions, dispositions, or bindings "
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
        lock = _review_lock_document(
            _canonical_inventories(root, selected)
        )
        _write_json(_rooted(root, args.review_lock), lock)
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
        review_lock_path=args.review_lock,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    summary = assignments["summary"]
    print(
        "Realtime T0 assignments generated: "
        f"{summary['tests']} tests, {summary['assertions']} assertions, "
        f"{summary['direct_oracle']} direct oracles, "
        f"{summary['setup_or_precondition']} setup, "
        f"{summary['non_oracle_observation']} non-oracle"
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
                "Realtime T0 plan generation arguments must be complete"
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
            "Realtime T0 catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
