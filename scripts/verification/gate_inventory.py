"""Fail-closed contract for reviewed gate scripts and GitHub workflow jobs."""

from __future__ import annotations

import json
import re
import tomllib
from collections.abc import Callable, Mapping
from pathlib import Path
from typing import Any

from .test_catalog_common import source_files
from .test_catalog_surfaces import (
    WORKFLOW_JOB_RE,
    YAML_NAME_RE,
    scan_gate_scripts,
    scan_workflow_jobs,
    unquote_yaml_scalar,
)


INVENTORY_PATH = Path("verification/gate-inventory.toml")
SCHEMA_PATH = Path("verification/schemas/gate-inventory.schema.json")

SOURCE_KINDS = {
    "catalog_test_command",
    "gate_script",
    "github_workflow_job",
    "workflow_template",
    "just_recipe",
}
COMMAND_ROLES = {"entrypoint", "helper", "reference"}
DISPOSITIONS = {"assigned", "report_only", "supporting", "excluded"}
ENFORCEMENTS = {
    "required",
    "conditional",
    "planned",
    "report_only",
    "supporting",
    "advisory",
    "excluded",
    "non_executable",
}
SUITE_IDS = {"veryquick", "pr", "nightly", "release", "hardware_lab", "supporting_local"}
DURATION_CLASSES = {"very_fast", "fast", "medium", "slow", "long", "lab", "manual"}
ENVIRONMENTS = {
    "local_or_ci",
    "local",
    "github_ubuntu",
    "github_matrix",
    "github_self_hosted_linux",
    "github_or_lab_runner",
    "github_release",
    "github_release_matrix",
    "github_pages",
    "github_nightly",
    "nightly_toolchain",
    "linux_tooling",
    "trust_builder",
    "consumer_project_ci",
}
ARTIFACT_KINDS = {
    "ci_job_result",
    "none",
    "machine_local",
    "ci_artifact",
    "release_object",
    "lab_report",
    "committed_file",
}
ARTIFACT_RETENTIONS = {
    "none",
    "machine_local",
    "repository_default",
    "release_object",
    "committed",
}

REQUIRED_FIELDS = {
    "schema_version",
    "id",
    "source_kind",
    "path",
    "name",
    "command",
    "variant",
    "command_role",
    "disposition",
    "suite_ids",
    "owner",
    "duration_class",
    "environment",
    "artifact_kind",
    "artifact_paths",
    "artifact_retention",
    "enforcement",
    "required_env",
    "rationale",
}
OPTIONAL_FIELDS = {"discovery_id"}
ID_RE = re.compile(r"^GATE_[A-Z0-9_]+$")
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
ENV_RE = re.compile(r"^[A-Z][A-Z0-9_]*=[^\s=]+$")
CATALOG_PATH = Path("verification/test-catalog.toml")
HARDWARE_TEST_PATH = Path("crates/trust-runtime/tests/device_in_the_loop.rs")
HARDWARE_OPT_IN = "TRUST_DIT_REQUIRE_HARDWARE=1"
REVIEWED_JUST_RECIPES = {
    "verification-veryquick": (
        "mkdir -p target/gate-artifacts/veryquick",
        "python3 scripts/run_verification_focused_tests.py",
        "scripts/verification_metadata_gate.sh",
        "just test-hir-fast",
        "just test-fast",
        "./scripts/cargo_test_fast_link.sh test -p trust-syntax --lib",
        "./scripts/cargo_test_fast_link.sh test -p trust-runtime-core --lib",
        "./scripts/cargo_test_fast_link.sh test -p trust-runtime --test bytecode_validation",
        "cargo run -p trust-runtime --bin trust-runtime -- conformance --suite-root conformance "
        "--filter cfm_arithmetic_conversion_compare_001 --output "
        "target/gate-artifacts/veryquick/conformance.json",
    )
}


class GateInventoryError(ValueError):
    """Raised when the inventory cannot be loaded without losing identity."""


