"""Generate reviewed assignments for the runtime restart/retain batch."""

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


PATH = "crates/trust-runtime/src/runtime/restart.rs"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/restart-5-semantic-lock.json"
)

RESTART_INVARIANT = "RT_SAFE_RESTART_001"
MIGRATION_INVARIANT = "RT_RETAIN_MIGRATION_001"
RESTART_FAILURE = (
    "FAILED_WARM_RESTART_INSTANCE_CONSTRUCTION_PRESERVES_LIVE_RUNTIME"
)
RECURSIVE_IDENTITY = "RECURSIVE_DECLARED_IDENTITY_CANONICALIZATION"
NESTED_REJECTION = "INCOMPATIBLE_NESTED_VALUE_REJECTION"

RESTART_TEST = "restart_instance_construction_failure_preserves_live_state"
ENUM_TEST = "apply_retain_snapshot_canonicalizes_legacy_enum_type_name"
NESTED_ENUM_TEST = (
    "apply_retain_snapshot_canonicalizes_nested_enum_in_struct"
)
STRUCT_DRIFT_TEST = "apply_retain_snapshot_rejects_struct_field_type_drift"
ARRAY_TEST = (
    "apply_retain_snapshot_canonicalizes_array_of_struct_"
    "and_rejects_bad_element"
)

