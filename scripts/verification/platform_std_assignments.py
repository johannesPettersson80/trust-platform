"""Generate reviewed assignments for the frozen platform clock batch."""

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


PATH = "crates/trust-runtime/tests/platform_std.rs"
INVARIANT = "RT_RUNTIME_CLOCK_BOUNDARY_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/platform-std-4-semantic-lock.json"
)

STDCLOCK = "STDCLOCK_NONDECREASING_ELAPSED_TIME"
NON_PACING_TICK = "MANUAL_RESOURCE_TICK_DOES_NOT_SLEEP"
TIME_LOGICAL = "TIME_BUILTIN_RETURNS_INJECTED_LOGICAL_TIME"
INJECTED_READINESS = "RESOURCE_READINESS_USES_INJECTED_CLOCK_ONLY"

MONOTONIC_TEST = "monotonic_time"
NON_PACING_TEST = "sleep_not_in_tests"
TIME_TEST = "time_builtin_uses_runtime_clock"
READINESS_TEST = (
    "scheduler_ignores_wall_time_when_monotonic_clock_is_fixed"
)

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_04E3AE8E579EE7353332": MONOTONIC_TEST,
    "DISC_27187199BD3E588FC6BB": NON_PACING_TEST,
    "DISC_3D9BC624619267817197": TIME_TEST,
    "DISC_AC89BA6FAA7BC42ACCBC": READINESS_TEST,
}

TEST_ORDER = (
    MONOTONIC_TEST,
    NON_PACING_TEST,
    TIME_TEST,
    READINESS_TEST,
)

COMMON_EXCLUSION = (
    "This batch does not specify or prove CURRENT_DT, host wall-clock epoch, "
    "timezone, precision, range, failure handling, or replay control."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    MONOTONIC_TEST: (
        "The StdClock case proves nondecrease for two reviewed samples only; "
        "equality is permitted. It proves no strict progress, minimum "
        "resolution, sleep duration, long-run ordering, or saturation case.",
        COMMON_EXCLUSION,
    ),
    NON_PACING_TEST: (
        "The no-sleep case is bounded to one direct ResourceRunner tick with "
        "one ManualClock. It does not cover all tests, TestHarness cycles, "
        "the spawned resource loop, or sleeping elsewhere in the runtime.",
        COMMON_EXCLUSION,
    ),
    TIME_TEST: (
        "The TIME case is bounded to one zero-argument TIME() call after one "
        "explicit 123 ms harness advance. It does not cover CURRENT_DT, a "
        "civil wall-clock value, additional calls, or other clock surfaces.",
        COMMON_EXCLUSION,
    ),
    READINESS_TEST: (
        "The readiness case is bounded to one one-millisecond periodic task, "
        "one fixed ManualClock sample, and one explicit advance to its first "
        "deadline. It does not observe host wall-clock movement, other task "
        "modes, overruns, backward jumps, or the spawned resource loop.",
        COMMON_EXCLUSION,
    ),
}

Route = tuple[str, str, str]


def _reviewed_routes() -> dict[str, dict[str, Route]]:
    result: dict[str, dict[str, Route]] = {}

    def register(
        test_name: str,
        behavior: str,
        *,
        setup: tuple[str, ...],
        oracle: tuple[str, ...],
    ) -> None:
        routes = result.setdefault(test_name, {})
        for assertion_id, disposition in (
            *((value, "setup_or_precondition") for value in setup),
            *((value, "oracle_bound") for value in oracle),
        ):
            if assertion_id in routes:
                raise ValueError(f"duplicate reviewed assertion {assertion_id}")
            routes[assertion_id] = (INVARIANT, behavior, disposition)

    register(
        MONOTONIC_TEST,
        STDCLOCK,
        setup=(),
        oracle=("ASSERT_6E5D52488F89670FFCC0E798",),
    )
    register(
        NON_PACING_TEST,
        NON_PACING_TICK,
        setup=(
            "ASSERT_A0EF7027A43C0D601198907C",
            "ASSERT_66F13840E995E491FD38D834",
        ),
        oracle=("ASSERT_3B62C2259A4B19A57704214F",),
    )
    register(
        TIME_TEST,
        TIME_LOGICAL,
        setup=("ASSERT_C3886323870778602B7514E4",),
        oracle=(
            "ASSERT_7D87EF4A72CF70CEE1C474D8",
            "ASSERT_EA17E7989E59E9695EF1C9CD",
        ),
    )
    register(
        READINESS_TEST,
        INJECTED_READINESS,
        setup=(
            "ASSERT_D792130F568E91212A16FBC2",
            "ASSERT_69679646E0AB928B976A1462",
            "ASSERT_E5E9409C6F0978EDAD54912C",
            "ASSERT_0C91C2E009B40CDDBB5E937E",
        ),
        oracle=(
            "ASSERT_6CCB23303B92847FDD391982",
            "ASSERT_9EFB5A6F7AB986FDA7C3D9D2",
        ),
    )
    return result