def load_gate_inventory(root: Path) -> dict[str, dict[str, Any]]:
    """Load the committed inventory keyed by reviewed inventory ID."""

    path = root / INVENTORY_PATH
    try:
        payload = tomllib.loads(path.read_text())
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as exc:
        raise GateInventoryError(f"cannot load {INVENTORY_PATH}: {exc}") from exc
    if set(payload) != {"schema_version", "surfaces"}:
        raise GateInventoryError(
            "gate inventory top-level fields drift: "
            f"expected ['schema_version', 'surfaces'], got {sorted(payload)}"
        )
    if payload.get("schema_version") != 1:
        raise GateInventoryError("gate inventory top-level schema_version must be 1")
    rows = payload.get("surfaces")
    if not isinstance(rows, list):
        raise GateInventoryError("gate inventory surfaces must be an array of tables")
    records: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise GateInventoryError(f"gate inventory surfaces[{index}] must be a table")
        record_id = row.get("id")
        if not isinstance(record_id, str) or not record_id:
            raise GateInventoryError(f"gate inventory surfaces[{index}] has no id")
        if record_id in records:
            raise GateInventoryError(f"duplicate gate inventory id {record_id}")
        records[record_id] = dict(row)
    return records


def validate_gate_inventory(
    root: Path,
    records: Mapping[str, Mapping[str, Any]] | None = None,
    *,
    on_failure: Callable[[Path, str], None] | None = None,
) -> list[str]:
    """Validate inventory semantics and its exhaustive join to current source facts."""

    root = root.resolve()
    failures: list[str] = []
    if records is None:
        try:
            records = load_gate_inventory(root)
        except GateInventoryError as exc:
            failures.append(str(exc))
            return _finish(root, failures, on_failure)
        _validate_schema_contract(root, failures)
    normalized = {key: dict(value) for key, value in records.items()}
    for key, record in normalized.items():
        _validate_record(key, record, failures)

    live_facts = []
    for batch in (scan_gate_scripts(root), scan_workflow_jobs(root)):
        live_facts.extend(batch.facts)
        for diagnostic in batch.diagnostics:
            if diagnostic.severity == "error":
                failures.append(
                    f"live discovery failed at {diagnostic.path}:{diagnostic.line}: {diagnostic.message}"
                )
    live_by_id: dict[str, Any] = {}
    for fact in live_facts:
        if fact.stable_id in live_by_id:
            failures.append(f"live discovery_id {fact.stable_id} is not unique")
        live_by_id[fact.stable_id] = fact

    mapped: dict[str, list[str]] = {}
    for record_id, record in normalized.items():
        discovery_id = record.get("discovery_id")
        if not isinstance(discovery_id, str):
            continue
        mapped.setdefault(discovery_id, []).append(record_id)
        fact = live_by_id.get(discovery_id)
        if fact is None:
            failures.append(f"{record_id}: unknown live discovery_id {discovery_id}")
            continue
        for field, expected in (
            ("source_kind", fact.source_kind),
            ("path", fact.path),
            ("name", fact.name),
            ("command", fact.command_hint),
        ):
            if record.get(field) != expected:
                failures.append(
                    f"{record_id}: {field} binding mismatch; expected {expected!r}, got {record.get(field)!r}"
                )
    for discovery_id, record_ids in sorted(mapped.items()):
        if len(record_ids) > 1:
            failures.append(
                f"live discovery_id {discovery_id} is mapped by multiple inventory records: "
                + ", ".join(sorted(record_ids))
            )
    for discovery_id, fact in sorted(live_by_id.items()):
        if discovery_id not in mapped:
            failures.append(
                f"missing inventory record for live fact {discovery_id} {fact.path} {fact.name}"
            )

    _validate_template_partition(root, normalized, failures)
    _validate_just_recipe_sources(root, normalized, failures)
    _validate_catalog_command_sources(root, normalized, failures)
    _validate_workflow_artifact_sources(root, normalized, failures)
    _validate_hardware_source_contract(root, normalized, failures)
    _validate_report_only_source_contract(root, normalized, failures)
    return _finish(root, sorted(set(failures)), on_failure)


