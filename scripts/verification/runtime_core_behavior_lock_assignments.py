"""Generate reviewed assignments for the frozen runtime-core vertical batch."""

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


PATH = "crates/trust-runtime/tests/runtime_core_behavior_lock.rs"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-core-behavior-lock-4-semantic-lock.json"
)

ENCODER = "RT_BYTECODE_ENCODER_PROJECTION_001"
METADATA = "RT_BYTECODE_METADATA_APPLY_001"
VM_VALUES = "VM_RUNTIME_VALUE_MATERIALIZATION_001"
IO_CYCLE = "RT_IO_CYCLE_BINDING_001"

ENCODED_LOAD = "ENCODED_MODULE_VALIDATES_AND_LOADS"
APPLY_BYTES = "APPLY_ENCODED_BYTES_AND_EXECUTE"
CLEAN_VM_LOAD = "CURRENT_SOURCE_TO_STBC_TO_CLEAN_VM_LOAD"
TYPED_VM_CYCLE = "TYPED_VM_ONE_CYCLE_VERTICAL_EXECUTION"
TYPED_VM_RETAIN_RESTART = "TYPED_VM_RETAIN_RESTART_TRANSITIONS"
CYCLE_SNAPSHOT = "CYCLE_READ_EXECUTE_WRITE_SNAPSHOT"
ALL_READS_BEFORE_WRITES = "COMPOSED_DRIVER_ALL_READS_BEFORE_ALL_WRITES"

MULTI_DRIVER_TEST = (
    "cycle_boundary_reads_every_driver_before_any_driver_writes_outputs"
)
VM_TEST = "vm_fixture_execution_image_status_and_values_are_stable"
STABLE_LOAD_TEST = "stable_bytecode_fixture_loads_on_runtime_core_path"
SINGLE_DRIVER_TEST = (
    "cycle_boundary_latches_inputs_once_and_commits_outputs_after_ready_programs"
)

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_08E82F9BB3CCCB23ECD6": MULTI_DRIVER_TEST,
    "DISC_116A738C606DD35D5B09": VM_TEST,
    "DISC_268F47FD499F57BFF436": STABLE_LOAD_TEST,
    "DISC_BAD54416BD0F91BC3776": SINGLE_DRIVER_TEST,
}