GLOBAL_EXCLUSION = (
    "These in-process unit tests do not prove file-backed persistence, "
    "startup integration, power-loss behavior, or external interoperability."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    RESTART_TEST: (
        "The test covers one warm restart with an internally removed function-"
        "block definition, not cold restart or class/program construction "
        "failures.",
        "It does not prove task, time, fault, status, I/O, process-image, "
        "retained-global, or concurrent scheduler preservation.",
        "It observes one queued instance-field force and no stable diagnostic "
        "text; queued writes and broader debug state are not covered.",
        GLOBAL_EXCLUSION,
    ),
    ENUM_TEST: (
        "The test covers case-canonical compatible enum identity only, not "
        "renames, unknown or removed variants, numeric mismatch, or events.",
        GLOBAL_EXCLUSION,
    ),
    NESTED_ENUM_TEST: (
        "The test covers one enum nested in one structure field, not arrays, "
        "unions, deeper nesting, or migration events.",
        GLOBAL_EXCLUSION,
    ),
    STRUCT_DRIFT_TEST: (
        "The test covers one BOOL-for-INT structure field and only a generic "
        "error substring.",
        "It does not observe unchanged target state, sibling preservation, "
        "transactional atomicity, or event absence after rejection.",
        GLOBAL_EXCLUSION,
    ),
    ARRAY_TEST: (
        "The accepted case covers one one-dimensional [1..2] array of two "
        "structures and does not assert the retained x values.",
        "The rejected case covers one BOOL element and does not observe "
        "unchanged state after rejection.",
        "It does not prove multidimensional arrays, bound drift, deeper "
        "nesting, or migration events.",
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
            routes[assertion_id] = (invariant, behavior, disposition)

    register(
        RESTART_TEST,
        RESTART_INVARIANT,
        RESTART_FAILURE,
        setup=(
            "ASSERT_C05F0DB3405C99705E64046A",
            "ASSERT_874BEB8C7623E8C4B6E67D3E",
            "ASSERT_D7FCD931C4CB3E0579570668",
            "ASSERT_BD297B55AAE2699579AFBBC7",
        ),
        oracle=(
            "ASSERT_9998A8A4636267B8163A28BC",
            "ASSERT_420E7FD95209CB695EF8FFA4",
            "ASSERT_BE103E1C9A2DC7420DF41782",
            "ASSERT_F97224366D311A4371A23A75",
            "ASSERT_3DF4D1A16ABCB7B9A0993CB2",
            "ASSERT_A5DC25897B8E5CE3D3322545",
            "ASSERT_9B1EA804AE49A23FEE309336",
        ),
    )
    register(
        ENUM_TEST,
        MIGRATION_INVARIANT,
        RECURSIVE_IDENTITY,
        setup=(
            "ASSERT_05D6F94644C57C2D7FB52BF3",
            "ASSERT_D01408C1731AA3599B22D0CC",
        ),
        oracle=(
            "ASSERT_98469A8BC9BEB221762E3B55",
            "ASSERT_AE9BA1E370BEDA5310186018",
            "ASSERT_13E75EFF2841C55A374F1DE7",
            "ASSERT_02BB62096A58C019B19008E9",
            "ASSERT_7CA6470BEB0C86E5B18644D0",
            "ASSERT_AC379E0358C8C9E9EE38EF5E",
        ),
    )
    register(
        NESTED_ENUM_TEST,
        MIGRATION_INVARIANT,
        RECURSIVE_IDENTITY,
        setup=(
            "ASSERT_09B58B653CD3F71E5BDC5D76",
            "ASSERT_7861AC3E29880203EF4379A3",
        ),
        oracle=(
            "ASSERT_5B1F56E9B584AE5B8DE1D0B4",
            "ASSERT_F616FF7356223BC13D7C4180",
            "ASSERT_25188CDDC403D8DFE890EEFB",
            "ASSERT_49635D104B63A3580FD0882F",
            "ASSERT_B2DFD94C62145D9F1A95C93D",
            "ASSERT_DA89661771904C5843658C93",
        ),
    )
    register(
        STRUCT_DRIFT_TEST,
        MIGRATION_INVARIANT,
        NESTED_REJECTION,
        setup=(
            "ASSERT_F5CF25E11D9DE0A11A080C94",
            "ASSERT_2FB3FB734A0147DA68EE5462",
        ),
        oracle=(
            "ASSERT_B0FF92FC9E987863E7854A9A",
            "ASSERT_54B89288A70DB3B07F2C6343",
        ),
    )
    register(
        ARRAY_TEST,
        MIGRATION_INVARIANT,
        RECURSIVE_IDENTITY,
        setup=(
            "ASSERT_6BCD3F0E12ED5346648EEF4F",
            "ASSERT_196B0A16686CDE743D0F05AA",
            "ASSERT_6C7CB8E3FC433DE5D8A4A4BC",
        ),
        oracle=(
            "ASSERT_432801090882DC9FDA8982A6",
            "ASSERT_4A70BC0E0EB94983609C250B",
            "ASSERT_3461047E1740058932684B59",
            "ASSERT_FC5149978326CC661CD68126",
            "ASSERT_63D1F7A716E5D414870F4EB6",
            "ASSERT_BEA22C098B2053F9DF068FD4",
        ),
    )
    register(
        ARRAY_TEST,
        MIGRATION_INVARIANT,
        NESTED_REJECTION,
        setup=(
            "ASSERT_D68696272815F0D4209686D8",
            "ASSERT_8F80FE3583D8DFC578401C4B",
        ),
        oracle=(
            "ASSERT_35E1ACFA94D01511FFEB8589",
            "ASSERT_262A431C80FAEF00BD5D1EF6",
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
        raise ValueError("live restart denominator differs from review")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks restart mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            expected[discovery_id],
        ):
            raise ValueError(f"{discovery_id} restart identity drifted")
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
            raise ValueError(f"unexpected or duplicate restart test {path}:{name}")
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignments = []
        setup_ids = []
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
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(REVIEWED_ROUTES):
        raise ValueError("restart assignments are not exhaustive")
    if sum(dispositions.values()) != 44:
        raise ValueError("restart assertion count drifted from reviewed 44")
    return {
        "schema_version": 3,
        "basis": "reviewed_restart_5_semantic_lock",
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
        raise ValueError(f"unreviewed restart test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed restart assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("restart assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("restart document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("restart inventory is invalid")
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
        raise ValueError("saved restart assertion inventory is not canonical")
    return canonical


def _behavior_indexes(root: Path) -> dict[tuple[str, str], int]:
    required = {
        (RESTART_INVARIANT, RESTART_FAILURE),
        (MIGRATION_INVARIANT, RECURSIVE_IDENTITY),
        (MIGRATION_INVARIANT, NESTED_REJECTION),
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
        raise ValueError("restart invariant behaviors drifted")
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in REVIEWED_ROUTES:
            raise ValueError(f"unexpected restart test {path}:{name}")
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
        "id": "RESTART_5_SEMANTIC_LOCK",
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
            f"reviewed restart semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "restart expressions or bindings drifted from semantic lock"
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
        "restart assignments generated: "
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
            raise ValueError("restart plan arguments must be complete")
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
            f"restart catalog plans generated: {index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