def _validate_record(record_key: str, record: dict[str, Any], failures: list[str]) -> None:
    record_id = record.get("id")
    if record_id != record_key:
        failures.append(f"{record_key}: mapping key does not match record id {record_id!r}")
    if not isinstance(record_id, str) or not ID_RE.fullmatch(record_id):
        failures.append(f"{record_key}: id must match {ID_RE.pattern}")
    fields = set(record)
    missing = REQUIRED_FIELDS - fields
    extra = fields - REQUIRED_FIELDS - OPTIONAL_FIELDS
    if missing:
        failures.append(f"{record_key}: missing required fields {sorted(missing)}")
    if extra:
        failures.append(f"{record_key}: unexpected fields {sorted(extra)}")
    if record.get("schema_version") != 1:
        failures.append(f"{record_key}: schema_version must be 1")
    _enum(record_key, record, "source_kind", SOURCE_KINDS, failures)
    _enum(record_key, record, "command_role", COMMAND_ROLES, failures)
    _enum(record_key, record, "disposition", DISPOSITIONS, failures)
    _enum(record_key, record, "enforcement", ENFORCEMENTS, failures)
    _enum(record_key, record, "duration_class", DURATION_CLASSES, failures)
    _enum(record_key, record, "environment", ENVIRONMENTS, failures)
    _enum(record_key, record, "artifact_kind", ARTIFACT_KINDS, failures)
    _enum(record_key, record, "artifact_retention", ARTIFACT_RETENTIONS, failures)
    for field in ("path", "name", "command", "variant", "owner", "rationale"):
        if not isinstance(record.get(field), str) or not record.get(field, "").strip():
            failures.append(f"{record_key}: {field} must be a non-empty string")
    for field in ("suite_ids", "artifact_paths", "required_env"):
        values = record.get(field)
        if not isinstance(values, list) or any(not isinstance(item, str) or not item for item in values):
            failures.append(f"{record_key}: {field} must be an array of non-empty strings")
        elif values != sorted(set(values)):
            failures.append(f"{record_key}: {field} must be sorted and unique")
    suite_ids = record.get("suite_ids") if isinstance(record.get("suite_ids"), list) else []
    unknown_suites = set(suite_ids) - SUITE_IDS
    if unknown_suites:
        failures.append(f"{record_key}: unknown suite_ids {sorted(unknown_suites)}")
    required_env = record.get("required_env") if isinstance(record.get("required_env"), list) else []
    if any(not ENV_RE.fullmatch(item) for item in required_env if isinstance(item, str)):
        failures.append(f"{record_key}: required_env entries must be canonical NAME=VALUE assignments")

    source_kind = record.get("source_kind")
    command_role = record.get("command_role")
    discovery_id = record.get("discovery_id")
    if source_kind == "workflow_template":
        if "discovery_id" in record:
            failures.append(f"{record_key}: workflow_template forbids discovery_id")
        if record.get("disposition") != "excluded":
            failures.append(f"{record_key}: workflow_template must be excluded")
        if record.get("enforcement") != "non_executable":
            failures.append(f"{record_key}: workflow_template must be non_executable")
        if command_role != "reference":
            failures.append(f"{record_key}: workflow_template must use command_role = reference")
    elif source_kind == "just_recipe":
        if "discovery_id" in record:
            failures.append(f"{record_key}: just_recipe forbids discovery_id")
        if record.get("path") != "justfile":
            failures.append(f"{record_key}: just_recipe path must be justfile")
        if record.get("command") != f"just {record.get('name')}":
            failures.append(f"{record_key}: just_recipe command must match its recipe name")
        if suite_ids != ["veryquick"]:
            failures.append(f"{record_key}: just_recipe must map directly to veryquick")
        if record.get("environment") != "trust_builder":
            failures.append(f"{record_key}: just_recipe must use environment = trust_builder")
        if command_role != "entrypoint":
            failures.append(f"{record_key}: just_recipe must use command_role = entrypoint")
    elif source_kind == "catalog_test_command":
        if "discovery_id" in record:
            failures.append(f"{record_key}: catalog_test_command forbids discovery_id")
    elif not isinstance(discovery_id, str) or not DISCOVERY_ID_RE.fullmatch(discovery_id):
        failures.append(f"{record_key}: live surface requires a valid discovery_id")

    disposition = record.get("disposition")
    enforcement = record.get("enforcement")
    if disposition == "assigned":
        if not suite_ids:
            failures.append(f"{record_key}: assigned requires at least one suite_id")
        if enforcement not in {"required", "conditional", "planned"}:
            failures.append(f"{record_key}: assigned forbids enforcement = {enforcement}")
        if command_role not in {"entrypoint", "helper"}:
            failures.append(f"{record_key}: assigned requires command_role = entrypoint or helper")
    elif disposition == "report_only":
        if not suite_ids:
            failures.append(f"{record_key}: report_only requires at least one suite_id")
        if enforcement != "report_only":
            failures.append(f"{record_key}: report_only requires enforcement = report_only")
        if command_role not in {"entrypoint", "helper"}:
            failures.append(f"{record_key}: report_only requires command_role = entrypoint or helper")
    elif disposition == "supporting":
        if suite_ids != ["supporting_local"]:
            failures.append(f"{record_key}: supporting requires suite_ids = ['supporting_local']")
        if enforcement != "supporting":
            failures.append(f"{record_key}: supporting requires enforcement = supporting")
        if command_role != "helper":
            failures.append(f"{record_key}: supporting requires command_role = helper")
    elif disposition == "excluded":
        if suite_ids:
            failures.append(f"{record_key}: excluded requires empty suite_ids")
        if command_role != "reference":
            failures.append(f"{record_key}: excluded requires command_role = reference")
        if enforcement not in {"advisory", "excluded", "non_executable"}:
            failures.append(f"{record_key}: excluded has incompatible enforcement = {enforcement}")

    artifact_kind = record.get("artifact_kind")
    artifact_paths = record.get("artifact_paths") if isinstance(record.get("artifact_paths"), list) else []
    retention = record.get("artifact_retention")
    if artifact_kind == "none":
        if artifact_paths:
            failures.append(f"{record_key}: artifact_kind = none requires empty artifact_paths")
        if retention != "none":
            failures.append(f"{record_key}: artifact_kind = none requires artifact_retention = none")
    else:
        if not artifact_paths:
            failures.append(f"{record_key}: {artifact_kind} requires non-empty artifact_paths")
        allowed_retention = {
            "machine_local": {"machine_local"},
            "ci_artifact": {"repository_default"},
            "ci_job_result": {"repository_default"},
            "release_object": {"release_object"},
            "lab_report": {"machine_local", "repository_default", "committed"},
            "committed_file": {"committed"},
        }.get(artifact_kind, set())
        if retention not in allowed_retention:
            failures.append(
                f"{record_key}: {artifact_kind} requires artifact_retention = "
                + " or ".join(sorted(allowed_retention))
            )
    strict_hardware = HARDWARE_OPT_IN in required_env
    if strict_hardware and (
        suite_ids != ["hardware_lab"] or command_role != "entrypoint"
    ):
        failures.append(
            f"{record_key}: {HARDWARE_OPT_IN} is reserved to the exclusive "
            "hardware_lab entrypoint"
        )
    if "hardware_lab" in suite_ids and command_role == "entrypoint":
        if not strict_hardware:
            failures.append(
                f"{record_key}: hardware_lab requires {HARDWARE_OPT_IN} in required_env"
            )
        if record.get("environment") != "github_or_lab_runner":
            failures.append(f"{record_key}: hardware_lab requires environment = github_or_lab_runner")
    elif "hardware_lab" in suite_ids and required_env:
        failures.append(
            f"{record_key}: non-entrypoint hardware_lab rows cannot claim static required_env"
        )


