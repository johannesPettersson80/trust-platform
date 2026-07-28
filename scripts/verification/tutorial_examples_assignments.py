"""Generate reviewed assignments for the frozen tutorial-examples batch."""

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


PATH = "crates/trust-runtime/tests/tutorial_examples.rs"
INVENTORY_PATH = Path(
    "target/gate-artifacts/verification/"
    "tutorial-examples-4-assertions.json"
)
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "tutorial-examples-4-semantic-lock.json"
)

TUTORIALS = "RT_TUTORIAL_EXAMPLES_001"

COMPILE = "NINE_SINGLE_FILE_TUTORIALS_COMPILE"
BLINKER = "BLINKER_REVIEWED_TIMING_SEQUENCE"
TRAFFIC = "TRAFFIC_LIGHT_REVIEWED_PHASE_SEQUENCE"
MOTOR = "MOTOR_STARTER_REVIEWED_LATCH_SEQUENCE"

COMPILE_TEST = "tutorial_examples_parse_typecheck_and_compile_to_bytecode"
BLINKER_TEST = "tutorial_blinker_ton_timing_behavior"
TRAFFIC_TEST = "tutorial_traffic_light_state_sequence"
MOTOR_TEST = "tutorial_motor_starter_latch_and_unlatch"

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_B1B41EFAE7B35700E367": COMPILE_TEST,
    "DISC_CA2458841CE10965BD22": TRAFFIC_TEST,
    "DISC_CE8EBF020908F5AAF44F": MOTOR_TEST,
    "DISC_F46BC74C7DE6111123BE": BLINKER_TEST,
}

COMMON_EXCLUSION = (
    "This four-test batch does not establish arbitrary tutorial-source, "
    "IEC-wide, complete runtime, cross-platform, physical-I/O, LSP, editor, "
    "or documentation correctness."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    COMPILE_TEST: (
        "The compile case is bounded to the fixed nine reviewed single-file "
        "tutorial sources. It does not discover a future tutorial, cover the "
        "project-form tutorials numbered ten and above, or prove that every "
        "repository example compiles.",
        "Successful harness construction and path-aware module construction "
        "do not assert runtime output behavior, serialized byte identity or "
        "cross-version compatibility, malformed-container rejection, or "
        "absence of LSP diagnostics.",
        COMMON_EXCLUSION,
    ),
    BLINKER_TEST: (
        "The blinker case asserts only the Lamp sequence FALSE, TRUE, TRUE, "
        "FALSE at the reviewed observations. It does not observe TON Q or ET "
        "directly, prove timer accuracy between scans, or cover Enable FALSE.",
        "The trace does not cover PT changes, skipped timer calls, restart, "
        "nonmonotonic time, watchdog behavior, or arbitrary cycle cadence.",
        COMMON_EXCLUSION,
    ),
    TRAFFIC_TEST: (
        "The traffic case asserts only the Red, Yellow, and Green tuples "
        "after the initial scan and four reviewed helper advances. It does "
        "not inspect the internal Phase value or type.",
        "Each helper performs a 500 ms threshold scan and a following 1 ms "
        "scan, but the threshold-cycle output is not asserted separately. "
        "The case does not prove absence of early transitions, timer fault "
        "behavior, restart, or arbitrary timing.",
        COMMON_EXCLUSION,
    ),
    MOTOR_TEST: (
        "The motor case proves the reviewed start, seal-in, stop, restart, "
        "and overload observations only. It does not prove simultaneous "
        "Start plus Stop or Start plus Overload precedence, held-input "
        "behavior, or fault reset semantics.",
        "The second start does not assert SealInContact before the overload. "
        "The fixture is an in-memory logical example and does not command or "
        "prove a physical motor, direct address, safety relay, or hardware "
        "interlock.",
        COMMON_EXCLUSION,
    ),
}

BehaviorSelector = tuple[str, str]
Route = tuple[str, str, str]


def _reviewed_routes() -> dict[str, dict[str, Route]]:
    result: dict[str, dict[str, Route]] = {}

    def register(
        test_name: str,
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
            routes[assertion_id] = (TUTORIALS, behavior, disposition)

    register(
        COMPILE_TEST,
        COMPILE,
        oracle=(
            "ASSERT_5472BA873EE5C7311647265A",
            "ASSERT_5039D55B6590AE8A9558C75F",
        ),
    )
    register(
        BLINKER_TEST,
        BLINKER,
        setup=("ASSERT_550377BB91DB706D1C3A97D5",),
        oracle=(
            "ASSERT_D4425418EE6A1168C683EF88",
            "ASSERT_1C7DC45E31FFF9E1E59692D2",
            "ASSERT_25AE6DC83E987AF0AC3022E1",
            "ASSERT_AF1355D23706D5C81970CAAF",
            "ASSERT_EC100C276B792BA702B37406",
        ),
    )
    register(
        TRAFFIC_TEST,
        TRAFFIC,
        setup=("ASSERT_95AB4F440ECA3A14839193D6",),
        oracle=(
            "ASSERT_CC58CEE92FB05766CAD3D7B0",
            "ASSERT_3C91CC7316CCF52E6ED0408A",
        ),
    )
    register(
        MOTOR_TEST,
        MOTOR,
        setup=("ASSERT_11F4BD4AEDDD512F31BFA127",),
        oracle=(
            "ASSERT_B746A270D668C9168A40DD9A",
            "ASSERT_062CAA752302D0392B4D36F4",
            "ASSERT_F942EE3F115AC9B9F6FDAE0F",
            "ASSERT_8AD297511E8AEEC1C4B15732",
            "ASSERT_BB20AAA53E5CF965E3AD081C",
            "ASSERT_75C534E867D92C4E5FC80904",
            "ASSERT_CDDEF9CF86AC6831724E05A9",
            "ASSERT_59CBEF2CE0A507E1AAD4CF4D",
            "ASSERT_279476A0D34EBC6C467C6CE0",
            "ASSERT_7C5817BD99455DB0DE49FB55",
            "ASSERT_B89C346C5CBC79B4973DFFF8",
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
                f"{discovery_id} tutorial-examples denominator "
                "identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate tutorial-examples denominator {discovery_id}"
            )
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live tutorial-examples denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed tutorial-examples identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} tutorial-examples inventory identity drifted"
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
                "unexpected or duplicate tutorial-examples test "
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
        raise ValueError("tutorial-examples assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 20, "setup_or_precondition": 3}
    ):
        raise ValueError(
            "tutorial-examples assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_tutorial_examples_4_semantic_lock",
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
        raise ValueError(f"unreviewed tutorial-examples test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            "unreviewed tutorial-examples assertion "
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
        raise ValueError("tutorial-examples assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("tutorial-examples document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("tutorial-examples inventory is invalid")
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
            "saved tutorial-examples assertion inventory is not canonical"
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
            "missing tutorial-examples behavior selectors: "
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
                "unexpected tutorial-examples test "
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
        "id": "TUTORIAL_EXAMPLES_4_SEMANTIC_LOCK",
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
            "reviewed tutorial-examples semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "tutorial-examples expressions or bindings drifted "
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
        "tutorial-examples assignments generated: "
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
                "tutorial-examples plan arguments must be complete"
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
            "tutorial-examples catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
