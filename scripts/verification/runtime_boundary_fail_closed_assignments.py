"""Generate reviewed assignments for the runtime boundary fail-closed batch."""

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


REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-boundary-fail-closed-12-semantic-lock.json"
)

BOUNDARY_PATH = "crates/trust-runtime/tests/boundary_resolver.rs"
HARNESS_PATH = "crates/trust-runtime/tests/harness_fail_closed.rs"
INIT_PATH = "crates/trust-runtime/tests/init_fail_closed.rs"
REFERENCE_PATH = "crates/trust-runtime/src/value/reference.rs"

HARNESS = "RT_HARNESS_BOUNDARY_IO_001"
HOST_POU = "RT_HOST_POU_EXECUTION_001"
STORAGE = "RT_VARIABLE_STORAGE_REFERENCE_001"
CORE_VALUES = "RT_CORE_VALUE_PROGRAM_MODEL_001"
VM_REFERENCE = "VM_REFERENCE_RESOLUTION_001"

STRUCTURED_PATH = "STRUCTURED_SYMBOLIC_PATH_READ"
DECLARED_NULL = "DECLARED_NULL_REFERENCE_READ"
SYMBOL_REJECTION = "UNKNOWN_OR_AMBIGUOUS_SYMBOLIC_PATH_REJECTION"
TYPO_REJECTION = "TYPO_MUTATION_OR_BIND_REJECTION"
INTERFACE_NULL = "FUNCTION_INTERFACE_NULL_DEFAULTS"
QUEUED_DEBUG = "QUEUED_DEBUG_UNKNOWN_TARGET_REJECTION"
REF_PATH = "PARTIAL_ACCESS_AND_REFERENCE_PATH"
ARRAY_OFFSET = "ARRAY_OFFSET_ROW_MAJOR_AND_REPRESENTABLE"
INVALID_REFERENCE = "NULL_OR_INVALID_REFERENCE"

AMBIGUOUS_TEST = "resolver_reports_ambiguous_unqualified_program_var"
UNKNOWN_TEST = "resolver_reports_unknown_path_without_null_fallback"
STRUCTURED_TEST = "resolver_reads_indexed_and_dotted_program_paths"
NULL_REF_TEST = "declared_null_like_values_are_not_missing_name_errors"
BIND_TYPO_TEST = (
    "bind_direct_typo_returns_boundary_error_not_silent_binding"
)
INPUT_TYPO_TEST = (
    "set_input_typo_returns_boundary_error_not_silent_global_create"
)
INTERFACE_TEST = "interface_param_defaults_to_null_reference"
DEBUG_GLOBAL_TEST = "debug_queued_global_write_unknown_target_fails"
DEBUG_LVALUE_TEST = "debug_queued_lvalue_write_failure_is_observable"
REF_PATH_TEST = "common_ref_path_helpers_preserve_segment_order"
BOUNDS_TEST = "checked_array_offset_preserves_bounds_error"
EXTREME_OFFSET_TEST = "array_offset_handles_extreme_bounds_without_overflow"

REVIEWED_IDENTITIES: Mapping[str, tuple[str, str]] = {
    "DISC_06771ADC38B4CC93B5E3": (BOUNDARY_PATH, AMBIGUOUS_TEST),
    "DISC_243DD1FFC684D3B7D8EF": (BOUNDARY_PATH, UNKNOWN_TEST),
    "DISC_39CADCE0EA6E2CCADFB3": (BOUNDARY_PATH, STRUCTURED_TEST),
    "DISC_05CC41C865D947DD859F": (HARNESS_PATH, NULL_REF_TEST),
    "DISC_2F780CF67C0C68670372": (HARNESS_PATH, BIND_TYPO_TEST),
    "DISC_EDBEFF5DC3190173D71B": (HARNESS_PATH, INPUT_TYPO_TEST),
    "DISC_585BAF2FD96F5CBFED48": (INIT_PATH, INTERFACE_TEST),
    "DISC_5CB0738F4F5DD699DBEF": (INIT_PATH, DEBUG_GLOBAL_TEST),
    "DISC_6EB69C59453ED035612E": (INIT_PATH, DEBUG_LVALUE_TEST),
    "DISC_237287639D3028A4B38A": (REFERENCE_PATH, REF_PATH_TEST),
    "DISC_A79C89EAF1D56A37DADD": (REFERENCE_PATH, BOUNDS_TEST),
    "DISC_C59152675AB1C2D5DD96": (
        REFERENCE_PATH,
        EXTREME_OFFSET_TEST,
    ),
}

