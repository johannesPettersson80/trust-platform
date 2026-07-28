"""Generate reviewed assignments for the ADS-core point-quality batch."""

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


INVARIANT = "RT_ADS_CORE_POINT_QUALITY_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/ads-core-quality-3-semantic-lock.json"
)
PATH = "crates/trust-ads-core/src/quality.rs"

COLD_START = "COLD_START_POINT_IS_STALE"
EXPLICIT_STALE = "EXPLICIT_STALE_PRESERVES_SUPPLIED_FIELDS"
STALE_TO_GOOD = "STALE_TO_GOOD_CLEARS_DETAIL"
GOOD_TO_ERROR = "GOOD_TO_ERROR_REPLACES_FIELDS"
ERROR_TO_STALE = "ERROR_TO_STALE_PRESERVES_TIMESTAMP"

TEST_BINDINGS: Mapping[str, str] = {
    "cold_start_status_is_stale": COLD_START,
    "stale_at_preserves_last_update_timestamp": EXPLICIT_STALE,
    "quality_transitions_clear_and_preserve_fields": STALE_TO_GOOD,
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
        and str(row.get("name")) in TEST_BINDINGS
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError(
            "live ADS-core quality denominator does not equal reviewed bindings"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks ADS-core quality mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            expected[discovery_id],
        ):
            raise ValueError(
                f"{discovery_id} ADS-core quality identity drifted"
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
    seen: set[str] = set()
    behavior_counts: Counter[str] = Counter()
    assertion_count = 0
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in TEST_BINDINGS or name in seen:
            raise ValueError(
                f"unexpected or duplicate ADS-core quality test {path}:{name}"
            )
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignment_rows = []
        for index, assertion in enumerate(assertions):
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            kind = _string(assertion, "kind", assertion_id)
            if kind != "macro:assert_eq":
                raise ValueError(
                    f"unreviewed ADS-core quality assertion kind {kind!r}"
                )
            behavior = _behavior_for_assertion(name=name, index=index)
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": INVARIANT,
                    "behavior_index": behavior_indexes[behavior],
                }
            )
            assertion_count += 1
            behavior_counts[behavior] += 1
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
        raise ValueError("ADS-core quality assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_ads_core_quality_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": assertion_count,
            "direct_oracle": assertion_count,
            "setup_or_precondition": 0,
            "non_oracle_observation": 0,
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _behavior_for_assertion(*, name: str, index: int) -> str:
    if name != "quality_transitions_clear_and_preserve_fields":
        return TEST_BINDINGS[name]
    if index <= 2:
        return STALE_TO_GOOD
    if index <= 5:
        return GOOD_TO_ERROR
    if index <= 8:
        return ERROR_TO_STALE
    raise ValueError(f"quality transition assertion index {index} drifted")


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("ADS-core quality assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("ADS-core quality document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("ADS-core quality inventory is invalid")
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
            "saved ADS-core quality assertion inventory is not canonical"
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
        label = (
            partition.get("equals")
            if isinstance(partition, Mapping)
            else None
        )
        if not isinstance(label, str) or not label:
            raise ValueError(f"{INVARIANT} behavior {index} lacks selector")
        result[label] = index
    if set(result) != {
        COLD_START,
        EXPLICIT_STALE,
        STALE_TO_GOOD,
        GOOD_TO_ERROR,
        ERROR_TO_STALE,
    }:
        raise ValueError("ADS-core quality invariant behaviors drifted")
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
        for index, assertion in enumerate(assertions):
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            locked_assertions.append(
                {
                    "kind": _string(assertion, "kind", name),
                    "function": _string(assertion, "function", name),
                    "expression": _string(assertion, "expression", name),
                    "disposition": "oracle_bound",
                    "invariant_id": INVARIANT,
                    "behavior_label": _behavior_for_assertion(
                        name=name,
                        index=index,
                    ),
                }
            )
        tests.append(
            {"path": path, "name": name, "assertions": locked_assertions}
        )
    return {
        "schema_version": 1,
        "id": "ADS_CORE_QUALITY_3_SEMANTIC_LOCK",
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
            f"reviewed ADS-core quality semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "ADS-core quality expressions or bindings drifted from semantic lock"
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
        "ADS-core quality assignments generated: "
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
            raise ValueError("ADS-core quality plan arguments must be complete")
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
            "ADS-core quality catalog plans generated: "
            f"{index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
