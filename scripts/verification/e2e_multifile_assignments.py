"""Generate reviewed assignments for namespace-aware multifile runtime tests."""

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


PATH = "crates/trust-runtime/tests/e2e_multifile.rs"
INVARIANT = "RT_NAMESPACE_MULTIFILE_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/e2e-multifile-4-semantic-lock.json"
)

USING_FUNCTION = "USING_IMPORTED_FUNCTION_EXECUTES_ACROSS_SOURCES"
NAMESPACED_PROGRAM = "NAMESPACED_PROGRAM_REGISTERED_AND_EXECUTED"
SIBLING_POU_GRAPH = "NAMESPACED_SIBLING_POU_GRAPH_REGISTERED_AND_EXECUTED"
DUPLICATE_PROGRAM = "CASE_INSENSITIVE_DUPLICATE_PROGRAM_IDENTITY"

NAMESPACE_TEST = "namespace_resolution"
PROGRAM_TEST = "namespaced_programs_are_runtime_entry_points"
SIBLING_TEST = "namespaced_pous_resolve_sibling_interfaces"
DUPLICATE_TEST = "duplicate_program_name_errors"

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_8AAC146F12AF0309C40B": SIBLING_TEST,
    "DISC_8FFAEF4683A05CC09B12": DUPLICATE_TEST,
    "DISC_9DE91547140FFDC2347C": PROGRAM_TEST,
    "DISC_ABFDD7450BAC9E9C9097": NAMESPACE_TEST,
}

COMMON_EXCLUSION = (
    "This batch does not establish nested namespace behavior, visibility or "
    "access control, source-order independence, configuration or task "
    "scheduling, production launch behavior, or complete registry ordering."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    NAMESPACE_TEST: (
        "The USING case is bounded to two sources in reviewed order, one "
        "single-level direct import, one DINT helper returning x + 1, and one "
        "observed count. It does not cover ambiguity or additional POU kinds.",
        COMMON_EXCLUSION,
    ),
    PROGRAM_TEST: (
        "The program case is bounded to one public single-level namespace, "
        "one unassociated program, and one harness cycle. It does not cover "
        "multiple-program order or a production runtime launch.",
        COMMON_EXCLUSION,
    ),
    SIBLING_TEST: (
        "The sibling graph case checks reviewed registry identities, a clean "
        "cycle, and one value. It does not call through the interface "
        "reference, so it proves no interface dispatch; the class method is "
        "registered but not invoked.",
        COMMON_EXCLUSION,
    ),
    DUPLICATE_TEST: (
        "The duplicate case is bounded to two top-level PROGRAM declarations "
        "in separate source strings whose spelling differs only by case. It "
        "does not cover same-file, namespaced, or cross-kind collisions, "
        "source-order independence, partial-state details, stable error code "
        "or location, or diagnostic prose beyond the reviewed markers.",
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
        NAMESPACE_TEST,
        USING_FUNCTION,
        setup=("ASSERT_97DCB126F235C08D5E8C9AC8",),
        oracle=("ASSERT_CCCB05054EF4DF9653EAFCAB",),
    )
    register(
        PROGRAM_TEST,
        NAMESPACED_PROGRAM,
        setup=("ASSERT_11D228DA2C09AEACA6896ED4",),
        oracle=(
            "ASSERT_FBEFEF4128EB0FC32E1B7556",
            "ASSERT_75CD23504D69A7B574BA1F73",
        ),
    )
    register(
        SIBLING_TEST,
        SIBLING_POU_GRAPH,
        setup=("ASSERT_54C67B88D58A19832A4BA2F2",),
        oracle=(
            "ASSERT_86433D814645645109CF658D",
            "ASSERT_8FA2711510C4B7DE699919A5",
            "ASSERT_B9832D0AA29DAF64E0251644",
            "ASSERT_9C04AA5300022A922F1BE79C",
            "ASSERT_B294E5D310A375F069E49616",
            "ASSERT_B5E71A766F3CD07BF7A0506F",
        ),
    )
    register(
        DUPLICATE_TEST,
        DUPLICATE_PROGRAM,
        setup=(),
        oracle=(
            "ASSERT_319868A68361139583B7093E",
            "ASSERT_38BD1486625EC4840708EE76",
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
                f"{discovery_id} e2e multifile denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate e2e multifile denominator {discovery_id}"
            )
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live e2e multifile denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed e2e multifile identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} e2e multifile inventory identity drifted"
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
    behavior_indexes: Mapping[str, int],
) -> dict[str, Any]:
    rows = []
    seen: set[str] = set()
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[str] = Counter()
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
                "unexpected or duplicate e2e multifile test "
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
        raise ValueError("e2e multifile assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 11, "setup_or_precondition": 3}
    ):
        raise ValueError(
            "e2e multifile assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_e2e_multifile_4_semantic_lock",
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
        raise ValueError(f"unreviewed e2e multifile test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed e2e multifile assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("e2e multifile assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("e2e multifile document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("e2e multifile inventory is invalid")
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
            "saved e2e multifile assertion inventory is not canonical"
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
        label = partition.get("equals", partition.get("malformed"))
        if isinstance(label, str) and label:
            result[label] = index
    required = {
        USING_FUNCTION,
        NAMESPACED_PROGRAM,
        SIBLING_POU_GRAPH,
        DUPLICATE_PROGRAM,
    }
    if required != set(result):
        raise ValueError("e2e multifile invariant behaviors drifted")
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
                f"unexpected e2e multifile test {discovery_id}:{path}:{name}"
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
        "id": "E2E_MULTIFILE_4_SEMANTIC_LOCK",
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
            "reviewed e2e multifile semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "e2e multifile expressions or bindings drifted from semantic lock"
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
        "e2e multifile assignments generated: "
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
                "e2e multifile plan arguments must be complete"
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
            "e2e multifile catalog plans generated: "
            f"{index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
