"""Closed Phase 5 suite and direct inventory-binding contracts."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable, Mapping

from ..area_routing import MILESTONE_SUITE_IDS
from ..gate_inventory import (
    ARTIFACT_KINDS,
    ARTIFACT_RETENTIONS,
    DURATION_CLASSES,
    ENVIRONMENTS,
    HARDWARE_OPT_IN,
)
from .constants import STATUSES


Fail = Callable[[Path, str], None]

DURABLE_ARTIFACT_KINDS = {
    "ci_artifact",
    "ci_job_result",
    "release_object",
    "lab_report",
    "committed_file",
}
HARDWARE_ENVIRONMENTS = {"github_self_hosted_linux", "github_or_lab_runner"}

SUITE_FIELDS = {
    "schema_version",
    "id",
    "title",
    "area",
    "owner",
    "status",
    "last_reviewed",
    "purpose",
    "duration_class",
    "environment",
    "commands",
    "command_bindings",
    "inventory_ids",
    "evidence_destination",
    "includes",
    "excludes",
    "approved_proof_producers",
}


def validate_suite_records(
    *,
    fail: Fail,
    suites: Mapping[str, Mapping[str, Any]],
    inventory: Mapping[str, Mapping[str, Any]],
) -> None:
    """Validate direct suite membership without defining include composition."""

    for suite_id, record in suites.items():
        path = _path(record)
        _validate_suite_shape(fail, path, suite_id, record)
        includes = record.get("includes", [])
        if isinstance(includes, list):
            for included_id in includes:
                if included_id not in suites:
                    fail(path, f"{suite_id} includes unknown suite {included_id}")

        commands = record.get("commands")
        bindings = record.get("command_bindings")
        inventory_ids = record.get("inventory_ids")
        if suite_id in MILESTONE_SUITE_IDS:
            if record.get("placeholder") is True:
                fail(path, f"{suite_id} milestone suite cannot be a placeholder")
            if record.get("status") != "mapped":
                fail(path, f"{suite_id} milestone suite must use status = mapped")
            if not isinstance(commands, list) or not commands:
                fail(path, f"{suite_id} milestone suite must configure commands")
        if suite_id == "veryquick" and record.get("environment") != "trust_builder":
            fail(path, "veryquick must use the reviewed trust_builder environment")
        if suite_id == "release" and _uses_target(str(record.get("evidence_destination", ""))):
            fail(path, "release evidence_destination cannot use target/")

        if not all(isinstance(value, list) for value in (commands, bindings, inventory_ids)):
            continue
        _validate_inventory_projection(
            fail=fail,
            path=path,
            suite_id=suite_id,
            inventory_ids=inventory_ids,
            inventory=inventory,
            bindings=bindings,
        )
        projected_commands: list[str] = []
        for index, inventory_id in enumerate(bindings):
            if not isinstance(inventory_id, str):
                continue
            inventory_record = inventory.get(inventory_id)
            if inventory_record is None:
                fail(path, f"{suite_id} command_bindings[{index}] references unknown gate inventory id {inventory_id}")
                continue
            projected_commands.append(str(inventory_record.get("command", "")))
            _validate_command_binding(
                fail=fail,
                path=path,
                suite_id=suite_id,
                label=f"{suite_id} command_bindings[{index}]",
                inventory_record=inventory_record,
            )
        if projected_commands != commands:
            fail(path, f"{suite_id} commands must equal the ordered command_bindings projection")


def _validate_suite_shape(
    fail: Fail,
    path: Path,
    suite_id: str,
    record: Mapping[str, Any],
) -> None:
    actual_fields = set(record) - {"_path"}
    missing = sorted(SUITE_FIELDS - actual_fields)
    extra = sorted(actual_fields - SUITE_FIELDS)
    if missing:
        fail(path, f"{suite_id} missing fields: {', '.join(missing)}")
    if extra:
        fail(path, f"{suite_id} unexpected fields: {', '.join(extra)}")
    if record.get("schema_version") != 2:
        fail(path, f"{suite_id} must use suite schema_version = 2")
    if record.get("id") != suite_id:
        fail(path, f"suite registry key {suite_id} does not match record id {record.get('id')}")
    if record.get("area") != "suite":
        fail(path, f"suite {suite_id} must use area = suite")
    if record.get("status") not in STATUSES:
        fail(path, f"{suite_id} uses unknown status {record.get('status')!r}")
    for field in ("title", "owner", "last_reviewed", "purpose", "evidence_destination"):
        if not isinstance(record.get(field), str) or not record[field].strip():
            fail(path, f"{suite_id} {field} must be a non-empty string")
    if record.get("duration_class") not in DURATION_CLASSES:
        fail(path, f"{suite_id} has unknown duration_class {record.get('duration_class')!r}")
    if record.get("environment") not in ENVIRONMENTS:
        fail(path, f"{suite_id} has unknown environment {record.get('environment')!r}")
    list_fields = (
        "commands",
        "command_bindings",
        "inventory_ids",
        "includes",
        "excludes",
        "approved_proof_producers",
    )
    for field in list_fields:
        value = record.get(field)
        if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
            fail(path, f"{suite_id} {field} must contain only non-empty strings")
            continue
        if len(value) != len(set(value)):
            fail(path, f"{suite_id} {field} must not contain duplicates")
    inventory_ids = record.get("inventory_ids")
    if isinstance(inventory_ids, list) and inventory_ids != sorted(inventory_ids):
        fail(path, f"{suite_id} inventory_ids must use canonical sorted order")


def _validate_inventory_projection(
    *,
    fail: Fail,
    path: Path,
    suite_id: str,
    inventory_ids: list[Any],
    inventory: Mapping[str, Mapping[str, Any]],
    bindings: list[Any],
) -> None:
    for inventory_id in inventory_ids:
        if isinstance(inventory_id, str) and inventory_id not in inventory:
            fail(path, f"{suite_id} inventory_ids references unknown gate inventory id {inventory_id}")
    expected = {
        inventory_id
        for inventory_id, inventory_record in inventory.items()
        if suite_id in inventory_record.get("suite_ids", [])
    }
    actual = {value for value in inventory_ids if isinstance(value, str)}
    for inventory_id in sorted(expected - actual):
        fail(path, f"{suite_id} gate inventory row {inventory_id} assigned to {suite_id} is not referenced")
    for inventory_id in sorted(actual - expected):
        if inventory_id in inventory:
            fail(path, f"{suite_id} inventory_ids row {inventory_id} is not directly assigned to {suite_id}")
    for inventory_id in bindings:
        if isinstance(inventory_id, str) and inventory_id not in actual:
            fail(path, f"{suite_id} command binding {inventory_id} is absent from inventory_ids")

    expected_entrypoints = {
        inventory_id
        for inventory_id, inventory_record in inventory.items()
        if suite_id in inventory_record.get("suite_ids", [])
        and inventory_record.get("command_role") == "entrypoint"
    }
    actual_bindings = {value for value in bindings if isinstance(value, str)}
    missing_bindings = sorted(expected_entrypoints - actual_bindings)
    extra_bindings = sorted(actual_bindings - expected_entrypoints)
    if missing_bindings or extra_bindings:
        details = []
        if missing_bindings:
            details.append(f"missing={','.join(missing_bindings)}")
        if extra_bindings:
            details.append(f"extra={','.join(extra_bindings)}")
        fail(
            path,
            f"{suite_id} command_bindings must exactly cover directly assigned entrypoint rows; "
            + "; ".join(details),
        )


def _validate_command_binding(
    *,
    fail: Fail,
    path: Path,
    suite_id: str,
    label: str,
    inventory_record: Mapping[str, Any],
) -> None:
    suite_ids = inventory_record.get("suite_ids")
    if not isinstance(suite_ids, list) or suite_id not in suite_ids:
        fail(path, f"{label} gate inventory row is not directly assigned to {suite_id}")
    if inventory_record.get("disposition") not in {"assigned", "report_only"}:
        fail(path, f"{label} gate inventory row is not an executable milestone input")

    required_env = inventory_record.get("required_env")
    strict_hardware = isinstance(required_env, list) and HARDWARE_OPT_IN in required_env
    if suite_id == "hardware_lab":
        if not strict_hardware:
            fail(path, f"{label} must require {HARDWARE_OPT_IN}")
        if inventory_record.get("environment") not in HARDWARE_ENVIRONMENTS:
            fail(path, f"{label} hardware command must use a hardware-capable environment")
        if inventory_record.get("artifact_kind") == "none":
            fail(path, f"{label} hardware command must name a lab result artifact")
    elif strict_hardware:
        fail(path, f"{label} hardware opt-in outside hardware_lab")

    if suite_id == "release":
        artifact_kind = inventory_record.get("artifact_kind")
        artifact_paths = inventory_record.get("artifact_paths")
        if artifact_kind not in DURABLE_ARTIFACT_KINDS:
            fail(path, f"{label} release command must name durable evidence or a CI artifact")
        if isinstance(artifact_paths, list) and any(_uses_target(item) for item in artifact_paths):
            fail(path, f"{label} release command artifact cannot use target/")
        if artifact_kind in {"ci_artifact", "ci_job_result"} and (
            inventory_record.get("source_kind") != "github_workflow_job"
            or not str(inventory_record.get("path", "")).startswith(".github/workflows/")
        ):
            fail(path, f"{label} CI evidence must name a workflow job")


def _uses_target(value: str) -> bool:
    normalized = value.replace("\\", "/")
    return normalized == "target" or normalized.startswith("target/")


def _path(record: Mapping[str, Any]) -> Path:
    value = record.get("_path")
    return value if isinstance(value, Path) else Path("verification/suites/<unknown>.toml")
