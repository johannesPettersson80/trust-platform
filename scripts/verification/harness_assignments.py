"""Generate reviewed assignments for the in-process harness API batch."""

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


PATH = "crates/trust-runtime/tests/harness.rs"
API = "RT_TEST_HARNESS_API_001"
BOUNDARY = "RT_HARNESS_BOUNDARY_IO_001"
SOURCE_PASSIVE = "SOURCE_CONSTRUCTION_PASSIVE"
EXACT_ACCOUNTING = "EXACT_CYCLE_AND_TIME_ACCOUNTING"
RUN_UNTIL = "RUN_UNTIL_PRECHECK_AND_STOP"
BOUNDED = "BOUNDED_EXHAUSTION"
SYMBOLIC_IO = "DIRECT_API_SYMBOLIC_IO_ROUND_TRIP"
DIRECT_IO = "DIRECT_API_BOUND_ADDRESS_ROUND_TRIP"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/harness-6-semantic-lock.json"
)

TEST_NAMES = frozenset(
    {
        "from_source",
        "io_by_name",
        "io_by_address",
        "run_controls",
        "run_until_returns_immediately_when_condition_is_already_true",
        "run_until_max_panics_when_limit_is_exceeded",
    }
)

BehaviorRef = tuple[str, str]
ASSERTION_ROUTES: Mapping[str, BehaviorRef] = {
    "ASSERT_C046F81ADB71F501BCC5611D": (API, SOURCE_PASSIVE),
    "ASSERT_190FD66BCCAF483A7CC5C863": (API, SOURCE_PASSIVE),
    "ASSERT_AC6444DFD39BCA02D5803CCF": (API, EXACT_ACCOUNTING),
    "ASSERT_3BBE728BBF008735CC73370E": (BOUNDARY, SYMBOLIC_IO),
    "ASSERT_2A3FFC918FD08CDE13A3627B": (BOUNDARY, SYMBOLIC_IO),
    "ASSERT_B45416CB6C8776E8FA9274BC": (BOUNDARY, DIRECT_IO),
    "ASSERT_7042F64EDB42DC5D8C46C0E4": (BOUNDARY, DIRECT_IO),
    "ASSERT_03E5D7AC4FFCE024EF1C52F9": (BOUNDARY, DIRECT_IO),
    "ASSERT_AAD65CF183C57AE4D3DBAA73": (BOUNDARY, DIRECT_IO),
    "ASSERT_67CB5448980F6EB3EF69CB6E": (BOUNDARY, DIRECT_IO),
    "ASSERT_5293BC857167CC9AD3FEBF4E": (BOUNDARY, DIRECT_IO),
    "ASSERT_1EC22E2B1A30A9E55B239B22": (API, EXACT_ACCOUNTING),
    "ASSERT_52091D49DDA8F822F010D1D2": (API, EXACT_ACCOUNTING),
    "ASSERT_93AA87525AD35DF1CFB26826": (API, RUN_UNTIL),
    "ASSERT_255F4A5DB827399783B85A79": (API, EXACT_ACCOUNTING),
    "ASSERT_A595960694ABBED4C2AAF69C": (API, EXACT_ACCOUNTING),
    "ASSERT_CBCACDCD7F0C01CFD05E41AB": (API, RUN_UNTIL),
    "ASSERT_DAB943FF1E7E2BB2BBDE5F4F": (API, RUN_UNTIL),
    "ASSERT_868AD8FEB2CF635C69D3A68F": (API, RUN_UNTIL),
    "ASSERT_34C85C67F2C9A0277A269D8B": (API, BOUNDED),
    "ASSERT_329D7E2A2FADF7AF5FB4B52E": (API, BOUNDED),
}

