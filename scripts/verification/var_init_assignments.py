"""Generate reviewed assignments for the frozen var-init batch."""

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


PATH = "crates/trust-runtime/tests/var_init.rs"
INVENTORY_PATH = Path(
    "target/gate-artifacts/verification/var-init-4-assertions.json"
)
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/var-init-4-semantic-lock.json"
)

COMPILER = "IEC_HIR_VARIABLE_INITIALIZER_CONSTANT_001"
VM = "VM_RUNTIME_VALUE_MATERIALIZATION_001"

CONSTANT_INITIALIZER = "SCALAR_DEFAULT_LITERAL_AND_CONSTANT_INITIALIZER"
MUTABLE_REJECTED = "MUTABLE_VARIABLE_REFERENCE_INITIALIZER_REJECTED"
VALUE_MATERIALIZATION = "DECLARATION_ARRAY_INITIALIZER_MATERIALIZATION"

REPETITION_TEST = (
    "declaration_repetition_array_initializer_expands_group"
)
ARRAY_TEST = "declaration_array_initializer_end_to_end"
IEC_TABLE14_TEST = "iec_table14"
PARTIAL_TEST = (
    "declaration_partial_array_initializer_default_fills_remaining_elements"
)

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_228F27E52C6E7C5770DC": REPETITION_TEST,
    "DISC_64155A401AC80B11DB4F": ARRAY_TEST,
    "DISC_76C36D65723E0778AA23": IEC_TABLE14_TEST,
    "DISC_BB30FD4AFD91E52B8068": PARTIAL_TEST,
}

COMMON_EXCLUSION = (
    "This four-test batch does not establish arbitrary type, IEC-wide, "
    "cross-platform, physical-I/O, restart, or complete runtime correctness."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    REPETITION_TEST: (
        "The repetition case proves only one one-dimensional INT array and "
        "the reviewed threefold repetition of the pair 1, 2. It does not "
        "cover multidimensional arrays, nested repetition, zero or negative "
        "counts, bounds, coercion, or an excess element.",
        COMMON_EXCLUSION,
    ),
    ARRAY_TEST: (
        "The complete array case proves only one one-dimensional three-INT "
        "initializer materialized after one clean cycle. It does not cover "
        "multidimensional arrays, alternate element types, bounds, coercion, "
        "or malformed and excess element lists.",
        COMMON_EXCLUSION,
    ),
    PARTIAL_TEST: (
        "The partial array case proves only two explicit INT elements and "
        "three zero-filled remaining elements after one clean cycle. It does "
        "not assert an IEC warning, excess element handling, nonzero element "
        "defaults, multidimensional arrays, bounds, or coercion.",
        COMMON_EXCLUSION,
    ),
    IEC_TABLE14_TEST: (
        "The IEC case proves rejection of one mutable reference and acceptance "
        "of one reviewed INT constant expression. It does not exhaust the "
        "constant expression grammar, forward references, cycles, enums, "
        "operators, aggregate values, diagnostics, or arbitrary type rules.",
        "The runtime observations prove only the scalar values 0, 4, and 5 "
        "after one clean cycle; successful construction does not independently "
        "prove every compiler or VM path.",
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
        REPETITION_TEST,
        VM,
        VALUE_MATERIALIZATION,
        setup=("ASSERT_2AC73780C0E98CC95A82EE84",),
        oracle=(
            "ASSERT_15A1231317BC599CE2C4F301",
            "ASSERT_AC353EC4F0E6FF331773C873",
        ),
    )
    register(
        ARRAY_TEST,
        VM,
        VALUE_MATERIALIZATION,
        setup=("ASSERT_F4B72BCB1943D6B081E2E966",),
        oracle=(
            "ASSERT_27CCCD854964E3815CBA1B28",
            "ASSERT_2C64EDC87A15C547E0F78DD4",
        ),
    )
    register(
        PARTIAL_TEST,
        VM,
        VALUE_MATERIALIZATION,
        setup=("ASSERT_805005BA08A494380668F538",),
        oracle=(
            "ASSERT_1E9D2E5757B7EC50C6453541",
            "ASSERT_51508DEFD69668B5E1F9B025",
        ),
    )
    register(
        IEC_TABLE14_TEST,
        COMPILER,
        MUTABLE_REJECTED,
        oracle=(
            "ASSERT_3F2139596B6B54151698D5B0",
            "ASSERT_9D4D7EEF6E3A67AEE9A608B0",
        ),
    )
    register(
        IEC_TABLE14_TEST,
        COMPILER,
        CONSTANT_INITIALIZER,
        oracle=(
            "ASSERT_7593023EB0CD59B863CB6D14",
            "ASSERT_28C10143C6A4841D4E0E63BB",
            "ASSERT_5C616E1BF7D5174B512955E8",
            "ASSERT_4B2AC64D1E5D714358BAE912",
            "ASSERT_72B6840D17E62353275AC0CA",
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
                f"{discovery_id} var-init denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(f"duplicate var-init denominator {discovery_id}")
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live var-init denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError("saved inventory lacks reviewed var-init identities")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} var-init inventory identity drifted"
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
                f"unexpected or duplicate var-init test "
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
        raise ValueError("var-init assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 13, "setup_or_precondition": 3}
    ):
        raise ValueError(
            "var-init assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_var_init_4_semantic_lock",
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
        raise ValueError(f"unreviewed var-init test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed var-init assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("var-init assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("var-init document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("var-init inventory is invalid")
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
            "saved var-init assertion inventory is not canonical"
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
        raise ValueError(f"missing var-init behavior selectors: {sorted(missing)}")
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
                f"unexpected var-init test {discovery_id}:{path}:{name}"
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
        "id": "VAR_INIT_4_SEMANTIC_LOCK",
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
            f"reviewed var-init semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "var-init expressions or bindings drifted from semantic lock"
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
    parser.add_argument("--inventory", type=Path, default=INVENTORY_PATH)
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
        "var-init assignments generated: "
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
            raise ValueError("var-init plan arguments must be complete")
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
            "var-init catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
