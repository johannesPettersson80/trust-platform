"""Generate reviewed assignments for the frozen runtime-reliability batch."""

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


PATH = "crates/trust-runtime/tests/runtime_reliability.rs"
INVENTORY_PATH = Path(
    "target/gate-artifacts/verification/"
    "runtime-reliability-4-assertions.json"
)
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-reliability-4-semantic-lock.json"
)

HARNESS_IO = "RT_HARNESS_BOUNDARY_IO_001"
RESTART_TIME = "RT_SAFE_RESTART_TIME_002"
RETAIN_CODEC = "RT_RETAIN_PERSISTENCE_CODEC_001"
DEADLINE = "RT_SAFE_DEADLINE_001"
SAFE_RESTART = "RT_SAFE_RESTART_001"

DIRECT_BOUND_IO = "DIRECT_API_BOUND_ADDRESS_ROUND_TRIP"
WARM_TIME = "WARM_RESTART_PRESERVES_TIME_AND_REINITIALIZES_STATE"
UNSAVED_DROP = "UNFLUSHED_DIRTY_STATE_NOT_PERSISTED_ON_DROP"
PROGRAM_WARM = "PROGRAM_RETAIN_WARM_ROUNDTRIP"
SPAWNED_WATCHDOG = "SPAWNED_RESOURCE_WATCHDOG_TIMEOUT_FAULTS"
COLD_SKIPS_LOAD = "COLD_RESTART_WITH_RETAIN_STORE_SKIPS_LOAD"

STARTUP_IO_TEST = "e2e_startup_io_restart"
UNSAVED_DROP_TEST = "retain_power_loss_does_not_persist_unsaved"
WATCHDOG_TEST = "watchdog_faults_resource_on_overrun"
RETAIN_ROUNDTRIP_TEST = "e2e_retain_roundtrip_restart"

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_B02D0E3CFEC34E755FC0": STARTUP_IO_TEST,
    "DISC_B0616A747435CAE04D2A": UNSAVED_DROP_TEST,
    "DISC_C101BBAEC06311987C74": WATCHDOG_TEST,
    "DISC_EE4014B513F4E29CDCC3": RETAIN_ROUNDTRIP_TEST,
}

COMMON_EXCLUSION = (
    "This four-test batch does not establish arbitrary-source, IEC-wide, "
    "cross-platform, physical-I/O, or complete runtime safety correctness."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    STARTUP_IO_TEST: (
        "The direct-I/O case is bounded to one BOOL input binding, one BOOL "
        "output binding, and one successful explicit cycle. It does not prove "
        "physical I/O, arbitrary address or value types, driver faults, or "
        "multiple-cycle behavior.",
        "The restart observation proves only that one warm restart preserves "
        "the reviewed ten-millisecond logical time. It does not inspect I/O "
        "bindings or values after restart, cold restart, timers, tasks, or "
        "restart failure atomicity.",
        COMMON_EXCLUSION,
    ),
    UNSAVED_DROP_TEST: (
        "Plain TestHarness destruction is not a physical power loss or a "
        "graceful requested-stop path. This case proves only that the reviewed "
        "dirty value is absent after drop; it does not prove that no empty "
        "file was created.",
        "The case provides no filesystem durability, cadence, explicit-save, "
        "requested-stop, save-failure, atomic replacement, or crash-consistency "
        "proof.",
        COMMON_EXCLUSION,
    ),
    WATCHDOG_TEST: (
        "The watchdog case uses an effectively-zero wall-clock timeout and a "
        "100 ms polling guard. The guard bounds the test; it is not a product "
        "latency guarantee or a generally supported timeout threshold.",
        "StepClock drives scheduler time only and does not trigger the "
        "wall-clock watchdog. This case does not prove output-commit, "
        "safe-state, automatic-restart, debugger-pause, or recovery behavior.",
        COMMON_EXCLUSION,
    ),
    RETAIN_ROUNDTRIP_TEST: (
        "The retain case is bounded to one program-scoped RETAIN INT, one "
        "file-backed snapshot, and the in-process harness helper. It does not "
        "prove arbitrary retain values, multiple programs, migration, "
        "filesystem durability, or failure atomicity.",
        "The warm and cold checks do not cover the spawned resource "
        "pending-restart signal path, automatic restart, launcher startup, "
        "hot reload, or the internal persisted key encoding.",
        COMMON_EXCLUSION,
    ),
}

