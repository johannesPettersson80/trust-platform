"""Generate path plans for the frozen remaining trust-hir family."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .frozen_catalog_identity import (
    digest_bytes,
    digest_document,
    selection_digest,
)
from .frozen_hir_family_batch import (
    CATALOG_PATH,
    DENOMINATOR_PATH,
    HIR_REMAINING_PROFILE_KEY,
    SHIM_PATH,
    derive_hir_family_selection,
    hir_family_selection_digest,
    path_key,
)


def generate_path_plans(
    *,
    root: Path,
    metadata: Mapping[str, Any],
    inventory_document: Mapping[str, Any],
    assignments_document: Mapping[str, Any],
    output_dir: Path,
    reviewer_ids: Sequence[str],
    reviewed_date: str,
    timestamp: str,
) -> dict[str, Any]:
    """Generate counts, digests, inventories, and plans without hand metadata."""

    root = root.resolve()
    output_dir = output_dir.resolve()
    try:
        output_dir.relative_to(root)
    except ValueError as error:
        raise ValueError("HIR plan output directory escapes repository") from error
    reviewers = _distinct_strings(reviewer_ids, "reviewer_ids")
    if not reviewed_date or not timestamp:
        raise ValueError("reviewed_date and timestamp must be non-empty")

    selection = derive_hir_family_selection(root)
    _validate_metadata_selection(root, metadata, selection)
    denominator = _load_toml(root / DENOMINATOR_PATH)
    reviews = [
        row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("path") in selection.paths
    ]
    expected_keys = {
        (
            _required_string(row, "path", "denominator review"),
            _required_string(row, "name", "denominator review"),
        ): _required_string(row, "discovery_id", "denominator review")
        for row in reviews
    }
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("HIR assertion document lacks inventories")
    inventory_by_key: dict[tuple[str, str], tuple[str, Any]] = {}
    for discovery_id, inventory in inventories.items():
        if not isinstance(inventory, Mapping):
            raise ValueError(f"{discovery_id} assertion inventory is invalid")
        key = (
            _required_string(inventory, "path", str(discovery_id)),
            _required_string(inventory, "test_name", str(discovery_id)),
        )
        if key in inventory_by_key:
            raise ValueError(f"duplicate HIR inventory identity {key}")
        inventory_by_key[key] = (str(discovery_id), inventory)
    if set(inventory_by_key) != set(expected_keys):
        raise ValueError("HIR inventories do not equal reviewed denominator tests")
    if any(
        inventory_by_key[key][0] != expected_keys[key]
        for key in expected_keys
    ):
        raise ValueError("HIR inventory discovery identities drifted")
    assignments = _validate_assignments(
        root=root,
        document=assignments_document,
        expected_keys=set(expected_keys),
        inventory_by_key=inventory_by_key,
    )

    metadata_by_path = {
        _required_string(record, "path", "execution path record"): record
        for record in _mapping_array(metadata.get("paths"), "metadata paths")
    }
    if set(metadata_by_path) != set(selection.paths):
        raise ValueError("HIR execution paths do not equal frozen selection paths")

    reviews_by_path: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for row in reviews:
        reviews_by_path[str(row["path"])].append(row)
    output_dir.mkdir(parents=True, exist_ok=True)
    path_records = []
    for path in sorted(reviews_by_path):
        path_reviews = sorted(
            reviews_by_path[path],
            key=lambda row: str(row["discovery_id"]),
        )
        path_assignments = [
            assignments[(path, str(row["name"]))] for row in path_reviews
        ]
        invariant_ids = sorted(
            {
                str(behavior["invariant_id"])
                for row in path_assignments
                for behavior in row["assertion_assignments"]
            }
        )
        live_invariants = _load_invariants(root)
        metadata_record = metadata_by_path[path]
        projected_raw = Path(
            _required_string(
                metadata_record,
                "projected_raw_results_path",
                path,
            )
        )
        results_path = projected_raw.with_name(
            projected_raw.stem + "-results.json"
        )
        key = path_key(path)
        plan_path = output_dir / f"{key}-plan.json"
        inventory_path = output_dir / f"{key}-assertions.json"
        plan = {
            "schema_version": 1,
            "execution_kind": "rust_nextest_jsonl_projection",
            "target_profile": HIR_REMAINING_PROFILE_KEY,
            "path": path,
            "package": "trust-hir",
            "target_kind": _required_string(
                metadata_record, "target_kind", path
            ),
            "binary_id": _required_string(
                metadata_record, "binary_id", path
            ),
            "test_prefix": metadata_record.get("test_prefix"),
            "command": _required_string(metadata, "command", "metadata"),
            "runner_path": SHIM_PATH.as_posix(),
            "raw_results_path": projected_raw.as_posix(),
            "reviewed_date": reviewed_date,
            "timestamp": timestamp,
            "reviewer_ids": reviewers,
            "expected_count": len(path_reviews),
            "selection_digest": selection_digest(path_reviews),
            "invariant_ids": invariant_ids,
            "invariant_behavior_digests": {
                invariant_id: digest_document(
                    {"behavior": live_invariants[invariant_id].get("behavior")}
                )
                for invariant_id in invariant_ids
            },
            "assertion_assignments": sorted(
                (
                    _assertion_plan_row(
                        assignment=row,
                        inventory=inventory_by_key[
                            (path, str(row["name"]))
                        ][1],
                    )
                    for row in path_assignments
                ),
                key=lambda row: row["name"],
            ),
        }
        if not isinstance(plan["test_prefix"], str):
            raise ValueError(f"{path} metadata test_prefix is invalid")
        projected_inventory = {
            "schema_version": inventory_document.get("schema_version", 2),
            "inventories": {
                discovery_id: inventory
                for discovery_id, inventory in sorted(
                    (
                        inventory_by_key[
                            (path, str(row["name"]))
                        ]
                        for row in path_reviews
                    ),
                    key=lambda item: item[0],
                )
            },
        }
        _write_json_atomic(plan_path, plan)
        _write_json_atomic(inventory_path, projected_inventory)
        path_records.append(
            {
                "path": path,
                "expected_count": len(path_reviews),
                "plan_path": _repo_relative(root, plan_path),
                "inventory_path": _repo_relative(root, inventory_path),
                "results_path": results_path.as_posix(),
            }
        )
    index = {
        "schema_version": 1,
        "source_commit": metadata.get("source_commit"),
        "expected_count": len(reviews),
        "paths": path_records,
    }
    _write_json_atomic(output_dir / "index.json", index)
    return index


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--execution-metadata",
        type=Path,
        required=True,
    )
    parser.add_argument(
        "--assertion-inventory",
        type=Path,
        required=True,
    )
    parser.add_argument("--assignments", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--reviewer-id",
        action="append",
        dest="reviewer_ids",
        default=None,
    )
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    now = datetime.now(timezone.utc)
    try:
        metadata = _load_json(root, args.execution_metadata)
        inventory = _load_json(root, args.assertion_inventory)
        assignments = _load_json(root, args.assignments)
        output_dir = _workspace_path(root, args.output_dir)
        index = generate_path_plans(
            root=root,
            metadata=metadata,
            inventory_document=inventory,
            assignments_document=assignments,
            output_dir=output_dir,
            reviewer_ids=tuple(
                args.reviewer_ids
                or ("codex-main", "codex-specification-review")
            ),
            reviewed_date=args.reviewed_date or now.date().isoformat(),
            timestamp=args.timestamp
            or now.isoformat(timespec="seconds").replace("+00:00", "Z"),
        )
        print(
            "frozen HIR catalog plans generated "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"frozen HIR catalog plan generation failed: {error}")
        return 2


def _validate_metadata_selection(
    root: Path,
    metadata: Mapping[str, Any],
    selection: Any,
) -> None:
    expected = {
        "reviewed_nonmapping_count": selection.reviewed_nonmapping_count,
        "catalog_mapped_count": selection.catalog_mapped_count,
        "selected_test_count": len(selection.tests),
    }
    for field, value in expected.items():
        if metadata.get(field) != value:
            raise ValueError(f"HIR metadata {field} drifted")
    expected_digests = {
        "selection_sha256": hir_family_selection_digest(selection),
        "denominator_sha256": digest_bytes(
            (root / DENOMINATOR_PATH).read_bytes()
        ),
        "catalog_sha256": digest_bytes((root / CATALOG_PATH).read_bytes()),
    }
    for field, value in expected_digests.items():
        if metadata.get(field) != value:
            raise ValueError(f"HIR metadata {field} drifted")


def _validate_assignments(
    *,
    root: Path,
    document: Mapping[str, Any],
    expected_keys: set[tuple[str, str]],
    inventory_by_key: Mapping[
        tuple[str, str],
        tuple[str, Mapping[str, Any]],
    ],
) -> dict[tuple[str, str], dict[str, Any]]:
    if document.get("schema_version") != 2:
        raise ValueError("HIR assignments schema_version must equal 2")
    rows = _mapping_array(document.get("assignments"), "assignments")
    assignments: dict[tuple[str, str], dict[str, Any]] = {}
    invariants = _load_invariants(root)
    for index, row in enumerate(rows):
        if set(row) != {
            "path",
            "name",
            "assertion_assignments",
        }:
            raise ValueError(f"HIR assignment {index} fields drifted")
        path = _required_string(row, "path", f"assignment {index}")
        name = _required_string(row, "name", f"assignment {index}")
        key = (path, name)
        if key in assignments:
            raise ValueError(f"duplicate HIR semantic assignment {path}#{name}")
        raw_behaviors = row.get("assertion_assignments")
        behavior_rows = _mapping_array(
            raw_behaviors,
            f"assignment {index} assertion_assignments",
        )
        inventory_entry = inventory_by_key.get(key)
        if inventory_entry is None:
            raise ValueError(f"HIR assignment selects absent test {path}#{name}")
        inventory_assertions = _mapping_array(
            inventory_entry[1].get("assertions"),
            f"{path}#{name} assertions",
        )
        inventory_ids = [
            _required_string(
                assertion,
                "assertion_id",
                f"{path}#{name} assertion",
            )
            for assertion in inventory_assertions
        ]
        if len(inventory_ids) != len(set(inventory_ids)):
            raise ValueError(f"HIR inventory {path}#{name} duplicates assertions")
        assigned_ids: set[str] = set()
        normalized_behaviors = []
        for behavior_offset, behavior in enumerate(behavior_rows):
            label = f"assignment {index} assertion {behavior_offset}"
            if set(behavior) != {
                "assertion_id",
                "invariant_id",
                "behavior_index",
            }:
                raise ValueError(f"HIR {label} fields drifted")
            assertion_id = _required_string(
                behavior,
                "assertion_id",
                label,
            )
            if assertion_id in assigned_ids:
                raise ValueError(f"HIR {label} duplicates {assertion_id}")
            assigned_ids.add(assertion_id)
            invariant_id = _required_string(behavior, "invariant_id", label)
            behavior_index = behavior.get("behavior_index")
            if (
                not isinstance(behavior_index, int)
                or isinstance(behavior_index, bool)
                or behavior_index < 0
            ):
                raise ValueError(f"HIR {label} behavior_index is invalid")
            invariant = invariants.get(invariant_id)
            if invariant is None:
                raise ValueError(
                    f"HIR assignment selects missing {invariant_id}"
                )
            invariant_behaviors = invariant.get("behavior")
            if (
                not isinstance(invariant_behaviors, Sequence)
                or isinstance(invariant_behaviors, (str, bytes))
                or behavior_index >= len(invariant_behaviors)
            ):
                raise ValueError(
                    f"HIR assignment selects absent "
                    f"{invariant_id}#behavior:{behavior_index}"
                )
            normalized_behaviors.append(dict(behavior))
        if assigned_ids != set(inventory_ids):
            raise ValueError(
                f"HIR assignment {path}#{name} assertion IDs do not equal "
                "the reviewed inventory"
            )
        assignments[key] = {
            "path": path,
            "name": name,
            "assertion_assignments": sorted(
                normalized_behaviors,
                key=lambda behavior: str(behavior["assertion_id"]),
            ),
        }
    if set(assignments) != expected_keys:
        missing = sorted(expected_keys - set(assignments))
        extra = sorted(set(assignments) - expected_keys)
        raise ValueError(
            "HIR assignments do not equal reviewed denominator tests: "
            f"missing={missing}, extra={extra}"
        )
    return assignments


def _assertion_plan_row(
    *,
    assignment: Mapping[str, Any],
    inventory: Mapping[str, Any],
) -> dict[str, Any]:
    name = _required_string(assignment, "name", "assignment")
    assertions = _mapping_array(inventory.get("assertions"), f"{name} assertions")
    behaviors = _mapping_array(
        assignment.get("assertion_assignments"),
        f"{name} assertion_assignments",
    )
    inventory_ids = {
        _required_string(assertion, "assertion_id", f"{name} assertion")
        for assertion in assertions
    }
    behavior_by_id = {
        _required_string(behavior, "assertion_id", f"{name} behavior"): behavior
        for behavior in behaviors
    }
    if set(behavior_by_id) != inventory_ids or len(behavior_by_id) != len(behaviors):
        raise ValueError(f"{name} assertion assignment IDs drifted")
    return {
        "name": name,
        "assertions": [
            {
                "assertion_id": assertion_id,
                "invariant_id": _required_string(
                    behavior_by_id[assertion_id],
                    "invariant_id",
                    f"{name} behavior",
                ),
                "behavior_index": behavior_by_id[assertion_id][
                    "behavior_index"
                ],
            }
            for assertion_id in sorted(inventory_ids)
        ],
    }


def _load_invariants(root: Path) -> dict[str, dict[str, Any]]:
    invariants = {}
    for path in (root / "verification/invariants").rglob("*.toml"):
        document = _load_toml(path)
        invariant_id = document.get("id")
        if isinstance(invariant_id, str) and invariant_id:
            if invariant_id in invariants:
                raise ValueError(f"duplicate invariant {invariant_id}")
            invariants[invariant_id] = document
    return invariants


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _load_json(root: Path, value: Path) -> dict[str, Any]:
    path = _workspace_path(root, value)
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
    return document


def _workspace_path(root: Path, value: Path) -> Path:
    path = value if value.is_absolute() else root / value
    path = path.resolve()
    try:
        path.relative_to(root)
    except ValueError as error:
        raise ValueError(f"path escapes repository: {value}") from error
    if path.is_symlink():
        raise ValueError(f"path cannot be a symlink: {value}")
    return path


def _mapping_array(value: Any, label: str) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(row, Mapping) for row in value)
    ):
        raise ValueError(f"{label} must be an array of objects")
    return list(value)


def _distinct_strings(value: Sequence[str], label: str) -> list[str]:
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)) or not result:
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return sorted(result)


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _repo_relative(root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError as error:
        raise ValueError(f"path escapes repository: {path}") from error


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = json.dumps(document, indent=2, ensure_ascii=False) + "\n"
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as temporary:
        temporary.write(content)
        temporary_path = Path(temporary.name)
    os.replace(temporary_path, path)
