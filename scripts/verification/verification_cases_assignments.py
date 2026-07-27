"""Partition verification-case helper tests into mappings and oracle debt."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import generate_path_plans


INVARIANT = "VERIFICATION_CASE_ARTIFACT_HELPER_001"
UNRESOLVED_REASON = "assertion_oracle_unresolved"

BEHAVIOR_BY_TEST: Mapping[str, int] = {
    "workspace_absolute_case_path_is_recorded_as_committed_relative_identity": 0,
    "default_artifact_dir_is_workspace_target_gate_artifacts": 1,
    "stamped_artifact_dir_overrides_compile_time_workspace_path": 1,
    "blocked_cases_are_recorded_without_executing_the_runner": 2,
    "runnable_cases_capture_snapshots_and_state_delta": 3,
    "expected_case_file_digest_is_enforced_before_execution": 4,
    "schema_version_mismatch_is_rejected_before_execution": 4,
    "trust_verify_env_stamps_are_recorded_in_artifact": 5,
    "partial_trust_verify_env_stamps_fail_before_execution": 6,
    "mismatched_trust_verify_test_id_fails_before_execution": 7,
    "mismatched_trust_verify_case_file_digest_fails_before_execution": 7,
    "mismatched_trust_verify_artifact_dir_fails_before_execution": 7,
}


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("assertion inventory document lacks inventories")
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
        and row.get("name") in BEHAVIOR_BY_TEST
    }
    if {name for _path, name in expected.values()} != set(BEHAVIOR_BY_TEST):
        raise ValueError(
            "live verification-cases denominator does not equal reviewed rules"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventory lacks live verification-cases mapping debt"
        )
    for discovery_id, raw in selected.items():
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_partition(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("verification-cases inventory is invalid")
    behavior_count = _invariant_behavior_count(root)
    assignments = []
    classifications = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if name in seen_names:
            raise ValueError(f"duplicate verification-cases test {name}")
        seen_names.add(name)
        behavior_index = BEHAVIOR_BY_TEST[name]
        if behavior_index >= behavior_count:
            raise ValueError(f"{name} selects missing invariant behavior")
        unresolved = raw.get("unresolved_assertion_helpers")
        if not isinstance(unresolved, Sequence) or isinstance(
            unresolved,
            (str, bytes),
        ):
            raise ValueError(f"{name} unresolved helper list is invalid")
        if unresolved:
            classifications.append(
                {
                    "path": path,
                    "name": name,
                    "reason_code": UNRESOLVED_REASON,
                }
            )
            continue
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{name} lacks assertions")
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": INVARIANT,
                    "behavior_index": behavior_index,
                }
            )
            if _is_setup(name=name, expression=expression):
                setup_ids.append(assertion_id)
        if len(setup_ids) == len(assertion_rows):
            raise ValueError(f"{name} has no product-oracle assertion")
        assignments.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen_names != set(BEHAVIOR_BY_TEST):
        raise ValueError("verification-cases partition is not exhaustive")
    return (
        {
            "schema_version": 3,
            "basis": "reviewed_verification_case_artifact_rules",
            "assignments": assignments,
        },
        {
            "schema_version": 1,
            "basis": "reviewed_generated_input_oracle_limitations",
            "classifications": classifications,
        },
    )


def _is_setup(*, name: str, expression: str) -> bool:
    return (
        name == "stamped_artifact_dir_overrides_compile_time_workspace_path"
        and not expression.startswith("assert_eq !")
    )


def _invariant_behavior_count(root: Path) -> int:
    matches = list(
        (root / "verification/invariants").glob(f"**/{INVARIANT}.toml")
    )
    if len(matches) != 1:
        raise ValueError(f"expected one {INVARIANT} invariant")
    record = tomllib.loads(matches[0].read_text())
    behaviors = record.get("behavior")
    if not isinstance(behaviors, list):
        raise ValueError(f"{INVARIANT} lacks behaviors")
    return len(behaviors)


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
    parser.add_argument("--classifications-out", type=Path, required=True)
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
    assignments, classifications = generate_partition(
        root=root,
        inventory_document=selected,
    )
    mapped_ids = {
        (row["path"], row["name"]) for row in assignments["assignments"]
    }
    mapping_inventory = {
        "schema_version": selected["schema_version"],
        "inventories": {
            discovery_id: row
            for discovery_id, row in selected["inventories"].items()
            if (row["path"], row["test_name"]) in mapped_ids
        },
    }
    _write_json(_rooted(root, args.selected_inventory_out), mapping_inventory)
    _write_json(_rooted(root, args.assignments_out), assignments)
    _write_json(_rooted(root, args.classifications_out), classifications)
    print(
        "verification-cases partition generated: "
        f"{len(assignments['assignments'])} mapped, "
        f"{len(classifications['classifications'])} explicit nonmapping"
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
                "verification-cases plan arguments must be complete"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=mapping_inventory,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="verification-cases-remaining",
        )
        print(
            "verification-cases catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