BehaviorSelector = tuple[str, str]
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
    ) -> None:
        routes = result.setdefault(test_name, {})
        for assertion_id, disposition in (
            *((value, "setup_or_precondition") for value in setup),
            *((value, "oracle_bound") for value in oracle),
        ):
            if assertion_id in routes:
                raise ValueError(f"duplicate reviewed assertion {assertion_id}")
            routes[assertion_id] = (invariant, behavior, disposition)

    register(
        STARTUP_IO_TEST,
        HARNESS_IO,
        DIRECT_BOUND_IO,
        setup=("ASSERT_762D6440DC5230262D0DCED5",),
        oracle=(
            "ASSERT_8E1AA64880685A4C7286CDC1",
            "ASSERT_454E6285987729FE62F12AF2",
            "ASSERT_FAC50CE54D4A8E2F2AB44F45",
            "ASSERT_83BD456C94A238DE8B46386D",
            "ASSERT_016D9761E46077AE31861D1E",
            "ASSERT_56FD4290842A17DAB5B49A81",
        ),
    )
    register(
        STARTUP_IO_TEST,
        RESTART_TIME,
        WARM_TIME,
        oracle=(
            "ASSERT_58D0B9B90F8F53883F273E1A",
            "ASSERT_1120F55A609DB8E5AA5852E6",
        ),
    )

    register(
        UNSAVED_DROP_TEST,
        RETAIN_CODEC,
        UNSAVED_DROP,
        setup=("ASSERT_8EC5134DC034DFA4707A4869",),
        oracle=(
            "ASSERT_ACC13F06E8BCFB492FEC9112",
            "ASSERT_E3653B762F145D17FAF7D909",
        ),
    )

    register(
        WATCHDOG_TEST,
        DEADLINE,
        SPAWNED_WATCHDOG,
        setup=(
            "ASSERT_40D4D15A6F0A003E5F3D3AC4",
            "ASSERT_E52B014D66F48216BFB65463",
        ),
        oracle=(
            "ASSERT_09FB9A0827AEAA0B5ADC9E8A",
            "ASSERT_AED9713054342BC8D406EF5D",
            "ASSERT_C8FF1EAEAB827C698D718E02",
        ),
    )

    register(
        RETAIN_ROUNDTRIP_TEST,
        RETAIN_CODEC,
        PROGRAM_WARM,
        setup=(
            "ASSERT_F18708F5B196486556E99305",
            "ASSERT_D221D1BF992D4D373641D979",
        ),
        oracle=(
            "ASSERT_6174C37D34E360E4BA3758E5",
            "ASSERT_05F686783F7F36102B0E69F7",
        ),
    )
    register(
        RETAIN_ROUNDTRIP_TEST,
        SAFE_RESTART,
        COLD_SKIPS_LOAD,
        oracle=(
            "ASSERT_C03387FB83C16AA26DDF4272",
            "ASSERT_86E64245E65676F517F3B4A1",
        ),
    )
    return result


REVIEWED_ROUTES: Mapping[str, Mapping[str, Route]] = _reviewed_routes()


