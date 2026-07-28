"""Generate reviewed assignments for the resource scheduler E2E batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import (
    generate_path_plans,
)


PATH = "crates/trust-runtime/tests/e2e_scheduler.rs"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/scheduler-5-semantic-lock.json"
)

READINESS_INVARIANT = "RT_TASK_READINESS_001"
E2E_INVARIANT = "RT_RESOURCE_SCHEDULER_E2E_001"
MULTIPLE_DUE = "MULTIPLE_PERIODS_DUE"
PERIODIC_EVENT = "PERIODIC_AND_EVENT_RESOURCE_SEQUENCE"
BACKWARD_CLOCK = "BACKWARD_CLOCK_NO_REPLAY_RESOURCE_SEQUENCE"
FORWARD_CLOCK = "FORWARD_CLOCK_COALESCING_RESOURCE_SEQUENCE"
FAULT_LATCH = "EXECUTION_FAULT_LATCH_AND_SUBSEQUENT_REJECTION"
TRACE_REPRODUCIBILITY = "MANUAL_CLOCK_RUNTIME_EVENT_TRACE_REPRODUCIBILITY"

PERIODIC_TEST = "periodic_and_event"
BACKWARD_TEST = (
    "backward_clock_step_does_not_replay_or_overrun_periodic_task"
)
RESUMED_TEST = "resumed_clock_jump_runs_once_and_records_missed_intervals"
FAULT_TEST = "overrun_and_fault"
TRACE_TEST = "trace_determinism"

GLOBAL_EXCLUSION = (
    "These in-process ManualClock tests do not prove operating-system clock "
    "behavior, real-time jitter, external I/O, or hardware scheduling."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    PERIODIC_TEST: (
        "The test covers one periodic and one event task with INT counters at "
        "0, 10, and 20 ms; it does not prove wider counter types.",
        "Counter values prove activation counts, not exact same-tick execution "
        "order, priority contention, or execution latency.",
        GLOBAL_EXCLUSION,
    ),
    BACKWARD_TEST: (
        "The test covers one 10 ms periodic task and one 10-to-2 ms backward "
        "step followed by recovery at 20 ms.",
        "It does not prove clock wrap, saturation, repeated regressions, "
        "multiple-task ordering, or real clock correction.",
        GLOBAL_EXCLUSION,
    ),
    RESUMED_TEST: (
        "The test covers one forward jump to 55 ms for one 10 ms INT task.",
        "It proves one coalesced execution and four recorded misses, not replay "
        "order, saturation, jitter, or multiple-task behavior.",
        GLOBAL_EXCLUSION,
    ),
    FAULT_TEST: (
        "The test covers a DivisionByZero under the reviewed default resource "
        "fault behavior after one periodic overrun.",
        "It does not prove safe outputs, restart/recovery policy, contained "
        "panic, concurrent faults, or persistence of fault evidence.",
        GLOBAL_EXCLUSION,
    ),
    TRACE_TEST: (
        "The test proves equality of two in-process traces for one fixed "
        "ManualClock sequence; it does not assert the expected event contents.",
        "It does not prove durable trace storage, external telemetry ordering, "
        "cross-process replay, or cross-platform byte identity.",
        GLOBAL_EXCLUSION,
    ),
}

Route = tuple[str, str, str]


def _reviewed_routes() -> dict[str, dict[str, Route]]:
    result: dict[str, dict[str, Route]] = {}

    def register(
        test_name: str,
        invariant: str,
        behavior: str,
        *,
        setup: tuple[str, ...] = (),
        oracle: tuple[str, ...] = (),
        non_oracle: tuple[str, ...] = (),
    ) -> None:
        routes = result.setdefault(test_name, {})
        disposition_rows = (
            *((value, "setup_or_precondition") for value in setup),
            *((value, "oracle_bound") for value in oracle),
            *((value, "non_oracle_observation") for value in non_oracle),
        )
        for assertion_id, disposition in disposition_rows:
            if assertion_id in routes:
                raise ValueError(f"duplicate reviewed assertion {assertion_id}")
            routes[assertion_id] = (invariant, behavior, disposition)

    register(
        PERIODIC_TEST,
        E2E_INVARIANT,
        PERIODIC_EVENT,
        setup=(
            "ASSERT_C1719FE3873600DF2C265A56",
            "ASSERT_E953771610B95BA2800DAAB3",
            "ASSERT_BB2795A24631DF9CA57FBEB1",
        ),
        oracle=(
            "ASSERT_B15AE761D1A634F486015055",
            "ASSERT_D801B793590A524699098C30",
            "ASSERT_2C7DE32EBB6B07B99EF714B7",
            "ASSERT_58BDD56EBBAE40EA0AEDFA43",
            "ASSERT_9A9A528E88C1679C7CC9F85D",
        ),
        non_oracle=(
            "ASSERT_7B2E2FD046F6927265890213",
            "ASSERT_CEA06C2FF6156619A49DA18F",
        ),
    )
    register(
        BACKWARD_TEST,
        E2E_INVARIANT,
        BACKWARD_CLOCK,
        setup=(
            "ASSERT_FF785EA7B95BBA8E9FBF8D07",
            "ASSERT_A1FFBB1A608FB9250B6B0D0D",
        ),
        oracle=(
            "ASSERT_3E638013606E5EE14E6D485A",
            "ASSERT_4549A9DFB7E5D2220D4AF013",
            "ASSERT_37CDDA3EA71AE2B3F59A4054",
            "ASSERT_A41BFE3FE35A8D8B87E11F5B",
            "ASSERT_B646317DB1A113FD85FBAA7F",
            "ASSERT_B642C89987F86CC61FFEE2A9",
            "ASSERT_9A914FCC8998FFFBA24D5CBB",
        ),
        non_oracle=(
            "ASSERT_7EA16F5EA854C96B06117431",
            "ASSERT_87481090FD103AE14118A6D4",
        ),
    )
    register(
        RESUMED_TEST,
        E2E_INVARIANT,
        FORWARD_CLOCK,
        setup=(
            "ASSERT_7AE489615305FC85BD9569C9",
            "ASSERT_8F9B6C5529D0013070CE99B8",
        ),
        oracle=(
            "ASSERT_B89D36FABDFB52B87C8BA3B5",
            "ASSERT_BA9156CA9A4C5C858AC6294C",
            "ASSERT_F21F23EF67538674299B282E",
        ),
        non_oracle=(
            "ASSERT_A128031DE0C28AC2536DADFD",
            "ASSERT_55A903BEE41C0DEEA412BB6C",
        ),
    )
    register(
        RESUMED_TEST,
        READINESS_INVARIANT,
        MULTIPLE_DUE,
        oracle=("ASSERT_CFDAE48447F0702C0D4EEFF3",),
    )
    register(
        FAULT_TEST,
        E2E_INVARIANT,
        FAULT_LATCH,
        setup=("ASSERT_640437B6F06ED2535F6238D4",),
        oracle=(
            "ASSERT_A06D8266BAA486E280E301AB",
            "ASSERT_B2A7D6C74F526E5ACD736EFE",
            "ASSERT_370D9B2161A9ED4ECFDA21F1",
            "ASSERT_E04172C261FACB13778ED395",
            "ASSERT_BBA20E0C7C23E7A03F666377",
            "ASSERT_D8AB4D928BC246EC76CF5799",
        ),
    )
    register(
        FAULT_TEST,
        READINESS_INVARIANT,
        MULTIPLE_DUE,
        oracle=("ASSERT_044CC0B303AC001971DECD7B",),
    )
    register(
        TRACE_TEST,
        E2E_INVARIANT,
        TRACE_REPRODUCIBILITY,
        setup=("ASSERT_47268C8027ADABFE65761E39",),
        oracle=(
            "ASSERT_D0E2CE00B681C4B2B9B7C72D",
            "ASSERT_5D694812864CE8D393C95FA3",
            "ASSERT_1E2260CCC6F0776D6F752510",
            "ASSERT_E2EDC659C048C6FE1AC7BA5F",
        ),
    )
    return result


REVIEWED_ROUTES: Mapping[str, Mapping[str, Route]] = _reviewed_routes()


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): str(row["name"])
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") == PATH
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and str(row.get("name")) in REVIEWED_ROUTES
    }
    if set(expected.values()) != set(REVIEWED_ROUTES):
        raise ValueError("live scheduler denominator differs from review")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks scheduler mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            expected[discovery_id],
        ):
            raise ValueError(f"{discovery_id} scheduler identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    return _generate_assignment_document(inventories, _behavior_indexes(root))


def _generate_assignment_document(
    inventories: Mapping[str, Any],
    behavior_indexes: Mapping[tuple[str, str], int],
) -> dict[str, Any]:
    rows = []
    seen: set[str] = set()
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[str] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in REVIEWED_ROUTES or name in seen:
            raise ValueError(
                f"unexpected or duplicate scheduler test {path}:{name}"
            )
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignments = []
        setup_ids = []
        non_oracle_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            elif disposition == "non_oracle_observation":
                non_oracle_ids.append(assertion_id)
            assignments.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant,
                    "behavior_index": behavior_indexes[
                        (invariant, behavior)
                    ],
                }
            )
            dispositions[disposition] += 1
            behavior_counts[f"{invariant}:{behavior}"] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignments,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": non_oracle_ids,
            }
        )
    if seen != set(REVIEWED_ROUTES):
        raise ValueError("scheduler assignments are not exhaustive")
    if sum(dispositions.values()) != 42:
        raise ValueError("scheduler assertion count drifted from reviewed 42")
    return {
        "schema_version": 3,
        "basis": "reviewed_scheduler_5_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions[
                "setup_or_precondition"
            ],
            "non_oracle_observation": dispositions[
                "non_oracle_observation"
            ],
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _route(*, test_name: str, assertion_id: str) -> Route:
    routes = REVIEWED_ROUTES.get(test_name)
    if routes is None:
        raise ValueError(f"unreviewed scheduler test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed scheduler assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("scheduler assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("scheduler document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("scheduler inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append(
            {"discovery_id": discovery_id, "path": path, "name": name}
        )
        batch_rows.append(
            {
                key: value
                for key, value in raw.items()
                if key != "inventory_digest"
            }
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError(
            "saved scheduler assertion inventory is not canonical"
        )
    return canonical


def _behavior_indexes(root: Path) -> dict[tuple[str, str], int]:
    required = {
        route[:2]
        for routes in REVIEWED_ROUTES.values()
        for route in routes.values()
    }
    result = {}
    for invariant in sorted({value[0] for value in required}):
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition")
            label = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if isinstance(label, str) and label:
                result[(invariant, label)] = index
    if not required <= set(result):
        raise ValueError("scheduler invariant behaviors drifted")
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in REVIEWED_ROUTES:
            raise ValueError(f"unexpected scheduler test {path}:{name}")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            locked_assertions.append(
                {
                    "assertion_id": assertion_id,
                    "kind": _string(assertion, "kind", assertion_id),
                    "function": _string(
                        assertion,
                        "function",
                        assertion_id,
                    ),
                    "expression": _string(
                        assertion,
                        "expression",
                        assertion_id,
                    ),
                    "disposition": disposition,
                    "invariant_id": invariant,
                    "behavior_label": behavior,
                }
            )
        tests.append(
            {
                "path": path,
                "name": name,
                "exclusions": list(TEST_EXCLUSIONS[name]),
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "SCHEDULER_5_SEMANTIC_LOCK",
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
            f"reviewed scheduler semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "scheduler expressions or bindings drifted from semantic lock"
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
    parser.add_argument("--inventory", type=Path, required=True)
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
    document = json.loads(_rooted(root, args.inventory).read_text())
    selected = select_live_inventories(
        root=root,
        inventory_document=document,
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
        "scheduler assignments generated: "
        f"{summary['tests']} tests, {summary['direct_oracle']} direct, "
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
            raise ValueError("scheduler plan arguments must be complete")
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
            f"scheduler catalog plans generated: {index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