SETUP_ASSERTION_IDS = frozenset(
    {
        "ASSERT_C046F81ADB71F501BCC5611D",
        "ASSERT_3BBE728BBF008735CC73370E",
        "ASSERT_B45416CB6C8776E8FA9274BC",
        "ASSERT_1EC22E2B1A30A9E55B239B22",
        "ASSERT_CBCACDCD7F0C01CFD05E41AB",
        "ASSERT_34C85C67F2C9A0277A269D8B",
    }
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    "from_source": (
        "This proves one source fixture, passive initialization, and one "
        "explicit cycle; it does not prove arbitrary source execution.",
    ),
    "io_by_name": (
        "This proves one symbolic BOOL input/output pair, not VAR_ACCESS, "
        "JSON transport, physical I/O, or missing-name behavior.",
    ),
    "io_by_address": (
        "This proves one bound BOOL input and output address, not hardware, "
        "driver transport, other widths, or invalid-address behavior.",
    ),
    "run_controls": (
        "This proves exact accounting for one zero-time counter fixture, not "
        "implicit time advancement or arbitrary predicates.",
    ),
    "run_until_returns_immediately_when_condition_is_already_true": (
        "This proves the already-true precheck only.",
    ),
    "run_until_max_panics_when_limit_is_exceeded": (
        "This locks the direct Rust panic policy for bound three; it does not "
        "prove the JSON protocol timeout error or arbitrary resource limits.",
    ),
}


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
    }
    if set(expected.values()) != TEST_NAMES:
        raise ValueError("live harness denominator differs from review")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks harness mapping debt")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    indexes = _behavior_indexes(root)
    rows = []
    dispositions: Counter[str] = Counter()
    seen_names: set[str] = set()
    seen_assertions: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in TEST_NAMES or name in seen_names:
            raise ValueError(f"unexpected or duplicate harness test {path}:{name}")
        seen_names.add(name)
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
            route = _assertion_route(
                test_name=name,
                assertion_id=assertion_id,
            )
            disposition = _disposition(
                test_name=name,
                assertion_id=assertion_id,
            )
            if assertion_id in seen_assertions:
                raise ValueError(f"duplicate harness assertion {assertion_id}")
            seen_assertions.add(assertion_id)
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            assignments.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": route[0],
                    "behavior_index": indexes[route],
                }
            )
            dispositions[disposition] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignments,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen_names != TEST_NAMES or seen_assertions != set(ASSERTION_ROUTES):
        raise ValueError("harness assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_harness_6_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions["setup_or_precondition"],
            "non_oracle_observation": 0,
        },
        "assignments": rows,
    }


def _assertion_route(*, test_name: str, assertion_id: str) -> BehaviorRef:
    if test_name not in TEST_NAMES or assertion_id not in ASSERTION_ROUTES:
        raise ValueError(
            f"unreviewed harness assertion {test_name}:{assertion_id}"
        )
    return ASSERTION_ROUTES[assertion_id]


def _disposition(*, test_name: str, assertion_id: str) -> str:
    _assertion_route(test_name=test_name, assertion_id=assertion_id)
    return (
        "setup_or_precondition"
        if assertion_id in SETUP_ASSERTION_IDS
        else "oracle_bound"
    )


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("harness assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("harness document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("harness inventory is invalid")
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
        raise ValueError("saved harness assertion inventory is not canonical")
    return canonical


def _behavior_indexes(root: Path) -> dict[BehaviorRef, int]:
    result: dict[BehaviorRef, int] = {}
    invariant_ids = {invariant_id for invariant_id, _ in ASSERTION_ROUTES.values()}
    for invariant_id in sorted(invariant_ids):
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant_id}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        for index, behavior in enumerate(record.get("behavior", [])):
            partition = behavior.get("partition")
            label = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if isinstance(label, str):
                result[(invariant_id, label)] = index
    if set(result) & set(ASSERTION_ROUTES.values()) != set(
        ASSERTION_ROUTES.values()
    ):
        raise ValueError("harness invariant behaviors drifted")
    return {route: result[route] for route in set(ASSERTION_ROUTES.values())}


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        assertions = raw.get("assertions")
        if path != PATH or name not in TEST_NAMES or not isinstance(
            assertions, list
        ):
            raise ValueError(f"unexpected harness test {path}:{name}")
        locked = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            route = _assertion_route(
                test_name=name,
                assertion_id=assertion_id,
            )
            locked.append(
                {
                    "assertion_id": assertion_id,
                    "kind": _string(assertion, "kind", assertion_id),
                    "function": _string(assertion, "function", assertion_id),
                    "expression": _string(
                        assertion,
                        "expression",
                        assertion_id,
                    ),
                    "disposition": _disposition(
                        test_name=name,
                        assertion_id=assertion_id,
                    ),
                    "invariant_id": route[0],
                    "behavior_label": route[1],
                }
            )
        tests.append(
            {
                "path": path,
                "name": name,
                "exclusions": list(TEST_EXCLUSIONS[name]),
                "assertions": locked,
            }
        )
    return {
        "schema_version": 1,
        "id": "HARNESS_6_SEMANTIC_LOCK",
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
            f"reviewed harness semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "harness expressions or bindings drifted from semantic lock"
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
        "harness assignments generated: "
        f"{summary['tests']} tests, {summary['direct_oracle']} direct, "
        f"{summary['setup_or_precondition']} setup"
    )
    plan_args = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_args):
        if not all(value is not None for value in plan_args):
            raise ValueError("harness plan arguments must be complete")
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
            f"harness catalog plans generated: {index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