def _denominator_row_is_live(
    row: Mapping[str, Any],
    *,
    discovery_id: str,
    expected_name: str,
) -> bool:
    if (
        row.get("discovery_id") != discovery_id
        or row.get("path") != PATH
        or row.get("name") != expected_name
    ):
        return False
    reviewed_nonmapping = (
        row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
    )
    catalog_mapped = (
        row.get("disposition") == "catalog_mapped"
        and row.get("catalog_test_id")
        == f"TEST_AUTO_{discovery_id.removeprefix('DISC_')}"
    )
    return reviewed_nonmapping or catalog_mapped


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    """Select the exact pre-map or post-map denominator identities."""

    inventories = _canonical_inventories(root, inventory_document)
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    matched: dict[str, str] = {}
    for row in denominator.get("reviews", []):
        if not isinstance(row, Mapping):
            continue
        discovery_id = str(row.get("discovery_id"))
        if discovery_id not in REVIEWED_IDENTITIES:
            continue
        expected_name = REVIEWED_IDENTITIES[discovery_id]
        if not _denominator_row_is_live(
            row,
            discovery_id=discovery_id,
            expected_name=expected_name,
        ):
            raise ValueError(
                f"{discovery_id} runtime-reliability denominator "
                "identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate runtime-reliability denominator {discovery_id}"
            )
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live runtime-reliability denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed runtime-reliability identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} runtime-reliability inventory "
                "identity drifted"
            )
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
    behavior_indexes: Mapping[BehaviorSelector, int],
) -> dict[str, Any]:
    rows = []
    seen: set[str] = set()
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[BehaviorSelector] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (
            path != PATH
            or name not in REVIEWED_ROUTES
            or name in seen
            or REVIEWED_IDENTITIES.get(discovery_id) != name
        ):
            raise ValueError(
                "unexpected or duplicate runtime-reliability test "
                f"{discovery_id}:{path}:{name}"
            )
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignment_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            selector = (invariant, behavior)
            try:
                behavior_index = behavior_indexes[selector]
            except KeyError as error:
                raise ValueError(
                    f"missing behavior selector {invariant}:{behavior}"
                ) from error
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant,
                    "behavior_index": behavior_index,
                }
            )
            dispositions[disposition] += 1
            behavior_counts[selector] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignment_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(REVIEWED_ROUTES):
        raise ValueError("runtime-reliability assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 17, "setup_or_precondition": 6}
    ):
        raise ValueError(
            "runtime-reliability assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_reliability_4_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions[
                "setup_or_precondition"
            ],
            "non_oracle_observation": 0,
            "behavior_assertions": {
                f"{invariant}:{behavior}": count
                for (invariant, behavior), count in sorted(
                    behavior_counts.items()
                )
            },
        },
        "assignments": rows,
    }


def _route(*, test_name: str, assertion_id: str) -> Route:
    routes = REVIEWED_ROUTES.get(test_name)
    if routes is None:
        raise ValueError(
            f"unreviewed runtime-reliability test {test_name!r}"
        )
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            "unreviewed runtime-reliability assertion "
            f"{test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError(
            "runtime-reliability assertion document fields drifted"
        )
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("runtime-reliability document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("runtime-reliability inventory is invalid")
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
            "saved runtime-reliability assertion inventory is not canonical"
        )
    return canonical


def _behavior_indexes(root: Path) -> dict[BehaviorSelector, int]:
    required = {
        (invariant, behavior)
        for routes in REVIEWED_ROUTES.values()
        for invariant, behavior, _disposition in routes.values()
    }
    result: dict[BehaviorSelector, int] = {}
    for invariant_id in {selector[0] for selector in required}:
        matches = list(
            (root / "verification/invariants").glob(
                f"**/{invariant_id}.toml"
            )
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition")
            label = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if not isinstance(label, str) or not label:
                continue
            selector = (invariant_id, label)
            if selector in result:
                raise ValueError(
                    f"duplicate behavior selector {invariant_id}:{label}"
                )
            result[selector] = index
    missing = required - set(result)
    if missing:
        raise ValueError(
            "missing runtime-reliability behavior selectors: "
            f"{sorted(missing)}"
        )
    return {selector: result[selector] for selector in sorted(required)}


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (
            REVIEWED_IDENTITIES.get(discovery_id) != name
            or path != PATH
            or name not in REVIEWED_ROUTES
        ):
            raise ValueError(
                "unexpected runtime-reliability test "
                f"{discovery_id}:{path}:{name}"
            )
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
                "discovery_id": discovery_id,
                "path": path,
                "name": name,
                "source_digest": _string(
                    raw,
                    "source_digest",
                    discovery_id,
                ),
                "exclusions": list(TEST_EXCLUSIONS[name]),
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "RUNTIME_RELIABILITY_4_SEMANTIC_LOCK",
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
            "reviewed runtime-reliability semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "runtime-reliability expressions or bindings drifted "
            "from semantic lock"
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
    parser.add_argument(
        "--inventory",
        type=Path,
        default=INVENTORY_PATH,
    )
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
        "runtime-reliability assignments generated: "
        f"{summary['tests']} tests, {summary['direct_oracle']} direct, "
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
                "runtime-reliability plan arguments must be complete"
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
            "runtime-reliability catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