COMMON_EXCLUSION = (
    "This batch does not establish arbitrary Structured Text execution, "
    "successful mutation behavior, VM execution parity, or IEC-wide runtime "
    "correctness."
)
HARNESS_EXCLUSIONS = (
    "The reviewed names do not define candidate ordering, case-insensitive "
    "lookup beyond the observed inputs, a complete symbolic-path grammar, or "
    "the I/O address grammar.",
    COMMON_EXCLUSION,
)
DEBUG_EXCLUSIONS = (
    "The queued-write cases do not establish debug authorization, force and "
    "release lifecycle, successful mutation, concurrent queues, or restart "
    "semantics.",
    COMMON_EXCLUSION,
)
REFERENCE_EXCLUSIONS = (
    "The reference-helper cases do not establish multi-dimensional array "
    "linearization, every platform-width conversion, mutation behavior, or "
    "VM execution parity.",
    COMMON_EXCLUSION,
)
TEST_EXCLUSIONS: dict[str, tuple[str, ...]] = {
    name: HARNESS_EXCLUSIONS
    for name in (
        AMBIGUOUS_TEST,
        UNKNOWN_TEST,
        STRUCTURED_TEST,
        NULL_REF_TEST,
        BIND_TYPO_TEST,
        INPUT_TYPO_TEST,
    )
}
TEST_EXCLUSIONS.update(
    {
        INTERFACE_TEST: (
            "The initial NULL slot does not establish interface dispatch, "
            "assignment compatibility, dereference failure, or class "
            "implementation completeness.",
            COMMON_EXCLUSION,
        ),
        DEBUG_GLOBAL_TEST: DEBUG_EXCLUSIONS,
        DEBUG_LVALUE_TEST: DEBUG_EXCLUSIONS,
        REF_PATH_TEST: REFERENCE_EXCLUSIONS,
        BOUNDS_TEST: REFERENCE_EXCLUSIONS,
        EXTREME_OFFSET_TEST: REFERENCE_EXCLUSIONS,
    }
)

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
        AMBIGUOUS_TEST,
        HARNESS,
        SYMBOL_REJECTION,
        setup=("ASSERT_31EC3FDD3562ABE26491630C",),
        oracle=(
            "ASSERT_F14F75EA4A79BE934784B203",
            "ASSERT_54E2637D39B37E4AD1A30802",
            "ASSERT_1A0243E23C9A2C09734EE73A",
            "ASSERT_304921E273623AB47767E3E4",
            "ASSERT_C8432C9E5F2539FC84ADC3E4",
        ),
    )
    register(
        UNKNOWN_TEST,
        HARNESS,
        SYMBOL_REJECTION,
        setup=("ASSERT_CEB87F9B2E0E53113939441A",),
        oracle=(
            "ASSERT_880CD7C3757ACDD57C52E830",
            "ASSERT_C9C388A0423F6663AAC5984A",
            "ASSERT_EB8238D09E7DA4579D352CFE",
            "ASSERT_842395BBACD4D6DBF9953F4D",
        ),
    )
    register(
        STRUCTURED_TEST,
        HARNESS,
        STRUCTURED_PATH,
        setup=("ASSERT_EF7859FD9A23A4339810BC73",),
        oracle=(
            "ASSERT_7730796F9FBFFE13DCA5A993",
            "ASSERT_08482BD3EF7A99F39A729052",
            "ASSERT_DCADD395B38021EF06443136",
        ),
    )
    register(
        NULL_REF_TEST,
        HARNESS,
        DECLARED_NULL,
        setup=("ASSERT_6FFEC6C7D5403D66AFB5062C",),
        oracle=("ASSERT_8C4E167E406003EEFAA7ABDB",),
    )
    register(
        BIND_TYPO_TEST,
        HARNESS,
        TYPO_REJECTION,
        setup=("ASSERT_10AA42029DBA3455A540C6CF",),
        oracle=(
            "ASSERT_D9F8D3319AE6F7F8D7E29B62",
            "ASSERT_BD7AB016C171EFB7D384C11C",
        ),
    )
    register(
        INPUT_TYPO_TEST,
        HARNESS,
        TYPO_REJECTION,
        setup=("ASSERT_EFABD97DC41ABA868AAE4CAC",),
        oracle=(
            "ASSERT_C32FA688964A18AD38266252",
            "ASSERT_95AA4C0BAD035732C5FB97B3",
            "ASSERT_04913A699FE983048B88A14E",
            "ASSERT_F2E1074EC070AC64D18320D1",
            "ASSERT_C71454130BE6BBA04808BD12",
            "ASSERT_F6730ED43691B154C9E1024C",
        ),
    )
    register(
        INTERFACE_TEST,
        HOST_POU,
        INTERFACE_NULL,
        setup=(
            "ASSERT_959F44AFE239BA2EA598DF64",
            "ASSERT_4848D847DAEBEC153C9B0B69",
            "ASSERT_73A502D86743EA6AE7C415D6",
        ),
        oracle=("ASSERT_50849A0093A0CE875C5AD72F",),
    )
    register(
        DEBUG_GLOBAL_TEST,
        STORAGE,
        QUEUED_DEBUG,
        oracle=(
            "ASSERT_4E78A62A3C7C6ED30799EEA5",
            "ASSERT_4E504FE5E8EB9239B5F54353",
            "ASSERT_44ECDAA2E85E5D007F905CEB",
        ),
    )
    register(
        DEBUG_LVALUE_TEST,
        STORAGE,
        QUEUED_DEBUG,
        oracle=(
            "ASSERT_FB59C01D9102C62D95D95FE4",
            "ASSERT_F8E83EF7FF54C107BCD27363",
            "ASSERT_0D9FA2B124CA48A397B9752A",
            "ASSERT_01930211E546321A1371789D",
        ),
    )
    register(
        REF_PATH_TEST,
        CORE_VALUES,
        REF_PATH,
        oracle=(
            "ASSERT_86716703C1CC29EB41330614",
            "ASSERT_DBC92AD862EF442170777655",
            "ASSERT_BB521DCCD82B176CFDF5644E",
            "ASSERT_17FB61C9234FB47C09E4C6AA",
            "ASSERT_B32CEBFA60D55A88416933E5",
        ),
    )
    register(
        BOUNDS_TEST,
        VM_REFERENCE,
        INVALID_REFERENCE,
        oracle=("ASSERT_A2CC3E7237D9E288653C3DCE",),
    )
    register(
        EXTREME_OFFSET_TEST,
        CORE_VALUES,
        ARRAY_OFFSET,
        oracle=("ASSERT_5ECD9786377E2B541C15C5B3",),
    )
    return result


