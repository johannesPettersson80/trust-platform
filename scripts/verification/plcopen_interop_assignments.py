"""Generate reviewed PLCopen interop assertion assignments in one batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.runtime_bytecode_assignments import (
    generate_path_plans,
)


EXPORT_PROFILE = "crates/trust-plcopen/src/plcopen/tests/export_profile.rs"
ROUNDTRIP = "crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs"
SHIMS = "crates/trust-plcopen/src/plcopen/tests/shims.rs"
COMMAND = "crates/trust-runtime/tests/plcopen_command.rs"
EXPECTED_PATH_COUNTS: Mapping[str, int] = {
    EXPORT_PROFILE: 8,
    ROUNDTRIP: 5,
    SHIMS: 2,
    COMMAND: 7,
}

TEST_BINDINGS: Mapping[str, tuple[str, int]] = {
    "profile_declares_strict_subset_contract": (
        "PLCO_ST_COMPLETE_PROFILE_001",
        0,
    ),
    "plcopen_profile_json_emits_contract": (
        "PLCO_ST_COMPLETE_PROFILE_001",
        1,
    ),
    "import_supports_data_type_subset_and_generates_type_source": (
        "PLCO_ST_COMPLETE_INTERCHANGE_001",
        0,
    ),
    "round_trip_export_import_export_preserves_pou_subset": (
        "PLCO_ST_COMPLETE_INTERCHANGE_001",
        1,
    ),
    "import_accepts_st_body_with_body_level_adddata_metadata": (
        "PLCO_IMPORT_001",
        1,
    ),
    "import_rejects_non_st_bodies_with_named_diagnostics": (
        "PLCO_NON_ST_REJECTION_SET_001",
        0,
    ),
    "import_rejects_malformed_xml": ("PLCO_IMPORT_FAILURE_001", 0),
    "plcopen_import_fails_for_missing_input": (
        "PLCO_IMPORT_FAILURE_001",
        1,
    ),
    "import_reports_unsupported_nodes_and_preserves_vendor_extensions": (
        "PLCO_MIGRATION_REPORT_AND_EXTENSION_001",
        0,
    ),
    "export_reinjects_vendor_extension_hook_file": (
        "PLCO_MIGRATION_REPORT_AND_EXTENSION_001",
        1,
    ),
    "plcopen_export_import_json_reports_include_compatibility_diagnostics": (
        "PLCO_MIGRATION_REPORT_AND_EXTENSION_001",
        2,
    ),
    "export_emits_codesys_global_vars_and_project_structure_metadata": (
        "PLCO_CODESYS_METADATA_001",
        0,
    ),
    "export_emits_codesys_pou_add_data_metadata": (
        "PLCO_CODESYS_METADATA_001",
        1,
    ),
    "export_emits_codesys_method_metadata_on_standard_pous_and_roundtrips": (
        "PLCO_CODESYS_METADATA_001",
        2,
    ),
    "import_applies_siemens_library_shims_and_reports_them": (
        "PLCO_VENDOR_LIBRARY_SHIMS_001",
        0,
    ),
    "library_shim_rewrites_type_and_call_sites_only": (
        "PLCO_VENDOR_LIBRARY_SHIMS_001",
        1,
    ),
    "plcopen_import_json_reports_applied_vendor_library_shims": (
        "PLCO_VENDOR_LIBRARY_SHIMS_001",
        2,
    ),
    "plcopen_import_json_detects_openplc_ecosystem_and_shims": (
        "PLCO_VENDOR_LIBRARY_SHIMS_001",
        3,
    ),
    "plcopen_openplc_fixture_in_st_complete_bundle_import_export_smoke": (
        "PLCO_VENDOR_LIBRARY_SHIMS_001",
        4,
    ),
    "export_with_vendor_target_emits_adapter_report_and_metadata": (
        "PLCO_EXPORT_ADAPTERS_001",
        0,
    ),
    "export_siemens_target_emits_scl_bundle_and_program_ob_mapping": (
        "PLCO_EXPORT_ADAPTERS_001",
        1,
    ),
    "plcopen_export_target_generates_adapter_report_and_default_target_path": (
        "PLCO_EXPORT_ADAPTERS_001",
        2,
    ),
}


def combine_inventory_documents(
    documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    inventories: dict[str, Any] = {}
    for document in documents:
        rows = document.get("inventories")
        if not isinstance(rows, Mapping):
            raise ValueError("PLCopen assertion document lacks inventories")
        for discovery_id, row in rows.items():
            if discovery_id in inventories:
                raise ValueError(f"duplicate PLCopen inventory {discovery_id}")
            inventories[str(discovery_id)] = row
    return {"schema_version": 2, "inventories": inventories}


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("PLCopen assertion document lacks inventories")
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") in EXPECTED_PATH_COUNTS
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
    }
    expected_keys = {
        (path, name)
        for path in EXPECTED_PATH_COUNTS
        for name in TEST_BINDINGS
        if _expected_path_for_name(name) == path
    }
    if set(expected.values()) != expected_keys:
        raise ValueError("live PLCopen denominator does not equal reviewed rules")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live PLCopen mapping debt")
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
        raise ValueError("PLCopen assertion document lacks inventories")
    behavior_counts = _invariant_behavior_counts(root)
    rows = []
    seen: set[tuple[str, str]] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if (path, name) in seen or _expected_path_for_name(name) != path:
            raise ValueError(f"unexpected or duplicate PLCopen test {path}:{name}")
        seen.add((path, name))
        invariant_id, behavior_index = TEST_BINDINGS[name]
        if behavior_index >= behavior_counts.get(invariant_id, -1):
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
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_index,
                }
            )
            if _is_setup(expression):
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
    if seen != {
        (_expected_path_for_name(name), name) for name in TEST_BINDINGS
    }:
        raise ValueError("PLCopen assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": "reviewed_plcopen_interop_semantic_rules",
        "assignments": rows,
    }


def _expected_path_for_name(name: str) -> str:
    if name not in TEST_BINDINGS:
        raise ValueError(f"no reviewed PLCopen rule for {name}")
    if name in {
        "plcopen_profile_json_emits_contract",
        "plcopen_export_import_json_reports_include_compatibility_diagnostics",
        "plcopen_export_target_generates_adapter_report_and_default_target_path",
        "plcopen_import_fails_for_missing_input",
        "plcopen_import_json_detects_openplc_ecosystem_and_shims",
        "plcopen_openplc_fixture_in_st_complete_bundle_import_export_smoke",
        "plcopen_import_json_reports_applied_vendor_library_shims",
    }:
        return COMMAND
    if name in {
        "round_trip_export_import_export_preserves_pou_subset",
        "import_reports_unsupported_nodes_and_preserves_vendor_extensions",
        "import_accepts_st_body_with_body_level_adddata_metadata",
        "import_rejects_non_st_bodies_with_named_diagnostics",
        "import_supports_data_type_subset_and_generates_type_source",
    }:
        return ROUNDTRIP
    if name in {
        "import_applies_siemens_library_shims_and_reports_them",
        "library_shim_rewrites_type_and_call_sites_only",
    }:
        return SHIMS
    return EXPORT_PROFILE


def _is_setup(expression: str) -> bool:
    if expression.startswith("assert ! (fixture . is_file ()"):
        return True
    return not expression.startswith(("assert", "panic")) and (
        ". expect_err" not in expression
    )


def _invariant_behavior_counts(root: Path) -> dict[str, int]:
    result = {}
    for invariant_id, _ in set(TEST_BINDINGS.values()):
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
        result[invariant_id] = len(behaviors)
    return result


def _subset(
    inventory: Mapping[str, Any],
    assignments: Mapping[str, Any],
    paths: set[str],
) -> tuple[dict[str, Any], dict[str, Any]]:
    return (
        {
            "schema_version": 2,
            "inventories": {
                discovery_id: row
                for discovery_id, row in inventory["inventories"].items()
                if row["path"] in paths
            },
        },
        {
            "schema_version": 3,
            "basis": assignments["basis"],
            "assignments": [
                row for row in assignments["assignments"] if row["path"] in paths
            ],
        },
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
    parser.add_argument("--inventory", type=Path, action="append", required=True)
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--plcopen-execution-dir", type=Path)
    parser.add_argument("--runtime-execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    combined = combine_inventory_documents(
        [
            json.loads(_rooted(root, path).read_text())
            for path in args.inventory
        ]
    )
    selected = select_live_inventories(
        root=root,
        inventory_document=combined,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(f"PLCopen assignments generated: {len(assignments['assignments'])}")
    plan_arguments = (
        args.plcopen_execution_dir,
        args.runtime_execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError("PLCopen plan arguments must be complete")
        plans_out = _rooted(root, args.plans_out)
        plcopen_inventory, plcopen_assignments = _subset(
            selected,
            assignments,
            {EXPORT_PROFILE, ROUNDTRIP, SHIMS},
        )
        runtime_inventory, runtime_assignments = _subset(
            selected,
            assignments,
            {COMMAND},
        )
        plcopen_index = generate_path_plans(
            root=root,
            inventory_document=plcopen_inventory,
            assignments_document=plcopen_assignments,
            execution_dir=_rooted(root, args.plcopen_execution_dir),
            output_dir=plans_out / "plcopen-lib",
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="plcopen-remaining",
        )
        runtime_index = generate_path_plans(
            root=root,
            inventory_document=runtime_inventory,
            assignments_document=runtime_assignments,
            execution_dir=_rooted(root, args.runtime_execution_dir),
            output_dir=plans_out / "runtime-cli",
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="runtime-remaining",
        )
        print(
            "PLCopen catalog plans generated: "
            f"{plcopen_index['expected_count'] + runtime_index['expected_count']} "
            "tests across 4 paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