def _validate_template_partition(
    root: Path,
    records: Mapping[str, dict[str, Any]],
    failures: list[str],
) -> None:
    expected = {(item["path"], item["name"], item["command"]) for item in _template_facts(root)}
    actual: dict[tuple[str, str, str], list[str]] = {}
    for record_id, record in records.items():
        if record.get("source_kind") != "workflow_template":
            continue
        identity = (record.get("path"), record.get("name"), record.get("command"))
        actual.setdefault(identity, []).append(record_id)
    for identity in sorted(expected - set(actual)):
        failures.append(f"missing non-executable workflow template exclusion {identity!r}")
    for identity in sorted(set(actual) - expected):
        path, name, command = identity
        if not any(item["path"] == path for item in _template_facts(root)):
            failures.append(f"invented workflow template exclusion {identity!r}")
        elif not any(item["name"] == name for item in _template_facts(root) if item["path"] == path):
            failures.append(f"template name binding mismatch for {path}: {name!r}")
        else:
            failures.append(f"template command binding mismatch for {path}: {command!r}")
    for identity, record_ids in actual.items():
        if len(record_ids) > 1:
            failures.append(f"workflow template {identity!r} is mapped more than once")


def _validate_just_recipe_sources(
    root: Path,
    records: Mapping[str, dict[str, Any]],
    failures: list[str],
) -> None:
    recipe_records = [
        (record_id, record)
        for record_id, record in records.items()
        if record.get("source_kind") == "just_recipe"
    ]
    if not recipe_records:
        return
    justfile = root / "justfile"
    if justfile.is_symlink() or not justfile.is_file():
        failures.append("just recipe source must be a regular nonsymlinked justfile")
        return
    try:
        text = justfile.read_text()
    except (OSError, UnicodeError) as exc:
        failures.append(f"just recipe source cannot be read: {exc}")
        return
    for record_id, record in recipe_records:
        name = record.get("name")
        body = _just_recipe_body(text, name) if isinstance(name, str) else None
        if body is None:
            failures.append(f"{record_id}: just recipe {name} is absent from justfile")
            continue
        expected = REVIEWED_JUST_RECIPES.get(name)
        if expected is None or body != expected:
            failures.append(
                f"{record_id}: just recipe {name} does not match the reviewed command sequence"
            )


