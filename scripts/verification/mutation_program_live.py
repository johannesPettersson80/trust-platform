"""Live inputs and provenance for the Phase 10 focused mutation program."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping

from .focused_mutation_artifact import (
    SCHEMA_PATH as FOCUSED_ARTIFACT_SCHEMA_PATH,
    canonical_json,
    validate_execution_artifact,
)
from .mutation_execution import discover_mutants
from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .metadata_validator.mutation_shards import (
    MutationContractError,
    load_mutation_contract,
    validate_mutation_report,
)
from .mutation_program_contract import (
    MUTATION_PROGRAM_PATH,
    MUTATION_PROGRAM_SCHEMA_PATH,
    REQUIRED_SHARD_IDS,
    load_mutation_program,
    validate_mutation_program_contract,
)
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest
from .test_catalog_scanner import scan_repository


REPORT_SCHEMA_PATH = "verification/schemas/mutation-survivor-report.schema.json"
PILOT_REPORT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-08/"
    "p1b-bytecode-validator-mutation-report.json"
)
BOARD_PATH = "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
POLICY_PATH = "docs/internal/testing/checklists/plc-verification-program/policy.md"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
TIMESTAMP_RE = re.compile(
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})$"
)
REQUIRED_OPEN_ROWS = (
    "VERIF-P8-005",
    "VERIF-P8-006",
)
REQUIRED_OPEN_POLICY_ROWS = ("VERIF-STOP-014",)
REPORT_CONTRACT_PATHS = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    POLICY_PATH,
    "docs/internal/testing/checklists/plc-verification-program/README.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-model.md",
    "docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md",
    "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
    "docs/internal/testing/checklists/plc-verification-program/verification-areas.md",
    "scripts/report_mutation_program.py",
    "scripts/run_focused_mutation_shard.py",
    "scripts/validate_mutation_program_report.py",
    FOCUSED_ARTIFACT_SCHEMA_PATH,
    MUTATION_PROGRAM_PATH,
    MUTATION_PROGRAM_SCHEMA_PATH,
    REPORT_SCHEMA_PATH,
    PILOT_REPORT_PATH,
    "verification/README.md",
    "verification/test-catalog.toml",
}
PILOT_TEST_ID = "TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"
TOOL_VERSION = "cargo-mutants 27.0.0"


@dataclass(frozen=True)
class LiveMutationProgramState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    tool_version: str
    shards: tuple[dict[str, Any], ...]
    survivors: tuple[dict[str, Any], ...]
    coverage: dict[str, Any]
    summary: dict[str, int]


def build_live_mutation_program_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = True,
) -> LiveMutationProgramState:
    """Build the six-shard state without executing a mutation or coverage run."""

    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("full metadata validation requires the repository root")
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "metadata validation failed: "
            + "; ".join(f"{item.path}: {item.message}" for item in validator.failures)
        )
    program = load_mutation_program(root)
    failures = validate_mutation_program_contract(root, program)
    if failures:
        raise ValueError("; ".join(failures))

    tool_version = _command_text(root, ("cargo", "mutants", "--version"))
    if tool_version != TOOL_VERSION:
        raise ValueError(
            f"installed mutation tool must be {TOOL_VERSION!r}, found {tool_version!r}"
        )
    scan = scan_repository(root, timestamp="1970-01-01T00:00:00Z")
    scan_errors = [item for item in scan.diagnostics if item.severity == "error"]
    if scan_errors:
        raise ValueError(
            "test scanner failed: " + "; ".join(item.message for item in scan_errors)
        )
    facts = {fact.stable_id: fact for fact in scan.inferred_facts}
    if len(facts) != len(scan.inferred_facts):
        raise ValueError("current test scanner contains duplicate discovery IDs")
    _validate_test_joins(program, facts)
    generated_names = _resolve_selectors(root, program)
    pilot_report, pilot_digest = _load_pilot_report(root)
    shards = _build_shard_rows(root, program, generated_names, pilot_report, pilot_digest)
    summary = _summarize(shards)
    coverage = {
        "runs": 0,
        "line_percent": None,
        "branch_percent": None,
        "posture": "adequacy_signal_not_release_safety_proof",
    }

    board = (root / BOARD_PATH).read_text()
    policy = (root / POLICY_PATH).read_text()
    failures = validate_open_board_rows(board)
    failures.extend(validate_open_policy_rows(policy))
    if failures:
        raise ValueError("; ".join(failures))
    source_paths = {
        mutation["source_file"]
        for shard in _program_shards(program)
        for mutation in _mutation_rows(shard)
        if isinstance(mutation.get("source_file"), str)
    }
    invariant_paths = {
        path.relative_to(root).as_posix()
        for path in (root / "verification/invariants").glob("*/*.toml")
        if path.is_file() or path.is_symlink()
    }
    input_paths = tuple(
        sorted(
            set(REPORT_CONTRACT_PATHS)
            | validator_code_input_paths(root)
            | set(scan.provenance.input_paths)
            | source_paths
            | invariant_paths
            | _measured_artifact_paths(program)
            | _survivor_resolution_paths(program)
            | {"verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml"}
        )
    )
    failures = validate_bound_input_paths(root, input_paths)
    if failures:
        raise ValueError("; ".join(failures))
    commit = _head_commit(root)
    if require_clean_commit:
        dirty = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
            check=False,
            capture_output=True,
        )
        if dirty.returncode != 0 or dirty.stdout:
            raise ValueError("source commit must identify a clean full Git SHA")
    report_timestamp = timestamp or datetime.now(timezone.utc).isoformat(timespec="seconds")
    validate_timestamp(report_timestamp)
    return LiveMutationProgramState(
        commit=commit,
        timestamp=report_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=input_paths,
        input_digest=input_digest(root, list(input_paths)),
        tool_version=tool_version,
        shards=tuple(shards),
        survivors=tuple(dict(item) for item in program.get("survivor_resolutions", [])),
        coverage=coverage,
        summary=summary,
    )


def validate_open_board_rows(board: str) -> list[str]:
    failures = []
    for row_id in REQUIRED_OPEN_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", board, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 10 mutation audit")
    return failures


def validate_open_policy_rows(policy: str) -> list[str]:
    failures = []
    for row_id in REQUIRED_OPEN_POLICY_ROWS:
        if not re.search(rf"^- \[ \] `{re.escape(row_id)}`(?:\s|$)", policy, re.MULTILINE):
            failures.append(f"{row_id} must remain open for the Phase 10 mutation audit")
    return failures


def validate_timestamp(value: object) -> None:
    if not isinstance(value, str) or not TIMESTAMP_RE.fullmatch(value):
        raise ValueError("timestamp must be ISO-8601 with a timezone")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError("timestamp must be ISO-8601 with a timezone") from exc
    if parsed.tzinfo is None:
        raise ValueError("timestamp must be ISO-8601 with a timezone")


def validate_source_revision(
    root: Path,
    commit: object,
    input_paths: tuple[str, ...],
) -> list[str]:
    root = root.resolve()
    failures = validate_bound_input_paths(root, input_paths)
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        return sorted(set([*failures, "commit must identify a clean full Git SHA"]))
    resolved = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if resolved.returncode != 0:
        return [f"commit does not resolve in repository: {commit}"]
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "-z", commit],
        check=False,
        capture_output=True,
    )
    if tree.returncode != 0:
        return [f"could not inspect source commit: {commit}"]
    tree_paths = {item.decode() for item in tree.stdout.split(b"\0") if item}
    missing = sorted(set(input_paths) - tree_paths)
    if missing:
        failures.append("source commit lacks report inputs: " + ", ".join(missing[:8]))
    changed = subprocess.run(
        ["git", "-C", str(root), "diff", "--quiet", commit, "--", *input_paths],
        check=False,
        capture_output=True,
    )
    if changed.returncode == 1:
        failures.append("report inputs differ from the claimed source commit")
    elif changed.returncode != 0:
        failures.append(f"could not compare report inputs with source commit: exit {changed.returncode}")
    return sorted(set(failures))


def _validate_test_joins(program: Mapping[str, Any], facts: Mapping[str, Any]) -> None:
    failures: list[str] = []
    pilot_contract = load_mutation_contract(PILOT_TEST_ID, root=METADATA_ROOT)
    case_ids = set(pilot_contract.case_ids)
    for shard in _program_shards(program):
        for index, binding in enumerate(_associated_tests(shard)):
            label = f"{shard.get('id', '<unknown>')} associated_tests[{index}]"
            if binding.get("id_kind") == "committed_case_id":
                if binding.get("id") not in case_ids:
                    failures.append(f"{label} names an unknown committed pilot case ID")
                if (
                    binding.get("path") != pilot_contract.case_file
                    or binding.get("source_kind") != "case_table"
                    or binding.get("name") != binding.get("id")
                    or binding.get("ignore_state") != "not_applicable"
                ):
                    failures.append(f"{label} committed case binding drifted")
                continue
            if binding.get("id_kind") != "scanner_discovery_id":
                failures.append(f"{label} has an unsupported id_kind")
                continue
            discovery_id = binding.get("id")
            fact = facts.get(discovery_id)
            if fact is None:
                failures.append(f"{label} discovery_id is absent from current scanner facts")
                continue
            for field, actual in (
                ("path", fact.path),
                ("name", fact.name),
                ("source_kind", fact.source_kind),
                ("ignore_state", fact.ignore_state),
            ):
                if binding.get(field) != actual:
                    failures.append(f"{label} {field} does not match current scanner fact")
            if fact.ignore_state != "not_ignored":
                failures.append(f"{label} must resolve to a not_ignored scanner fact")
    if failures:
        raise ValueError("; ".join(failures))


def _resolve_selectors(root: Path, program: Mapping[str, Any]) -> dict[str, str]:
    generated_by_file: dict[str, list[dict[str, Any]]] = {}
    resolved: dict[str, str] = {}
    environment = os.environ.copy()
    for shard in _program_shards(program):
        for mutation in _mutation_rows(shard):
            source_file = mutation.get("source_file")
            mutation_id = mutation.get("id")
            if not isinstance(source_file, str) or not isinstance(mutation_id, str):
                raise ValueError("mutation selectors require source_file and id strings")
            candidates = generated_by_file.get(source_file)
            if candidates is None:
                candidates = discover_mutants(root / source_file, root, environment)
                generated_by_file[source_file] = candidates
            matches = [
                candidate
                for candidate in candidates
                if candidate.get("name") == mutation.get("selector_name")
                and candidate.get("function", {}).get("function_name")
                == mutation.get("function")
                and candidate.get("genre") == mutation.get("genre")
                and candidate.get("replacement") == mutation.get("replacement")
            ]
            if len(matches) != 1:
                raise ValueError(
                    f"mutation selector for {mutation_id} found {len(matches)} generated mutants"
                )
            selected = matches[0]
            name = selected.get("name")
            if not isinstance(name, str) or not name:
                raise ValueError(f"{mutation_id} generated mutant name is missing")
            resolved[mutation_id] = name
    return resolved


def _load_pilot_report(root: Path) -> tuple[dict[str, Any], str]:
    path = root / PILOT_REPORT_PATH
    try:
        report = json.loads(path.read_text())
        contract = load_mutation_contract(PILOT_TEST_ID, root=root)
    except (OSError, json.JSONDecodeError, MutationContractError) as exc:
        raise ValueError(f"bytecode mutation pilot cannot be loaded: {exc}") from exc
    failures = validate_mutation_report(report, contract)
    if failures:
        raise ValueError("bytecode mutation pilot is invalid: " + "; ".join(failures))
    digest = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
    return report, digest


def _build_shard_rows(
    root: Path,
    program: Mapping[str, Any],
    generated_names: Mapping[str, str],
    pilot_report: Mapping[str, Any],
    pilot_digest: str,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    shards = _program_shards(program)
    if [row.get("id") for row in shards] != list(REQUIRED_SHARD_IDS):
        raise ValueError("mutation program shard order drifted")
    for index, shard in enumerate(shards):
        mutations = []
        for mutation in _mutation_rows(shard):
            mutations.append(
                {
                    "id": mutation["id"],
                    "source_file": mutation["source_file"],
                    "source_digest": mutation["source_digest"],
                    "function": mutation["function"],
                    "genre": mutation["genre"],
                    "replacement": mutation["replacement"],
                    "generated_mutant_name": generated_names[mutation["id"]],
                    "build_command": list(mutation.get("build_command", [])),
                    "test_command": list(mutation.get("test_command", [])),
                    "association_ids": list(mutation.get("association_ids", [])),
                }
            )
        results = []
        artifact = None
        if index == 0:
            if shard.get("legacy_report_path") != PILOT_REPORT_PATH:
                raise ValueError("bytecode shard legacy report path drifted")
            results = [_normalize_pilot_result(item) for item in pilot_report["mutations"]]
            artifact = {"path": PILOT_REPORT_PATH, "sha256": pilot_digest}
            if [item["id"] for item in results] != [item["id"] for item in mutations]:
                raise ValueError("bytecode pilot outcomes do not match configured program mutants")
        elif index < 5 and shard.get("execution_status") == "measured":
            payload, digest = _load_focused_artifact(root, shard)
            results = [
                _normalize_focused_result(item)
                for item in _artifact_mutations(payload)
            ]
            artifact = {"path": shard["result_artifact_path"], "sha256": digest}
            if [item["id"] for item in results] != [item["id"] for item in mutations]:
                raise ValueError(
                    f"{shard['id']} focused outcomes do not match configured program mutants"
                )
        rows.append(
            {
                "id": shard["id"],
                "title": shard.get("title", shard["id"]),
                "area": shard.get("area"),
                "invariant_ids": list(shard.get("invariant_ids", [])),
                "association_semantics": shard.get("association_semantics"),
                "owner": shard.get("owner"),
                "execution_status": shard["execution_status"],
                "delivered_build_requirement": shard.get("delivered_build_requirement"),
                "delivered_binary_path": shard.get("delivered_binary_path"),
                "delivered_confirmation_requirements": list(
                    shard.get("delivered_confirmation", [])
                ),
                "delivered_build_confirmation": None,
                "associated_tests": [dict(item) for item in _associated_tests(shard)],
                "mutations": mutations,
                "result_artifact": artifact,
                "results": results,
            }
        )
    return rows


def _normalize_pilot_result(value: Mapping[str, Any]) -> dict[str, Any]:
    """Adapt the bytecode-only report without leaking its case-specific fields."""

    fields = (
        "id",
        "source_file",
        "function",
        "genre",
        "replacement",
        "generated_mutant_name",
        "build_command",
        "build_exit_status",
        "build_output_tail",
        "build_timed_out",
        "test_command",
        "test_exit_status",
        "test_output_tail",
        "test_timed_out",
        "duration_seconds",
        "result",
    )
    result = {field: value.get(field) for field in fields}
    result["build_command"] = list(value.get("build_command", []))
    result["test_command"] = list(value.get("test_command", []))
    result["association_ids"] = list(value.get("related_case_ids", []))
    return result


def _normalize_focused_result(value: Mapping[str, Any]) -> dict[str, Any]:
    """Adapt a source-runner artifact to the closed generic report row."""

    fields = (
        "id",
        "source_file",
        "function",
        "genre",
        "replacement",
        "generated_mutant_name",
        "build_exit_status",
        "build_timed_out",
        "test_exit_status",
        "test_timed_out",
        "duration_seconds",
        "result",
    )
    result = {field: value.get(field) for field in fields}
    result["build_command"] = list(value.get("build_command", []))
    result["test_command"] = list(value.get("test_command", []))
    result["build_output_tail"] = _output_tail(
        value.get("build_stdout"), value.get("build_stderr")
    )
    result["test_output_tail"] = _output_tail(
        value.get("test_stdout"), value.get("test_stderr")
    )
    result["association_ids"] = list(value.get("association_ids", []))
    return result


def _output_tail(stdout: Any, stderr: Any) -> str:
    output = "\n".join(
        item for item in (stdout, stderr) if isinstance(item, str) and item
    )
    return output[-4000:]


def _load_focused_artifact(
    root: Path, shard: Mapping[str, Any]
) -> tuple[dict[str, Any], str]:
    relative = shard.get("result_artifact_path")
    if not isinstance(relative, str):
        raise ValueError(f"{shard.get('id')} measured shard lacks result_artifact_path")
    path = root / relative
    try:
        raw = path.read_bytes()
        payload = json.loads(raw)
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"focused mutation artifact cannot be loaded at {relative}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"focused mutation artifact at {relative} must be an object")
    if canonical_json(payload).encode() != raw:
        raise ValueError(f"focused mutation artifact at {relative} must use canonical JSON")
    failures = validate_execution_artifact(root, payload, shard)
    if failures:
        raise ValueError(
            f"focused mutation artifact at {relative} is invalid: " + "; ".join(failures)
        )
    digest = "sha256:" + hashlib.sha256(raw).hexdigest()
    return payload, digest


def _artifact_mutations(payload: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    value = payload.get("mutations")
    if not isinstance(value, list) or not all(isinstance(item, Mapping) for item in value):
        raise ValueError("focused mutation artifact mutations must be an object array")
    return value


def _summarize(shards: list[dict[str, Any]]) -> dict[str, int]:
    results = [result for shard in shards for result in shard["results"]]
    return {
        "shards": len(shards),
        "measured_shards": sum(row["execution_status"] == "measured" for row in shards),
        "planned_shards": sum(row["execution_status"] == "planned" for row in shards),
        "defined_mutants": sum(len(row["mutations"]) for row in shards),
        "measured_mutants": len(results),
        "caught": sum(row.get("result") == "caught" for row in results),
        "survived": sum(row.get("result") == "survived" for row in results),
        "unviable": sum(row.get("result") == "unviable" for row in results),
        "timeout": sum(row.get("result") == "timeout" for row in results),
        "error": sum(row.get("result") == "error" for row in results),
    }


def _program_shards(program: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    value = program.get("shards")
    if not isinstance(value, list) or not all(isinstance(item, Mapping) for item in value):
        raise ValueError("mutation program shards must be an object array")
    return value


def _mutation_rows(shard: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    value = shard.get("mutations")
    if not isinstance(value, list) or not all(isinstance(item, Mapping) for item in value):
        raise ValueError("mutation program mutations must be an object array")
    return value


def _associated_tests(shard: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    value = shard.get("associated_tests")
    if not isinstance(value, list) or not all(isinstance(item, Mapping) for item in value):
        raise ValueError("mutation program associated_tests must be an object array")
    return value


def _survivor_resolution_paths(program: Mapping[str, Any]) -> set[str]:
    value = program.get("survivor_resolutions")
    if not isinstance(value, list):
        return set()
    return {
        item["resolution_ref"]
        for item in value
        if isinstance(item, Mapping)
        and isinstance(item.get("resolution_ref"), str)
        and item["resolution_ref"]
    }


def _measured_artifact_paths(program: Mapping[str, Any]) -> set[str]:
    return {
        shard["result_artifact_path"]
        for shard in _program_shards(program)
        if shard.get("execution_status") == "measured"
        and isinstance(shard.get("result_artifact_path"), str)
    }


def _command_text(root: Path, command: tuple[str, ...]) -> str:
    result = subprocess.run(command, cwd=root, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        raise ValueError(f"command failed: {list(command)}")
    return result.stdout.strip()


def _head_commit(root: Path) -> str:
    commit = _command_text(root, ("git", "rev-parse", "HEAD"))
    if not COMMIT_RE.fullmatch(commit):
        raise ValueError("source commit must identify a clean full Git SHA")
    return commit
