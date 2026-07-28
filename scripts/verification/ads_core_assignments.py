"""Generate reviewed assignments for the ADS-core codec batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


INVARIANT = "RT_ADS_CORE_VALUE_CODEC_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/ads-core-6-semantic-lock.json"
)

SCALAR = "FIXED_WIDTH_SCALAR_ROUNDTRIP"
STRING = "DECLARED_STRING_CAPACITY_AND_NUL_PADDING"
ARRAY = "ONE_DIMENSION_ARRAY_BOUNDS_ORDER_ROUNDTRIP"
MISMATCH = "LENGTH_TYPE_OR_SHAPE_MISMATCH"
EXTENT_MATCH = "SYMBOL_DESCRIPTOR_EXTENT_MATCH"
EXTENT_MISMATCH = "SYMBOL_DESCRIPTOR_EXTENT_MISMATCH"

TestKey = tuple[str, str]

TEST_BINDINGS: Mapping[TestKey, str] = {
    (
        "crates/trust-ads-core/src/mapping.rs",
        "rejects_shape_type_and_length_mismatches",
    ): MISMATCH,
    (
        "crates/trust-ads-core/src/mapping.rs",
        "scalar_array_mapping_round_trips_dimensions",
    ): ARRAY,
    (
        "crates/trust-ads-core/src/mapping.rs",
        "scalar_mapping_round_trips_every_supported_type",
    ): SCALAR,
    (
        "crates/trust-ads-core/src/mapping.rs",
        "string_mapping_uses_declared_capacity_and_terminator",
    ): STRING,
    (
        "crates/trust-ads-core/src/symbols.rs",
        "validates_array_byte_size_against_type_descriptor",
    ): EXTENT_MATCH,
    (
        "crates/trust-ads-core/src/symbols.rs",
        "validates_endpoint_byte_size_against_type_descriptor",
    ): EXTENT_MATCH,
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
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and (str(row.get("path")), str(row.get("name"))) in TEST_BINDINGS
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError(
            "live ADS-core denominator does not equal reviewed bindings"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live ADS-core mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(
                f"{discovery_id} ADS-core inventory identity drifted"
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
    behavior_indexes = _behavior_indexes(root)
    rows = []
    seen: set[TestKey] = set()
    behavior_counts: Counter[str] = Counter()
    kind_counts: Counter[str] = Counter()
    assertion_count = 0
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        key = (path, name)
        if key not in TEST_BINDINGS or key in seen:
            raise ValueError(f"unexpected or duplicate ADS-core test {path}:{name}")
        seen.add(key)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        assignment_rows = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            kind = _string(assertion, "kind", assertion_id)
            expression = _string(assertion, "expression", assertion_id)
            if kind not in {"macro:assert", "macro:assert_eq"}:
                raise ValueError(f"unreviewed ADS-core assertion kind {kind!r}")
            behavior = _behavior_for_assertion(
                name=name,
                default=TEST_BINDINGS[key],
                expression=expression,
            )
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": INVARIANT,
                    "behavior_index": behavior_indexes[behavior],
                }
            )
            assertion_count += 1
            behavior_counts[behavior] += 1
            kind_counts[kind] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignment_rows,
                "setup_assertion_ids": [],
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("ADS-core assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_ads_core_codec_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": assertion_count,
            "direct_oracle": assertion_count,
            "setup_or_precondition": 0,
            "non_oracle_observation": 0,
            "assertion_kinds": dict(sorted(kind_counts.items())),
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _behavior_for_assertion(
    *,
    name: str,
    default: str,
    expression: str,
) -> str:
    if (
        name == "validates_endpoint_byte_size_against_type_descriptor"
        and "ByteSizeMismatch" in expression
    ):
        return EXTENT_MISMATCH
    return default


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("ADS-core assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("ADS-core assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("ADS-core inventory is invalid")
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
        raise ValueError("saved ADS-core assertion inventory is not canonical")
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
        label = (
            partition.get("equals")
            if isinstance(partition, Mapping)
            else None
        )
        if not isinstance(label, str) or not label:
            raise ValueError(f"{INVARIANT} behavior {index} lacks selector")
        if label in result:
            raise ValueError(f"duplicate behavior selector {label}")
        result[label] = index
    expected = {
        SCALAR,
        STRING,
        ARRAY,
        MISMATCH,
        EXTENT_MATCH,
        EXTENT_MISMATCH,
    }
    if set(result) != expected:
        raise ValueError("ADS-core invariant behaviors drifted")
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            expression = _string(assertion, "expression", name)
            locked_assertions.append(
                {
                    "kind": _string(assertion, "kind", name),
                    "function": _string(assertion, "function", name),
                    "expression": expression,
                    "disposition": "oracle_bound",
                    "invariant_id": INVARIANT,
                    "behavior_label": _behavior_for_assertion(
                        name=name,
                        default=TEST_BINDINGS[(path, name)],
                        expression=expression,
                    ),
                }
            )
        tests.append(
            {"path": path, "name": name, "assertions": locked_assertions}
        )
    return {
        "schema_version": 1,
        "id": "ADS_CORE_6_SEMANTIC_LOCK",
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
            f"reviewed ADS-core semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "ADS-core reviewed expressions or bindings drifted from semantic lock"
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
        "ADS-core assignments generated: "
        f"{summary['tests']} tests, {summary['assertions']} direct oracles"
    )
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError("ADS-core plan arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="ads-core-remaining",
        )
        print(
            "ADS-core catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