def _just_recipe_body(text: str, name: str) -> tuple[str, ...] | None:
    lines = text.splitlines()
    header = re.compile(rf"^{re.escape(name)}:(?:\s|$)")
    start = next((index for index, line in enumerate(lines) if header.match(line)), None)
    if start is None:
        return None
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line and not line.startswith((" ", "\t", "#")):
            break
        if line.startswith("\t"):
            body.append(line[1:])
    return tuple(body)


def _validate_catalog_command_sources(
    root: Path,
    records: Mapping[str, dict[str, Any]],
    failures: list[str],
) -> None:
    catalog_records = [
        (record_id, record)
        for record_id, record in records.items()
        if record.get("source_kind") == "catalog_test_command"
    ]
    if not catalog_records:
        return
    path = root / CATALOG_PATH
    if path.is_symlink() or not path.is_file():
        failures.append("catalog command source must be a regular nonsymlinked test catalog")
        return
    try:
        payload = tomllib.loads(path.read_text())
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as exc:
        failures.append(f"cannot load catalog command source: {exc}")
        return
    rows = payload.get("tests")
    if not isinstance(rows, list):
        failures.append("catalog command source has no tests array")
        return
    by_id: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        if isinstance(row, dict) and isinstance(row.get("id"), str):
            by_id.setdefault(row["id"], []).append(row)
    for record_id, record in catalog_records:
        test_id = record.get("name")
        matches = by_id.get(test_id, []) if isinstance(test_id, str) else []
        if len(matches) != 1:
            failures.append(
                f"{record_id}: catalog command id {test_id!r} must resolve exactly once"
            )
            continue
        row = matches[0]
        for field in ("path", "command"):
            if record.get(field) != row.get(field):
                failures.append(
                    f"{record_id}: catalog command binding mismatch for {field}; "
                    f"expected {row.get(field)!r}, got {record.get(field)!r}"
                )


def _validate_workflow_artifact_sources(
    root: Path,
    records: Mapping[str, dict[str, Any]],
    failures: list[str],
) -> None:
    for record_id, record in records.items():
        if record.get("source_kind") != "github_workflow_job":
            continue
        if record.get("command_role") == "reference":
            continue
        artifact_kind = record.get("artifact_kind")
        if artifact_kind == "ci_job_result":
            if record.get("artifact_paths") != [record.get("name")]:
                failures.append(
                    f"{record_id}: CI job result locator must equal the live workflow identity"
                )
            continue
        if artifact_kind not in {"ci_artifact", "lab_report", "release_object"}:
            continue
        block = _workflow_job_block(root, record)
        if block is None:
            failures.append(f"{record_id}: cannot resolve workflow job source for artifact binding")
            continue
        for claim in record.get("artifact_paths", []):
            if isinstance(claim, str) and claim not in block:
                failures.append(
                    f"{record_id}: artifact claim is absent from workflow job source: {claim}"
                )