COMMON_EXCLUSION = (
    "This batch does not establish arbitrary-source or IEC-wide runtime "
    "correctness, concurrency behavior, driver fault recovery, or complete "
    "cross-platform determinism."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    STABLE_LOAD_TEST: (
        "The load case is bounded to one reviewed source fixture and one "
        "successful current STBC path. A nonempty payload is not proof of "
        "canonical byte identity, cross-version compatibility, or corrupt "
        "container rejection.",
        "The clean VM observations cover backend selection and healthy fresh "
        "fault state only; they do not execute a cycle or prove restart "
        "storage and restart time semantics.",
        COMMON_EXCLUSION,
    ),
    VM_TEST: (
        "The vertical execution case is bounded to two executions of one "
        "fixture with one TRUE input. It proves the reviewed output image, "
        "cycle count, healthy status, enum, array, structure, live reference, "
        "function-block output, and retained value only.",
        "The two VM runs are not VM-to-host evaluator parity and do not "
        "establish every numeric, aggregate, reference, function-block, or "
        "IEC-wide value behavior.",
        "Warm and cold restart assertions cover one RETAIN DINT value; they "
        "do not prove restart time, timers, task baselines, NON_RETAIN state, "
        "automatic fault restart, or failed-restart atomicity.",
        COMMON_EXCLUSION,
    ),
    SINGLE_DRIVER_TEST: (
        "The single-driver cycle case proves one read, two ready program "
        "assignments, one output commit, and the observed output image. It "
        "does not prove multi-driver ordering, snapshot publication, task "
        "priority, skipped tasks, or driver fault behavior.",
        COMMON_EXCLUSION,
    ),
    MULTI_DRIVER_TEST: (
        "The composed-driver case is bounded to two drivers in one configured "
        "order and one successful cycle. It proves all reviewed reads precede "
        "all reviewed writes, but not snapshot publication, reverse order, "
        "dynamic registration, partial failure, or retry behavior.",
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
        STABLE_LOAD_TEST,
        ENCODER,
        ENCODED_LOAD,
        oracle=(
            "ASSERT_1D6CCD84B63504924AE6B1AF",
            "ASSERT_435CD08CF7F8513234414653",
        ),
    )
    register(
        STABLE_LOAD_TEST,
        METADATA,
        APPLY_BYTES,
        oracle=("ASSERT_4C42A2F2BBA9D9913142C1FF",),
    )
    register(
        STABLE_LOAD_TEST,
        METADATA,
        CLEAN_VM_LOAD,
        setup=(
            "ASSERT_DD68789D2502D00AF006CDDA",
            "ASSERT_08F8699B410F756274613538",
        ),
        oracle=(
            "ASSERT_BC8E6A565C4CFACF27F51407",
            "ASSERT_9E84BE98E22695BF51802D33",
            "ASSERT_5D53F305412F8113DDDFB985",
            "ASSERT_F920795D57773D37FFFF1067",
        ),
    )

    register(
        VM_TEST,
        VM_VALUES,
        TYPED_VM_CYCLE,
        setup=(
            "ASSERT_95B6D24C247DF7B21CBB00F4",
            "ASSERT_692A4DAC66B15CFA5B7C3471",
            "ASSERT_A5027FD6C720148567A45EE7",
            "ASSERT_4567F207E83235457220B7AE",
            "ASSERT_2882A68CF781A12ADB64F7BA",
            "ASSERT_2B235A408DB3D0C1E56B0668",
            "ASSERT_94FB7454C5E79AC0291240C5",
            "ASSERT_36754ECA6FB0B4446BB15A53",
            "ASSERT_F36938F6025958EB701955B7",
        ),
        oracle=(
            "ASSERT_8A431B983FD8DF2C236A86F7",
            "ASSERT_A1260EE542CCA0BCACF5F69D",
            "ASSERT_BBCC17BDD73C6F5C850D4620",
            "ASSERT_2B9E82D0BD76765FC5CAC120",
            "ASSERT_F09FEF7FE0DF88746535ED60",
            "ASSERT_DCD99C9260BF5CA33CBF69EF",
            "ASSERT_859164A9557E11D340DE25C7",
            "ASSERT_0E12E50E631410E0269F46D4",
            "ASSERT_FD6E8DFAF77CBC1176BC3B1B",
            "ASSERT_3EF2C434B8D6A194C9DF715E",
            "ASSERT_F5DAFBB04BAD561E579AD2E3",
            "ASSERT_33E291718F2C634E7B91BB1C",
            "ASSERT_3359F1BC9BD474E22075A7B9",
            "ASSERT_ECA5DE22C180499A925A88A3",
            "ASSERT_BDA3ADAAB1BE2F13A1C4D481",
            "ASSERT_88DD1A32F9E0DE82EC2B217F",
            "ASSERT_A4DA15DE2BF592ED07892523",
            "ASSERT_EA53B01B6FA242B20DA56E6C",
            "ASSERT_F415E6531628E542073CD4D9",
            "ASSERT_7B56296769917ECF75D36276",
            "ASSERT_F65B73BBE5F0D50EAFA004A6",
        ),
    )
    register(
        VM_TEST,
        VM_VALUES,
        TYPED_VM_RETAIN_RESTART,
        oracle=(
            "ASSERT_98A87FEB323DB10D35C2BCC0",
            "ASSERT_EF205D9B11BB11AD53EE33BB",
            "ASSERT_ADC7046985D7877B1D7FF935",
            "ASSERT_48E42FA84ADB897E419968AD",
            "ASSERT_108AB550896C334082B23172",
        ),
    )

    register(
        SINGLE_DRIVER_TEST,
        IO_CYCLE,
        CYCLE_SNAPSHOT,
        setup=(
            "ASSERT_3C74887A01215622B5EDDCA6",
            "ASSERT_CB7506E382BC4EE828458734",
            "ASSERT_1C87415DC221549B6BFD36A7",
            "ASSERT_794605653FD447BA62233D10",
            "ASSERT_8AACAC124CB8E58BB05BB984",
            "ASSERT_1B38FA0033AD0A44969B50BD",
        ),
        oracle=(
            "ASSERT_C4494AD53ADCACA55A5EF10B",
            "ASSERT_49A8797899B9A75102EE6882",
            "ASSERT_81E3E48E26D2F199DB6489AB",
            "ASSERT_8B2D9A4EB075EAA737FE68DE",
            "ASSERT_1436363C17CFCFDA78E60AE7",
            "ASSERT_F2457599E46EDA2E723E2D7A",
        ),
    )
    register(
        MULTI_DRIVER_TEST,
        IO_CYCLE,
        ALL_READS_BEFORE_WRITES,
        setup=(
            "ASSERT_153E2760794BC952D5DD6E7D",
            "ASSERT_BE20C60BCF69A36CB38347B9",
            "ASSERT_29AFBB6DF33D2380510864AB",
            "ASSERT_9DA5ED81AF0F61EDF1F4DB5A",
            "ASSERT_A534E09EFB1275536046F408",
            "ASSERT_D4F5CCBF5AC68E2650A01E4F",
        ),
        oracle=(
            "ASSERT_2FCE7991FD9580AB29352F07",
            "ASSERT_CA33660DE8C9EEFC25090604",
            "ASSERT_81B794AF02668D688997E0BA",
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
                f"{discovery_id} runtime-core denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate runtime-core denominator {discovery_id}"
            )
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live runtime-core denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed runtime-core identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} runtime-core inventory identity drifted"
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
                "unexpected or duplicate runtime-core test "
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
        raise ValueError("runtime-core assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 42, "setup_or_precondition": 23}
    ):
        raise ValueError(
            "runtime-core assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_core_behavior_lock_4_semantic_lock",
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
        raise ValueError(f"unreviewed runtime-core test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed runtime-core assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("runtime-core assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("runtime-core document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("runtime-core inventory is invalid")
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
            "saved runtime-core assertion inventory is not canonical"
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
            f"missing runtime-core behavior selectors: {sorted(missing)}"
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
                f"unexpected runtime-core test {discovery_id}:{path}:{name}"
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
        "id": "RUNTIME_CORE_BEHAVIOR_LOCK_4_SEMANTIC_LOCK",
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
            "reviewed runtime-core semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "runtime-core expressions or bindings drifted from semantic lock"
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
        "runtime-core behavior-lock assignments generated: "
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
                "runtime-core plan arguments must be complete"
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
            "runtime-core behavior-lock catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
