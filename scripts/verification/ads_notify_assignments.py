"""Generate reviewed ADS notification-sampling assertion assignments."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import generate_path_plans


INVARIANT = "RT_ADS_SERVER_NOTIFICATION_SAMPLING_001"

BEHAVIOR_BY_TEST: Mapping[str, int] = {
    "server_cycle_emits_every_due_tick": 0,
    "server_on_change_coalesces_equal_values": 0,
    "sampler_slices_values_to_registered_watch_length": 1,
    "symbol_version_notification_samples_symbol_source_version": 1,
    "system_bytes_notification_samples_without_runtime_read": 1,
    "read_error_invalidates_handle": 2,
    "filetime_conversion_matches_unix_epoch_offset": 3,
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
            "live ADS notification denominator does not equal reviewed rules"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventory lacks live ADS notification mapping debt"
        )
    for discovery_id, raw in selected.items():
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("ADS notification inventory is invalid")
    behavior_count = _invariant_behavior_count(root)
    rows = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if name in seen_names:
            raise ValueError(f"duplicate ADS notification test {name}")
        seen_names.add(name)
        behavior_index = BEHAVIOR_BY_TEST[name]
        if behavior_index >= behavior_count:
            raise ValueError(f"{name} selects missing invariant behavior")
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
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
            raise ValueError(f"{path}:{name} has no product-oracle assertion")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen_names != set(BEHAVIOR_BY_TEST):
        raise ValueError("ADS notification assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_ads_notification_sampling_semantic_rules",
        "assignments": rows,
    }


def _is_setup(*, name: str, expression: str) -> bool:
    return (
        name
        in {
            "sampler_slices_values_to_registered_watch_length",
            "symbol_version_notification_samples_symbol_source_version",
            "system_bytes_notification_samples_without_runtime_read",
            "read_error_invalidates_handle",
        }
        and expression.startswith("sampler . sample")
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
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(
        "ADS notification assignments generated: "
        f"{len(assignments['assignments'])}"
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
                "ADS notification plan arguments must be complete"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="ads-server-remaining",
        )
        print(
            "ADS notification catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