def _workflow_job_block(root: Path, record: Mapping[str, Any]) -> str | None:
    path_value = record.get("path")
    command = record.get("command")
    if not isinstance(path_value, str) or not isinstance(command, str) or "#" not in command:
        return None
    path = root / path_value
    if path.is_symlink() or not path.is_file():
        return None
    job_id = command.rsplit("#", 1)[1]
    try:
        lines = path.read_text().splitlines()
    except (OSError, UnicodeError):
        return None
    start = next(
        (
            index
            for index, line in enumerate(lines)
            if re.fullmatch(rf"  {re.escape(job_id)}:\s*(?:#.*)?", line)
        ),
        None,
    )
    if start is None:
        return None
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if WORKFLOW_JOB_RE.match(lines[index]):
            end = index
            break
        if lines[index] and not lines[index].startswith((" ", "\t", "#")):
            end = index
            break
    return "\n".join(lines[start:end])


def _validate_hardware_source_contract(
    root: Path,
    records: Mapping[str, dict[str, Any]],
    failures: list[str],
) -> None:
    strict = records.get("GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP")
    if strict is not None:
        expected = {
            "source_kind": "gate_script",
            "path": "scripts/runtime_device_in_loop_gate.sh",
            "command": "scripts/runtime_device_in_loop_gate.sh",
            "command_role": "entrypoint",
        }
        for field, value in expected.items():
            if strict.get(field) != value:
                failures.append(
                    f"GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP: strict hardware {field} must be {value!r}"
                )
        expected_artifacts = ["target/gate-artifacts/device-in-the-loop/"]
        if strict.get("artifact_paths") != expected_artifacts:
            failures.append(
                "GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP: strict hardware artifact_paths "
                "must equal the reviewed script default"
            )
        _require_source_fragments(
            root / "scripts/runtime_device_in_loop_gate.sh",
            (
                'OUT_DIR="${OUT_DIR:-target/gate-artifacts/device-in-the-loop}"',
                'TRUST_DIT_ARTIFACT_DIR="${OUT_DIR}"',
                "cargo test -p trust-runtime --test device_in_the_loop -- --ignored --nocapture",
            ),
            "strict device-in-loop script",
            failures,
        )
        _require_source_fragments(
            root / HARDWARE_TEST_PATH,
            ('env_bool("TRUST_DIT_REQUIRE_HARDWARE")', "if require_hardware()", "panic!("),
            "strict device-in-loop test",
            failures,
        )
    workflow = records.get("GATE_JOB_PROTOCOL_DEVICE_IN_LOOP")
    if workflow is not None:
        if workflow.get("command_role") != "helper":
            failures.append(
                "GATE_JOB_PROTOCOL_DEVICE_IN_LOOP: skip-capable workflow must use command_role = helper"
            )
        block = _workflow_job_block(root, workflow)
        required = (
            "TRUST_DIT_REQUIRE_HARDWARE: ${{ github.event.inputs.require_hardware || 'false' }}",
            "run: scripts/runtime_device_in_loop_gate.sh",
        )
        if block is None or any(fragment not in block for fragment in required):
            failures.append(
                "GATE_JOB_PROTOCOL_DEVICE_IN_LOOP: workflow no longer exposes the reviewed skip-capable hardware input"
            )


def _validate_report_only_source_contract(
    root: Path,
    records: Mapping[str, dict[str, Any]],
    failures: list[str],
) -> None:
    record = records.get("GATE_JOB_VERIFICATION_REPORT")
    if record is None:
        return
    block = _workflow_job_block(root, record)
    path_value = record.get("path")
    path = root / path_value if isinstance(path_value, str) else None
    try:
        workflow = path.read_text() if path is not None else ""
    except (OSError, UnicodeError):
        workflow = ""
    if not re.search(r"(?m)^permissions:\n  contents: read\n\njobs:\n", workflow):
        failures.append(
            "GATE_JOB_VERIFICATION_REPORT: report-only workflow must retain read-only permissions"
        )
    if block is None:
        failures.append(
            "GATE_JOB_VERIFICATION_REPORT: report-only workflow source cannot be resolved"
        )
        return
    if "--strict" in block:
        failures.append(
            "GATE_JOB_VERIFICATION_REPORT: report-only workflow must not pass --strict"
        )
    expected = (
        "python3 scripts/verification_report_gate.py \\",
        '--base "${BASE_SHA}" \\',
        '--head "${HEAD_SHA}" \\',
        "--intent bugfix \\",
        "--out-dir target/gate-artifacts/verification",
    )
    invocation = _continued_command(block, "python3 scripts/verification_report_gate.py")
    if invocation != expected:
        failures.append(
            "GATE_JOB_VERIFICATION_REPORT: report-only workflow invocation drifts from the reviewed command"
        )


