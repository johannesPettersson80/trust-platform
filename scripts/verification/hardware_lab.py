"""Closed Phase 11 hardware-lab program contract."""

from __future__ import annotations

import re
import subprocess
import tomllib
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any


HARDWARE_LAB_PATH = "verification/hardware-lab.toml"
MANIFEST_SCHEMA_PATH = "verification/schemas/hardware-lab.schema.json"
REPORT_SCHEMA_PATH = "verification/schemas/hardware-lab-report.schema.json"
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/hardware-lab.json")
DEFAULT_MARKDOWN_PATH = Path(
    "docs/internal/testing/evidence/plc-verification-program/2026-07-19/phase11-hardware-lab.md"
)
CASE_IDS = (
    "LAB_MODBUS_TCP_001",
    "LAB_MQTT_BROKER_001",
    "LAB_ADS_TWINCAT_001",
    "LAB_ETHERCAT_DISCOVERY_001",
    "LAB_ETHERCAT_STORAGE_001",
    "LAB_GPIO_OUTPUT_001",
)
PROTOCOLS = ("modbus_tcp", "mqtt", "ads_twincat", "ethercat", "gpio")
TOP_FIELDS = {
    "schema_version", "id", "owner", "status", "last_reviewed", "suite_id",
    "entrypoint_script", "workflow_path", "rust_harness_path", "gpio_script_path",
    "hardware_claim_spec_source_id", "public_claim_status", "strict_opt_in",
    "artifact_contract", "cases",
}
CASE_FIELDS = {
    "id", "board_row", "protocol", "title", "owner", "binding_kind",
    "ignored_test_ids", "command", "required_env_vars", "topology", "topology_ref",
    "artifact_paths", "assertions", "proof_status", "evidence_ids",
    "public_claim_impact",
}
SOURCE_BINDINGS = {
    "entrypoint_script": "scripts/runtime_device_in_loop_gate.sh",
    "workflow_path": ".github/workflows/protocol-device-in-loop.yml",
    "rust_harness_path": "crates/trust-runtime/tests/device_in_the_loop.rs",
    "gpio_script_path": "scripts/gpio_hardware_test.sh",
}
GPIO_PROJECT_FILES = (
    "examples/communication/gpio/README.md",
    "examples/communication/gpio/io.toml",
    "examples/communication/gpio/runtime.toml",
    "examples/communication/gpio/src/config.st",
    "examples/communication/gpio/src/main.st",
    "examples/communication/gpio/trust-lsp.toml",
)
BOARD_ROWS = {
    "modbus_tcp": "VERIF-P11-002",
    "mqtt": "VERIF-P11-003",
    "ads_twincat": "VERIF-P11-004",
    "ethercat": "VERIF-P11-005",
    "gpio": "VERIF-P11-006",
}
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
ENV_RE = re.compile(r"^[A-Z][A-Z0-9_]*$")


def load_hardware_lab_document(root: Path) -> dict[str, Any]:
    with (root / HARDWARE_LAB_PATH).open("rb") as handle:
        return tomllib.load(handle)


