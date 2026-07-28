"""Generate reviewed assignments for the ST test CLI integration batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


PATH = "crates/trust-runtime/tests/st_test_cli_command.rs"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/st-test-cli-7-semantic-lock.json"
)

REPORT = "DEV_TEST_REPORTING_001"
EXECUTION = "DEV_TEST_EXECUTION_001"
BUILD = "RT_BUILD_COMMAND_001"

MACHINE_REPORTS = "MACHINE_REPORT_FORMATS"
HUMAN_REPORTS = "HUMAN_AND_LIST_REPORTS"
CONFIGURATION_BINDING = "CONFIGURATION_TEST_PROGRAM_BINDING"
CASE_DEADLINE = "CASE_DEADLINE"
PREPARED_DEADLINE = "PREPARED_CASE_WITHIN_DEADLINE"
VALID_BUILD = "VALID_PROJECT_SOURCE_OVERRIDE_AND_DEPENDENCIES"

BehaviorRef = tuple[str, str]

TEST_BINDINGS: Mapping[str, BehaviorRef] = {
    "list_flag_lists_tutorial_10_tests_without_executing": (
        REPORT,
        HUMAN_REPORTS,
    ),
    "filter_zero_message_is_clear_in_human_output": (
        REPORT,
        HUMAN_REPORTS,
    ),
    "json_output_includes_duration_fields": (REPORT, MACHINE_REPORTS),
    "timeout_flag_reports_error_for_infinite_loop_test": (
        EXECUTION,
        CASE_DEADLINE,
    ),
    "timeout_budget_does_not_count_project_recompilation_per_case": (
        EXECUTION,
        PREPARED_DEADLINE,
    ),
    "test_program_runs_when_configuration_is_present": (
        EXECUTION,
        CONFIGURATION_BINDING,
    ),
    "build_accepts_recent_language_regression_cases": (
        BUILD,
        VALID_BUILD,
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
        and str(row.get("name")) in TEST_BINDINGS
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError(
            "live ST test CLI denominator does not equal reviewed bindings"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks ST test CLI mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            expected[discovery_id],
        ):
            raise ValueError(f"{discovery_id} ST test CLI identity drifted")
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
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[str] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in TEST_BINDINGS or name in seen:
            raise ValueError(
                f"unexpected or duplicate ST test CLI test {path}:{name}"
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
            function = _string(assertion, "function", assertion_id)
            kind = _string(assertion, "kind", assertion_id)
            expression = _string(assertion, "expression", assertion_id)
            disposition = _disposition(
                test_name=name,
                function=function,
                kind=kind,
                expression=expression,
            )
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            invariant_id, behavior_label = _behavior_for_assertion(
                test_name=name,
                expression=expression,
                disposition=disposition,
            )
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_indexes[
                        (invariant_id, behavior_label)
                    ],
                }
            )
            dispositions[disposition] += 1
            behavior_counts[f"{invariant_id}:{behavior_label}"] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignment_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("ST test CLI assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_st_test_cli_semantic_lock",
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


def _disposition(
    *,
    test_name: str,
    function: str,
    kind: str,
    expression: str,
) -> str:
    if function == test_name and kind in {
        "macro:assert",
        "macro:assert_eq",
    }:
        return "oracle_bound"
    if (
        test_name == "json_output_includes_duration_fields"
        and function == test_name
        and kind == "method:expect"
        and (
            "serde_json :: from_slice" in expression
            or '[\"tests\"] . as_array' in expression
        )
    ):
        return "oracle_bound"
    if kind == "method:expect":
        return "setup_or_precondition"
    raise ValueError(
        f"unreviewed ST test CLI assertion {function}:{kind}"
    )


def _behavior_for_assertion(
    *,
    test_name: str,
    expression: str,
    disposition: str,
) -> BehaviorRef:
    default = TEST_BINDINGS[test_name]
    if (
        test_name == "timeout_flag_reports_error_for_infinite_loop_test"
        and disposition == "oracle_bound"
        and "test timed out after 1 second" in expression
    ):
        return (REPORT, HUMAN_REPORTS)
    return default


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("ST test CLI assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("ST test CLI document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("ST test CLI inventory is invalid")
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
            "saved ST test CLI assertion inventory is not canonical"
        )
    return canonical


def _behavior_indexes(root: Path) -> dict[BehaviorRef, int]:
    selected = set(TEST_BINDINGS.values()) | {
        (REPORT, HUMAN_REPORTS),
    }
    result: dict[BehaviorRef, int] = {}
    for invariant_id in sorted({row[0] for row in selected}):
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
            if isinstance(label, str) and label:
                result[(invariant_id, label)] = index
    if not selected <= set(result):
        raise ValueError("ST test CLI invariant behaviors drifted")
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
            function = _string(assertion, "function", name)
            kind = _string(assertion, "kind", name)
            expression = _string(assertion, "expression", name)
            disposition = _disposition(
                test_name=name,
                function=function,
                kind=kind,
                expression=expression,
            )
            invariant_id, behavior_label = _behavior_for_assertion(
                test_name=name,
                expression=expression,
                disposition=disposition,
            )
            locked_assertions.append(
                {
                    "kind": kind,
                    "function": function,
                    "expression": expression,
                    "disposition": disposition,
                    "invariant_id": invariant_id,
                    "behavior_label": behavior_label,
                }
            )
        tests.append(
            {"path": path, "name": name, "assertions": locked_assertions}
        )
    return {
        "schema_version": 1,
        "id": "ST_TEST_CLI_7_SEMANTIC_LOCK",
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
            f"reviewed ST test CLI semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "ST test CLI expressions or bindings drifted from semantic lock"
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
        "ST test CLI assignments generated: "
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
            raise ValueError("ST test CLI plan arguments must be complete")
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
            "ST test CLI catalog plans generated: "
            f"{index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