REVIEWED_ROUTES: Mapping[str, Mapping[str, Route]] = _reviewed_routes()


def _denominator_row_is_live(
    row: Mapping[str, Any],
    *,
    discovery_id: str,
    expected_path: str,
    expected_name: str,
) -> bool:
    if (
        row.get("discovery_id") != discovery_id
        or row.get("path") != expected_path
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
    matched: dict[str, tuple[str, str]] = {}
    for row in denominator.get("reviews", []):
        if not isinstance(row, Mapping):
            continue
        discovery_id = str(row.get("discovery_id"))
        identity = REVIEWED_IDENTITIES.get(discovery_id)
        if identity is None:
            continue
        expected_path, expected_name = identity
        if not _denominator_row_is_live(
            row,
            discovery_id=discovery_id,
            expected_path=expected_path,
            expected_name=expected_name,
        ):
            raise ValueError(
                f"{discovery_id} runtime-boundary denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate runtime-boundary denominator {discovery_id}"
            )
        matched[discovery_id] = identity
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live runtime-boundary denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed runtime-boundary identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != REVIEWED_IDENTITIES[
            discovery_id
        ]:
            raise ValueError(
                f"{discovery_id} runtime-boundary inventory identity drifted"
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
            (path, name) != REVIEWED_IDENTITIES.get(discovery_id)
            or name not in REVIEWED_ROUTES
            or name in seen
        ):
            raise ValueError(
                "unexpected or duplicate runtime-boundary test "
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
        raise ValueError("runtime-boundary assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 36, "setup_or_precondition": 9}
    ):
        raise ValueError(
            "runtime-boundary assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_boundary_fail_closed_12_semantic_lock",
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
        raise ValueError(f"unreviewed runtime-boundary test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed runtime-boundary assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("runtime-boundary assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("runtime-boundary document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("runtime-boundary inventory is invalid")
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
            "saved runtime-boundary assertion inventory is not canonical"
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
            f"missing runtime-boundary behavior selectors: {sorted(missing)}"
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
            (path, name) != REVIEWED_IDENTITIES.get(discovery_id)
            or name not in REVIEWED_ROUTES
        ):
            raise ValueError(
                f"unexpected runtime-boundary test {discovery_id}:{path}:{name}"
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
        "id": "RUNTIME_BOUNDARY_FAIL_CLOSED_12_SEMANTIC_LOCK",
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
            "reviewed runtime-boundary semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "runtime-boundary expressions or bindings drifted from semantic lock"
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
        "runtime boundary fail-closed assignments generated: "
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
                "runtime-boundary plan arguments must be complete"
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
            "runtime boundary fail-closed catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