def validate_hardware_lab_document(
    root: Path,
    document: Mapping[str, Any],
    *,
    ignored_tests: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    suites: Mapping[str, Mapping[str, Any]],
    gate_inventory: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    failures: list[str] = []
    if set(document) != TOP_FIELDS:
        failures.append("hardware-lab program fields drift from the closed contract")
    for field, expected in (
        ("schema_version", 1),
        ("id", "HARDWARE_LAB_PROGRAM_001"),
        ("owner", "verification"),
        ("status", "mapped"),
        ("suite_id", "hardware_lab"),
        ("hardware_claim_spec_source_id", "SPEC_RELEASE_EVIDENCE_001"),
        ("public_claim_status", "preview_unverified"),
        ("strict_opt_in", "TRUST_DIT_REQUIRE_HARDWARE=1"),
        ("artifact_contract", "target/gate-artifacts/device-in-the-loop/*.json"),
    ):
        if document.get(field) != expected:
            failures.append(f"hardware-lab {field} must equal {expected!r}")
    if not DATE_RE.fullmatch(str(document.get("last_reviewed", ""))):
        failures.append("hardware-lab last_reviewed must be YYYY-MM-DD")
    for field, expected in SOURCE_BINDINGS.items():
        if document.get(field) != expected:
            failures.append(f"hardware-lab {field} must reuse {expected}")
        failures.extend(_validate_tracked_source(root, expected, field))

    suite = suites.get("hardware_lab", {})
    if suite.get("commands") != [SOURCE_BINDINGS["entrypoint_script"]]:
        failures.append("hardware-lab suite must retain the existing strict entrypoint")
    if suite.get("command_bindings") != ["GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP"]:
        failures.append("hardware-lab suite command binding drifted")
    entrypoint = gate_inventory.get("GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP", {})
    workflow = gate_inventory.get("GATE_JOB_PROTOCOL_DEVICE_IN_LOOP", {})
    if entrypoint.get("path") != SOURCE_BINDINGS["entrypoint_script"]:
        failures.append("hardware-lab gate entrypoint does not bind the existing script")
    if entrypoint.get("required_env") != ["TRUST_DIT_REQUIRE_HARDWARE=1"]:
        failures.append("hardware-lab strict entrypoint must require explicit hardware opt-in")
    if workflow.get("path") != SOURCE_BINDINGS["workflow_path"]:
        failures.append("hardware-lab workflow helper does not bind the existing workflow")

    source = spec_sources.get("SPEC_RELEASE_EVIDENCE_001", {})
    if source.get("source_status") != "active" or source.get("oracle_eligible") is not True:
        failures.append("hardware claim boundary requires the active release-evidence contract")
    if source.get("path") != "docs/specs/24-release-evidence.md":
        failures.append("hardware claim boundary source path drifted")
    try:
        board = (root / BOARD_PATH).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        failures.append(f"hardware-lab board cannot be read: {exc}")
    else:
        for row_id in ("VERIF-P5-000B", "VERIF-P11-001", *sorted(set(BOARD_ROWS.values())), "VERIF-P11-007", "VERIF-P11-008"):
            if board.count(f"`{row_id}`") != 1:
                failures.append(f"hardware-lab board row {row_id} must exist exactly once")

    cases = document.get("cases")
    if not isinstance(cases, list):
        failures.append("hardware-lab cases must be an array")
        return sorted(set(failures))
    if tuple(row.get("id") for row in cases if isinstance(row, Mapping)) != CASE_IDS:
        failures.append("hardware-lab cases drift from reviewed order")

    mapped_ignored: list[str] = []
    for index, value in enumerate(cases):
        if not isinstance(value, Mapping):
            failures.append(f"hardware-lab case {index} must be a table")
            continue
        _validate_case(root, value, ignored_tests, mapped_ignored, failures)
    expected_ignored = sorted(
        key for key, row in ignored_tests.items() if row.get("ignore_class") == "lab_required"
    )
    if sorted(mapped_ignored) != expected_ignored or len(mapped_ignored) != len(set(mapped_ignored)):
        failures.append("hardware-lab cases must exactly partition the lab-required ignored-test partition")
    return sorted(set(failures))


def validate_hardware_lab_schema(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("hardware-lab schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS or set(schema.get("properties", {})) != TOP_FIELDS:
        failures.append("hardware-lab schema top-level fields drift from validator")
    case = schema.get("$defs", {}).get("case", {})
    if set(case.get("required", [])) != CASE_FIELDS or set(case.get("properties", {})) != CASE_FIELDS:
        failures.append("hardware-lab schema case fields drift from validator")
    properties = schema.get("properties", {})
    for field, expected in (
        ("id", "HARDWARE_LAB_PROGRAM_001"),
        ("suite_id", "hardware_lab"),
        ("entrypoint_script", SOURCE_BINDINGS["entrypoint_script"]),
        ("workflow_path", SOURCE_BINDINGS["workflow_path"]),
        ("rust_harness_path", SOURCE_BINDINGS["rust_harness_path"]),
        ("gpio_script_path", SOURCE_BINDINGS["gpio_script_path"]),
        ("public_claim_status", "preview_unverified"),
    ):
        if properties.get(field, {}).get("const") != expected:
            failures.append(f"hardware-lab schema {field} const drifted")
    case_ids = case.get("properties", {}).get("id", {}).get("enum")
    protocols = case.get("properties", {}).get("protocol", {}).get("enum")
    if case_ids != list(CASE_IDS):
        failures.append("hardware-lab schema case IDs drift from reviewed order")
    if protocols != list(PROTOCOLS):
        failures.append("hardware-lab schema protocols drift from reviewed order")
    return sorted(set(failures))


def _validate_case(
    root: Path,
    row: Mapping[str, Any],
    ignored_tests: Mapping[str, Mapping[str, Any]],
    mapped_ignored: list[str],
    failures: list[str],
) -> None:
    case_id = str(row.get("id", "<unknown>"))
    label = f"hardware-lab case {case_id}"
    if set(row) != CASE_FIELDS:
        failures.append(f"{label} fields drift from the closed contract")
    protocol = row.get("protocol")
    if protocol not in PROTOCOLS:
        failures.append(f"{label} has unsupported protocol")
    elif row.get("board_row") != BOARD_ROWS[protocol]:
        failures.append(f"{label} board row does not match protocol")
    for field in ("title", "owner", "command", "topology", "topology_ref", "public_claim_impact"):
        if not isinstance(row.get(field), str) or not row[field].strip():
            failures.append(f"{label} requires non-empty {field}")
    for field in ("artifact_paths", "assertions"):
        if not _nonempty_strings(row.get(field)):
            failures.append(f"{label} requires a non-empty {field} array")
    if row.get("proof_status") != "skipped_unproven":
        failures.append(f"{label} must remain skipped_unproven until durable lab evidence exists")
    if row.get("evidence_ids") != []:
        failures.append(f"{label} skipped/unproven case cannot carry evidence")
    env_vars = row.get("required_env_vars")
    if not isinstance(env_vars, list) or env_vars != sorted(set(env_vars)) or any(not isinstance(item, str) or not ENV_RE.fullmatch(item) for item in env_vars):
        failures.append(f"{label} required_env_vars must be sorted unique public names")
    topology_ref = row.get("topology_ref")
    if isinstance(topology_ref, str):
        failures.extend(_validate_tracked_source(root, topology_ref, f"{label} topology_ref"))

    ignored_ids = row.get("ignored_test_ids")
    if not isinstance(ignored_ids, list) or ignored_ids != sorted(set(ignored_ids)):
        failures.append(f"{label} ignored_test_ids must be a sorted unique array")
        ignored_ids = []
    binding_kind = row.get("binding_kind")
    if protocol == "gpio":
        if binding_kind != "manual_script":
            failures.append("GPIO hardware-lab case must remain bound to the existing manual script")
        if ignored_ids:
            failures.append("GPIO hardware-lab case cannot invent an ignored Rust test binding")
        if row.get("command") != "scripts/gpio_hardware_test.sh examples/communication/gpio":
            failures.append("GPIO hardware-lab command must bind the existing script and tracked example")
        if row.get("required_env_vars") != []:
            failures.append("GPIO manual case has no required environment-variable contract")
        for relative in GPIO_PROJECT_FILES:
            failures.extend(_validate_tracked_source(root, relative, f"GPIO project file {relative}"))
    else:
        if binding_kind != "strict_harness":
            failures.append(f"{label} must use the existing strict harness")
        if row.get("command") != SOURCE_BINDINGS["entrypoint_script"]:
            failures.append(f"{label} command must reuse the strict entrypoint")
        if not ignored_ids:
            failures.append(f"{label} strict case requires an ignored-test binding")

    for ignored_id in ignored_ids:
        mapped_ignored.append(ignored_id)
        ignored = ignored_tests.get(ignored_id)
        if ignored is None:
            failures.append(f"{label} references unknown ignored test {ignored_id}")
            continue
        if ignored.get("ignore_class") != "lab_required":
            failures.append(f"{label} references non-lab ignored test {ignored_id}")
        if ignored.get("path") != SOURCE_BINDINGS["rust_harness_path"]:
            failures.append(f"{label} ignored test is outside the reviewed Rust harness")
        if ignored.get("required_env_vars") != row.get("required_env_vars"):
            failures.append(f"{label} required environment does not match {ignored_id}")
        if ignored.get("hardware_topology_ref") != row.get("topology_ref"):
            failures.append(f"{label} topology reference does not match {ignored_id}")


def _validate_tracked_source(root: Path, relative: str, label: str) -> list[str]:
    failures: list[str] = []
    if not _safe_path(relative):
        return [f"{label} must be a normalized workspace-relative path"]
    candidate = root / relative
    if not candidate.is_file() or candidate.is_symlink():
        failures.append(f"{label} must resolve to a regular non-symlink file")
    result = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", relative],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        failures.append(f"{label} must be tracked")
    return failures


def _safe_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts


def _nonempty_strings(value: Any) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) and item.strip() for item in value)