def _continued_command(block: str, prefix: str) -> tuple[str, ...]:
    lines = block.splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.strip().startswith(prefix)),
        None,
    )
    if start is None:
        return ()
    command: list[str] = []
    for line in lines[start:]:
        value = line.strip()
        command.append(value)
        if not value.endswith("\\"):
            break
    return tuple(command)


def _require_source_fragments(
    path: Path,
    fragments: tuple[str, ...],
    label: str,
    failures: list[str],
) -> None:
    if path.is_symlink() or not path.is_file():
        failures.append(f"{label} source must be a regular nonsymlinked file")
        return
    try:
        text = path.read_text()
    except (OSError, UnicodeError) as exc:
        failures.append(f"cannot read {label} source: {exc}")
        return
    missing = [fragment for fragment in fragments if fragment not in text]
    if missing:
        failures.append(f"{label} source is missing reviewed contract fragments: {missing}")


def _template_facts(root: Path) -> list[dict[str, str]]:
    workflows_root = root / ".github/workflows"
    result: list[dict[str, str]] = []
    for path in source_files(root, ".github/workflows", (".yml", ".yaml")):
        if path.parent == workflows_root:
            continue
        try:
            relative = path.resolve().relative_to(root).as_posix()
            lines = path.read_text().splitlines()
        except (OSError, UnicodeError, ValueError):
            continue
        workflow_name = path.stem
        for line in lines:
            match = YAML_NAME_RE.match(line)
            if match:
                workflow_name = unquote_yaml_scalar(match.group(1))
                break
        jobs_start = next(
            (index for index, line in enumerate(lines) if line.strip() == "jobs:" and not line.startswith(" ")),
            None,
        )
        if jobs_start is None:
            continue
        for line in lines[jobs_start + 1 :]:
            if line and not line.startswith((" ", "\t", "#")):
                break
            match = WORKFLOW_JOB_RE.match(line)
            if not match:
                continue
            job_id = match.group(1)
            result.append(
                {
                    "path": relative,
                    "name": f"{workflow_name} / {job_id}",
                    "command": f"non-executable workflow template {relative}#{job_id}",
                }
            )
    return result


def _validate_schema_contract(root: Path, failures: list[str]) -> None:
    path = root / SCHEMA_PATH
    try:
        schema = json.loads(path.read_text())
        row = schema["properties"]["surfaces"]["items"]
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError) as exc:
        failures.append(f"cannot load gate inventory schema: {exc}")
        return
    if schema.get("additionalProperties") is not False or row.get("additionalProperties") is not False:
        failures.append("gate inventory schema objects must be closed")
    if set(row.get("required", [])) != REQUIRED_FIELDS:
        failures.append("gate inventory schema required fields drift from validator")
    enum_contracts = {
        "source_kind": SOURCE_KINDS,
        "command_role": COMMAND_ROLES,
        "disposition": DISPOSITIONS,
        "enforcement": ENFORCEMENTS,
        "duration_class": DURATION_CLASSES,
        "environment": ENVIRONMENTS,
        "artifact_kind": ARTIFACT_KINDS,
        "artifact_retention": ARTIFACT_RETENTIONS,
    }
    for field, expected in enum_contracts.items():
        actual = set(row.get("properties", {}).get(field, {}).get("enum", []))
        if actual != expected:
            failures.append(f"gate inventory schema enum {field} drifts from validator")


def _enum(
    record_id: str,
    record: Mapping[str, Any],
    field: str,
    allowed: set[str],
    failures: list[str],
) -> None:
    if record.get(field) not in allowed:
        failures.append(f"{record_id}: {field} must be one of {sorted(allowed)}")


def _finish(
    root: Path,
    failures: list[str],
    callback: Callable[[Path, str], None] | None,
) -> list[str]:
    if callback is not None:
        for failure in failures:
            callback(root / INVENTORY_PATH, failure)
    return failures