REVIEWED_ROUTES: Mapping[str, Mapping[str, Route]] = _reviewed_routes()


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    """Select the exact live denominator identities reviewed for this batch."""

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
        if (
            row.get("path") != PATH
            or row.get("name") != expected_name
            or not (reviewed_nonmapping or catalog_mapped)
        ):
            raise ValueError(
                f"{discovery_id} platform std denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate platform std denominator {discovery_id}"
            )
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live platform std denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed platform std identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} platform std inventory identity drifted"
            )
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    """Assign all thirteen reviewed assertions to clock-boundary authority."""

    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    return _generate_assignment_document(inventories, _behavior_indexes(root))


def _generate_assignment_document(
    inventories: Mapping[str, Any],
    behavior_indexes: Mapping[str, int],
) -> dict[str, Any]:
    rows = []
    seen: set[str] = set()
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[str] = Counter()
    ordered_names = []
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
                "unexpected or duplicate platform std test "
                f"{discovery_id}:{path}:{name}"
            )
        seen.add(name)
        ordered_names.append(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignment_rows = []
        setup_ids = []
        test_behaviors: set[str] = set()
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            test_behaviors.add(behavior)
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant,
                    "behavior_index": behavior_indexes[behavior],
                }
            )
            dispositions[disposition] += 1
            behavior_counts[behavior] += 1
        if len(test_behaviors) != 1:
            raise ValueError(f"{path}:{name} does not map to one behavior")
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
        raise ValueError("platform std assignments are not exhaustive")
    if tuple(ordered_names) != TEST_ORDER:
        raise ValueError("platform std tests drifted from invariant order")
    if dispositions != Counter(
        {"setup_or_precondition": 7, "oracle_bound": 6}
    ):
        raise ValueError(
            "platform std assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_platform_std_4_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions[
                "setup_or_precondition"
            ],
            "non_oracle_observation": 0,
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _route(*, test_name: str, assertion_id: str) -> Route:
    routes = REVIEWED_ROUTES.get(test_name)
    if routes is None:
        raise ValueError(f"unreviewed platform std test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed platform std assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("platform std assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("platform std document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("platform std inventory is invalid")
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
            "saved platform std assertion inventory is not canonical"
        )
    return canonical


def _behavior_indexes(root: Path) -> dict[str, int]:
    matches = list(
        (root / "verification/invariants").glob(f"**/{INVARIANT}.toml")
    )
    if len(matches) != 1:
        raise ValueError(f"expected one {INVARIANT} invariant")
    record = tomllib.loads(matches[0].read_text())
    behaviors = record.get("behavior")
    if not isinstance(behaviors, list):
        raise ValueError(f"{INVARIANT} lacks behaviors")
    result = {}
    for index, behavior in enumerate(behaviors):
        partition = behavior.get("partition")
        if not isinstance(partition, Mapping):
            continue
        label = partition.get("equals")
        if isinstance(label, str) and label:
            result[label] = index
    required = {
        STDCLOCK,
        NON_PACING_TICK,
        TIME_LOGICAL,
        INJECTED_READINESS,
    }
    if required != set(result):
        raise ValueError("platform std invariant behaviors drifted")
    return result


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
                f"unexpected platform std test {discovery_id}:{path}:{name}"
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
    if tuple(row["name"] for row in tests) != TEST_ORDER:
        raise ValueError("platform std semantic lock order drifted")
    return {
        "schema_version": 1,
        "id": "PLATFORM_STD_4_SEMANTIC_LOCK",
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
            "reviewed platform std semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "platform std expressions or bindings drifted from semantic lock"
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
        "platform std assignments generated: "
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
                "platform std plan arguments must be complete"
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
            "platform std catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
