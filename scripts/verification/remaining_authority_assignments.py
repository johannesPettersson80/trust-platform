"""Partition the complete frozen Phase 18 missing-authority denominator.

The batch intentionally uses one immutable identity and assertion digest while
partitioning the 142 tests across nine product-authority groups. Tests with a
closed assertion oracle are mapped; tests whose extractor closure still names
an external oracle are explicitly reclassified as owned oracle debt.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import (
    generate_path_plans as _generate_path_plans,
)


ROOT = Path(__file__).resolve().parents[2]
FROZEN_REPORT_PATH = ROOT / (
    "target/gate-artifacts/verification/"
    "frozen-catalog-gaps-after-runtime-boundary-12.json"
)
DEFAULT_ASSERTIONS_PATH = ROOT / (
    "target/gate-artifacts/verification/"
    "p18-remaining-authority-142/all-assertions.json"
)
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "remaining-authority-142-semantic-lock.json"
)

REVIEWED_IDENTITY_SHA256 = (
    "5c1422cb6e177013052c88e0b8e9b040"
    "33227c808c25534c262b4b0bfc00541e"
)
REVIEWED_ASSERTION_INVENTORY_SHA256 = (
    "227c299f1126308e3f329485e55e178b"
    "a46c4357588d2a4ecc110365f1f3b42c"
)

VERIFICATION = "VERIFICATION_CASE_MODEL_CONFORMANCE_001"
ADS = "RT_ADS_FOUNDATION_CONFORMANCE_001"
BROWSER = "BROWSER_ANALYSIS_FOUNDATION_CONFORMANCE_001"
DEV = "DEV_WORKBENCH_CONFORMANCE_001"
AGENT = "RT_AGENT_COMMAND_CONFORMANCE_001"
LIFECYCLE = "RT_HOST_LIFECYCLE_CONFORMANCE_001"
EXECUTION = "RT_EXECUTION_LANGUAGE_CONFORMANCE_001"
SECURITY_WEB = "RT_HOST_SECURITY_WEB_CONFORMANCE_001"
IEC = "IEC_PROJECT_EXAMPLE_CONFORMANCE_001"

BEHAVIOR_BY_INVARIANT: Mapping[str, str] = {
    VERIFICATION: "REVIEWED_CASE_MODEL_AND_TRACE_BOUNDARY",
    ADS: "REVIEWED_ADS_FOUNDATION_AND_ROUTER_TRACE",
    BROWSER: "REVIEWED_BROWSER_ANALYSIS_FOUNDATION",
    DEV: "REVIEWED_WORKBENCH_AND_LIBRARY_GATE",
    AGENT: "REVIEWED_AGENT_COMMAND_PROTOCOL",
    LIFECYCLE: "REVIEWED_HOST_LIFECYCLE_TRACE",
    EXECUTION: "REVIEWED_LANGUAGE_EXECUTION_TRACE",
    SECURITY_WEB: "REVIEWED_HOST_SECURITY_AND_WEB_TRACE",
    IEC: "REVIEWED_IEC_PROJECT_AND_RUNTIME_CASE",
}

IEC_RUNTIME_FILES = frozenset(
    {
        "compile_time_constants.rs",
        "iec_examples.rs",
        "phase11_seam_contract.rs",
        "stdlib_split_locals.rs",
        "stmt_assign_attempt.rs",
        "stmt_full.rs",
        "stmt_jmp.rs",
        "types_bit_access.rs",
        "types_pointer.rs",
        "types_ref.rs",
        "types_struct_at.rs",
        "types_user.rs",
        "value_defaults.rs",
        "value_types.rs",
        "var_constants.rs",
        "var_stat.rs",
        "var_vla.rs",
        "vars_at.rs",
    }
)

LIFECYCLE_RUNTIME_FILES = frozenset(
    {
        "build.rs",
        "build_unbound_program.rs",
        "complete_program.rs",
        "config_schema_command.rs",
        "determinism.rs",
        "e2e_configuration.rs",
        "example_runtime_configs.rs",
        "real_world.rs",
        "resources.rs",
        "runtime_events.rs",
        "runtime_restart.rs",
        "scheduler_harness.rs",
        "scheduler_resource.rs",
        "scheduler_state.rs",
        "signal_smoke.rs",
        "tasks_fb.rs",
        "ui_no_input_smoke.rs",
    }
)

SECURITY_WEB_RUNTIME_FILES = frozenset(
    {
        "debug_stepping.rs",
        "prometheus_integration.rs",
        "protocol_envelope.rs",
        "registry_command.rs",
        "web_dispatch_hol_probe.rs",
        "web_tls_integration.rs",
    }
)

DEV_RUNTIME_FILES = frozenset(
    {
        "commit_command.rs",
        "oscat_oop_examples.rs",
        "plcopen_migration.rs",
        "plcopen_motion_oop_library.rs",
    }
)


def _json_digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def _load_reviewed_identities() -> dict[str, tuple[str, str]]:
    report = json.loads(FROZEN_REPORT_PATH.read_text())
    rows = sorted(
        (
            test["discovery_id"],
            batch["path"],
            test["name"],
        )
        for batch in report["specification_batches"]
        for test in batch["tests"]
    )
    if (
        len(rows) != 142
        or len({path for _identity, path, _name in rows}) != 92
        or _json_digest(rows) != REVIEWED_IDENTITY_SHA256
    ):
        raise ValueError("remaining-authority frozen identity set drifted")
    return {
        discovery_id: (path, name)
        for discovery_id, path, name in rows
    }


def _load_bound_assertions() -> dict[str, dict[str, Any]]:
    encoded = DEFAULT_ASSERTIONS_PATH.read_bytes()
    if hashlib.sha256(encoded).hexdigest() != (
        REVIEWED_ASSERTION_INVENTORY_SHA256
    ):
        raise ValueError("remaining-authority assertion inventory drifted")
    document = json.loads(encoded)
    inventories = document.get("inventories")
    if (
        document.get("schema_version") != 2
        or not isinstance(inventories, Mapping)
        or set(inventories) != set(REVIEWED_IDENTITIES)
    ):
        raise ValueError("remaining-authority assertion identities drifted")
    return {
        str(discovery_id): dict(inventory)
        for discovery_id, inventory in inventories.items()
        if isinstance(inventory, Mapping)
    }


def _authority_for(path: str, name: str) -> tuple[str, str]:
    if path.startswith("crates/verification-cases/"):
        invariant = VERIFICATION
    elif path.startswith(("crates/trust-ads-core/", "crates/trust-ads-server/")):
        invariant = ADS
    elif path.startswith("crates/trust-wasm-analysis/"):
        invariant = BROWSER
    elif path.startswith("crates/trust-dev/"):
        invariant = DEV
    elif path.startswith(("crates/trust-hir/", "crates/trust-syntax/")):
        invariant = IEC
    elif path.startswith("crates/trust-runtime-core/"):
        invariant = LIFECYCLE
    elif "/tests/agent_command/" in path:
        invariant = AGENT
    elif path.startswith("crates/trust-runtime/src/host/") and any(
        component in path
        for component in (
            "/debug/",
            "/mesh/",
            "/registry/",
            "/security/",
        )
    ):
        invariant = SECURITY_WEB
    elif path.startswith("crates/trust-runtime/src/runtime_cloud/"):
        invariant = SECURITY_WEB
    elif path.startswith("crates/trust-runtime/src/scheduler/"):
        invariant = LIFECYCLE
    elif path.startswith("crates/trust-runtime/tests/"):
        filename = Path(path).name
        if filename in DEV_RUNTIME_FILES:
            invariant = DEV
        elif filename in SECURITY_WEB_RUNTIME_FILES:
            invariant = SECURITY_WEB
        elif filename in IEC_RUNTIME_FILES:
            invariant = IEC
        elif filename in LIFECYCLE_RUNTIME_FILES:
            invariant = LIFECYCLE
        else:
            invariant = EXECUTION
    else:
        raise ValueError(f"unclassified remaining-authority test {path}:{name}")
    return invariant, BEHAVIOR_BY_INVARIANT[invariant]


REVIEWED_IDENTITIES: Mapping[str, tuple[str, str]] = (
    _load_reviewed_identities()
)
_BOUND_ASSERTIONS = _load_bound_assertions()
REVIEWED_ROUTES: Mapping[
    str,
    Mapping[str, tuple[str, str, str]],
] = {
    discovery_id: {
        str(assertion["assertion_id"]): (
            *_authority_for(
                str(inventory["path"]),
                str(inventory["test_name"]),
            ),
            "oracle_bound",
        )
        for assertion in inventory["assertions"]
    }
    for discovery_id, inventory in _BOUND_ASSERTIONS.items()
    if not inventory["unresolved_assertion_helpers"]
}


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("remaining-authority assertion document drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("remaining-authority document lacks inventories")
    requested = []
    batch = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("remaining-authority inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append(
            {"discovery_id": discovery_id, "path": path, "name": name}
        )
        batch.append(
            {
                key: value
                for key, value in raw.items()
                if key != "inventory_digest"
            }
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch},
        requested,
    )
    if canonical != saved:
        raise ValueError(
            "saved remaining-authority assertion inventory is not canonical"
        )
    return canonical


def _denominator_row_is_live(
    row: Mapping[str, Any],
    *,
    discovery_id: str,
    path: str,
    name: str,
) -> bool:
    if row.get("path") != path or row.get("name") != name:
        return False
    if (
        row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
    ):
        return True
    if (
        discovery_id not in REVIEWED_ROUTES
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == "assertion_oracle_unresolved"
    ):
        return True
    return (
        row.get("disposition") == "catalog_mapped"
        and row.get("catalog_test_id")
        == f"TEST_AUTO_{discovery_id.removeprefix('DISC_')}"
    )


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
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
        path, name = identity
        if not _denominator_row_is_live(
            row,
            discovery_id=discovery_id,
            path=path,
            name=name,
        ):
            raise ValueError(
                f"{discovery_id} remaining-authority denominator drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate remaining-authority denominator {discovery_id}"
            )
        matched[discovery_id] = identity
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live denominator differs from remaining-authority review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
    }
    return {"schema_version": 2, "inventories": selected}


def _behavior_indexes(root: Path) -> dict[tuple[str, str], int]:
    result: dict[tuple[str, str], int] = {}
    for invariant_id, label in BEHAVIOR_BY_INVARIANT.items():
        matches = list(
            (root / "verification/invariants").glob(
                f"**/{invariant_id}.toml"
            )
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        document = tomllib.loads(matches[0].read_text())
        for index, behavior in enumerate(document.get("behavior", [])):
            partition = behavior.get("partition")
            actual = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if actual == label:
                result[(invariant_id, label)] = index
    expected = {
        (invariant_id, label)
        for invariant_id, label in BEHAVIOR_BY_INVARIANT.items()
    }
    if set(result) != expected:
        raise ValueError("remaining-authority behavior selectors drifted")
    return result


def _assignment_document(
    *,
    root: Path,
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    indexes = _behavior_indexes(root)
    rows = []
    counts: Counter[tuple[str, str]] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (path, name) != REVIEWED_IDENTITIES.get(discovery_id):
            raise ValueError(
                f"unexpected remaining-authority test {discovery_id}"
            )
        helpers = raw.get("unresolved_assertion_helpers")
        if not isinstance(helpers, list):
            raise ValueError(
                f"{path}:{name} unresolved helper list is invalid"
            )
        if helpers:
            if discovery_id in REVIEWED_ROUTES:
                raise ValueError(
                    f"{path}:{name} unresolved oracle has mapping routes"
                )
            continue
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(
                f"{path}:{name} lacks a product-oracle assertion"
            )
        routes = REVIEWED_ROUTES[discovery_id]
        assignment_rows = []
        for assertion in assertions:
            assertion_id = _string(assertion, "assertion_id", name)
            try:
                invariant, behavior, disposition = routes[assertion_id]
            except KeyError as error:
                raise ValueError(
                    f"unreviewed assertion {discovery_id}:{assertion_id}"
                ) from error
            if disposition != "oracle_bound":
                raise ValueError(
                    f"{discovery_id}:{assertion_id} is not oracle-bound"
                )
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant,
                    "behavior_index": indexes[(invariant, behavior)],
                }
            )
            counts[(invariant, behavior)] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignment_rows,
                "setup_assertion_ids": [],
                "non_oracle_assertion_ids": [],
            }
        )
    if len(rows) != 139 or sum(counts.values()) != 1079:
        raise ValueError("remaining-authority assignment totals drifted")
    return {
        "schema_version": 3,
        "basis": "reviewed_remaining_authority_142_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(counts.values()),
            "direct_oracle": sum(counts.values()),
            "owned_nonmapping": len(inventories) - len(rows),
            "setup_or_precondition": 0,
            "non_oracle_observation": 0,
            "behavior_assertions": {
                f"{invariant}:{behavior}": count
                for (invariant, behavior), count in sorted(counts.items())
            },
        },
        "assignments": rows,
    }


def generate_assignments(
    *,
    root: Path,
    assertions_path: Path,
) -> dict[str, Any]:
    path = assertions_path if assertions_path.is_absolute() else root / assertions_path
    document = json.loads(path.read_text())
    inventories = _canonical_inventories(root, document)
    return _assignment_document(root=root, inventories=inventories)


def _classification_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    rows = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        helpers = raw.get("unresolved_assertion_helpers")
        if not isinstance(helpers, list):
            raise ValueError(
                f"{path}:{name} unresolved helper list is invalid"
            )
        if helpers:
            rows.append(
                {
                    "path": path,
                    "name": name,
                    "reason_code": "assertion_oracle_unresolved",
                }
            )
    if len(rows) != 3:
        raise ValueError(
            "remaining-authority unresolved-oracle partition drifted"
        )
    return {
        "schema_version": 1,
        "classifications": rows,
    }


def _mapping_inventory(
    selected: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = selected.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("remaining-authority selection lacks inventories")
    return {
        "schema_version": 2,
        "inventories": {
            discovery_id: raw
            for discovery_id, raw in inventories.items()
            if discovery_id in REVIEWED_ROUTES
        },
    }


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        helpers = raw.get("unresolved_assertion_helpers")
        if not isinstance(helpers, list):
            raise ValueError(
                f"{path}:{name} unresolved helper list is invalid"
            )
        unresolved = bool(helpers)
        invariant, behavior = _authority_for(path, name)
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        authority = (
            {
                "status": "withheld",
                "reason_code": "assertion_oracle_unresolved",
                "unresolved_assertion_helpers": helpers,
            }
            if unresolved
            else {
                "status": "mapped",
                "invariant_id": invariant,
                "behavior_label": behavior,
            }
        )
        assertion_rows = []
        for assertion in assertions:
            row = {
                "assertion_id": _string(
                    assertion,
                    "assertion_id",
                    name,
                ),
                "kind": _string(assertion, "kind", name),
                "function": _string(assertion, "function", name),
                "expression": _string(
                    assertion,
                    "expression",
                    name,
                ),
                "disposition": (
                    "mapping_withheld_external_oracle"
                    if unresolved
                    else "oracle_bound"
                ),
            }
            if not unresolved:
                row["invariant_id"] = invariant
                row["behavior_label"] = behavior
            assertion_rows.append(row)
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
                "authority": authority,
                "exclusions": [
                    "The review proves only the exact frozen test identity "
                    "and extracted assertions; it does not imply unselected "
                    "partitions, hardware interoperability, mutation "
                    "sensitivity, or the final broad gate."
                ],
                "assertions": assertion_rows,
            }
        )
    return {
        "schema_version": 1,
        "id": "REMAINING_AUTHORITY_142_SEMANTIC_LOCK",
        "identity_digest": REVIEWED_IDENTITY_SHA256,
        "assertion_inventory_digest": (
            REVIEWED_ASSERTION_INVENTORY_SHA256
        ),
        "reviewed_test_count": len(tests),
        "reviewed_assertion_count": sum(
            len(test["assertions"]) for test in tests
        ),
        "mapped_test_count": sum(
            test["authority"]["status"] == "mapped" for test in tests
        ),
        "owned_nonmapping_test_count": sum(
            test["authority"]["status"] == "withheld" for test in tests
        ),
        "tests": tests,
    }


def write_review_lock(
    *,
    assertions_path: Path,
    output_path: Path,
) -> None:
    document = json.loads(assertions_path.read_text())
    inventories = _canonical_inventories(ROOT, document)
    _write_json(output_path, _review_lock_document(inventories))


def _target_profile(path: str) -> str:
    prefixes = (
        ("crates/trust-runtime/", "runtime-remaining"),
        ("crates/trust-runtime-core/", "runtime-core-remaining"),
        ("crates/trust-dev/", "dev-remaining"),
        ("crates/trust-hir/", "hir-remaining"),
        ("crates/trust-syntax/", "syntax-remaining"),
        ("crates/trust-ads-core/", "ads-core-remaining"),
        ("crates/trust-ads-server/", "ads-server-remaining"),
        ("crates/verification-cases/", "verification-cases-remaining"),
        ("crates/trust-wasm-analysis/", "wasm-analysis-remaining"),
    )
    for prefix, profile in prefixes:
        if path.startswith(prefix):
            return profile
    raise ValueError(f"no remaining-authority target profile for {path}")


def generate_path_plans(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    assignments_document: Mapping[str, Any],
    execution_dir: Path,
    execution_overrides_dir: Path | None,
    output_dir: Path,
    reviewed_date: str,
    timestamp: str,
) -> dict[str, Any]:
    inventories = inventory_document.get("inventories")
    assignments = assignments_document.get("assignments")
    if not isinstance(inventories, Mapping) or not isinstance(assignments, list):
        raise ValueError("remaining-authority plan inputs are invalid")
    assignments_by_key = {
        (str(row["path"]), str(row["name"])): row
        for row in assignments
    }
    grouped: dict[str, dict[str, Any]] = defaultdict(dict)
    for discovery_id, inventory in inventories.items():
        if not isinstance(inventory, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        profile = _target_profile(str(inventory["path"]))
        grouped[profile][str(discovery_id)] = inventory
    records = []
    for profile, group in sorted(grouped.items()):
        keys = {
            (str(row["path"]), str(row["test_name"]))
            for row in group.values()
        }
        subset_assignments = [
            assignments_by_key[key] for key in sorted(keys)
        ]
        profile_output = output_dir / profile
        source_execution = execution_dir / profile
        expected_paths = {path for path, _name in keys}
        selected_sources: dict[str, tuple[Path, Path]] = {}
        execution_roots = [source_execution]
        if execution_overrides_dir is not None:
            execution_roots.append(execution_overrides_dir / profile)
        for execution_root in execution_roots:
            for metadata_path in sorted(
                (execution_root / "meta").glob("*.json")
            ):
                metadata = json.loads(metadata_path.read_text())
                if not isinstance(metadata, Mapping):
                    raise ValueError(
                        f"{metadata_path} is not an object"
                    )
                path = _string(
                    metadata,
                    "path",
                    metadata_path.as_posix(),
                )
                if path not in expected_paths:
                    continue
                results_path = (
                    execution_root / "results" / metadata_path.name
                )
                if not results_path.is_file():
                    raise ValueError(
                        f"missing selected execution result for {path}"
                    )
                selected_sources[path] = (metadata_path, results_path)
        if set(selected_sources) != expected_paths:
            raise ValueError(
                f"{profile} selected execution paths drifted"
            )
        source_results_by_path: dict[str, Path] = {}
        (root / "target/gate-artifacts").mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(
            prefix="remaining-authority-execution-",
            dir=root / "target/gate-artifacts",
        ) as temporary:
            selected_execution = Path(temporary)
            (selected_execution / "meta").mkdir()
            (selected_execution / "results").mkdir()
            for path, (
                metadata_path,
                results_path,
            ) in sorted(selected_sources.items()):
                selected_name = hashlib.sha256(path.encode()).hexdigest()[:16]
                destination_name = f"{selected_name}.json"
                shutil.copy2(
                    metadata_path,
                    selected_execution / "meta" / destination_name,
                )
                shutil.copy2(
                    results_path,
                    selected_execution / "results" / destination_name,
                )
                source_results_by_path[path] = results_path
            index = _generate_path_plans(
                root=root,
                inventory_document={
                    "schema_version": 2,
                    "inventories": group,
                },
                assignments_document={
                    "schema_version": 3,
                    "assignments": subset_assignments,
                },
                execution_dir=selected_execution,
                output_dir=profile_output,
                reviewed_date=reviewed_date,
                timestamp=timestamp,
                target_profile=profile,
            )
        for record in index["paths"]:
            path = str(record["path"])
            record["results_path"] = source_results_by_path[
                path
            ].relative_to(root).as_posix()
        _write_json(profile_output / "index.json", index)
        records.extend(index["paths"])
    aggregate = {
        "schema_version": 1,
        "source_commit": subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip(),
        "expected_count": len(assignments),
        "paths": sorted(records, key=lambda row: str(row["path"])),
    }
    _write_json(output_dir / "index.json", aggregate)
    return aggregate


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(document, sort_keys=True, indent=2) + "\n"
    )


def _rooted(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument(
        "--inventory",
        type=Path,
        default=DEFAULT_ASSERTIONS_PATH,
    )
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--classifications-out", type=Path, required=True)
    parser.add_argument("--review-lock", type=Path, default=REVIEW_LOCK_PATH)
    parser.add_argument("--emit-review-lock", action="store_true")
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--execution-overrides-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()

    root = args.root.resolve()
    inventory_path = _rooted(root, args.inventory)
    document = json.loads(inventory_path.read_text())
    selected = select_live_inventories(
        root=root,
        inventory_document=document,
    )
    inventories = _canonical_inventories(root, selected)
    if args.emit_review_lock:
        _write_json(
            _rooted(root, args.review_lock),
            _review_lock_document(inventories),
        )
    mapping_inventory = _mapping_inventory(selected)
    assignments = _assignment_document(
        root=root,
        inventories=inventories,
    )
    classifications = _classification_document(inventories)
    _write_json(
        _rooted(root, args.selected_inventory_out),
        mapping_inventory,
    )
    _write_json(_rooted(root, args.assignments_out), assignments)
    _write_json(
        _rooted(root, args.classifications_out),
        classifications,
    )
    print(
        "remaining-authority partition generated: "
        f"{assignments['summary']['tests']} tests, "
        f"{assignments['summary']['direct_oracle']} direct assertions, "
        f"{len(classifications['classifications'])} owned nonmapping"
    )

    required_plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if (
        any(value is not None for value in required_plan_arguments)
        or args.execution_overrides_dir is not None
    ):
        if not all(
            value is not None for value in required_plan_arguments
        ):
            raise ValueError(
                "remaining-authority plan arguments must be complete"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=mapping_inventory,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            execution_overrides_dir=(
                _rooted(root, args.execution_overrides_dir)
                if args.execution_overrides_dir is not None
                else None
            ),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=str(args.reviewed_date),
            timestamp=str(args.timestamp),
        )
        print(
            "remaining-authority plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
