"""Catalog contracts for focused bytecode-validator mutation shards."""

from __future__ import annotations

import hashlib
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .constants import ROOT


CASE_SEMANTICS = "association_only_case_ids_not_executed_by_mutation_runner"
SOURCE_PREFIX = "crates/trust-runtime/src/bytecode/validate/"


class MutationContractError(RuntimeError):
    pass


@dataclass(frozen=True)
class MutationSpec:
    id: str
    source_file: str
    function: str
    genre: str
    replacement: str
    build_command: tuple[str, ...]
    test_command: tuple[str, ...]
    related_case_ids: tuple[str, ...]
    survivor_action: str


@dataclass(frozen=True)
class MutationContract:
    record: dict[str, Any]
    test_id: str
    shard_id: str
    runner: str
    tool: str
    tool_version: str
    case_file: str
    case_file_digest: str
    case_ids: tuple[str, ...]
    case_semantics: str
    mutations: tuple[MutationSpec, ...]
    out_of_scope_case_ids: tuple[str, ...]
    out_of_scope_reason: str


def sha256_file(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load_mutation_contract(test_id: str, *, root: Path = ROOT) -> MutationContract:
    catalog_path = root / "verification/test-catalog.toml"
    try:
        catalog = tomllib.loads(catalog_path.read_text())
    except Exception as exc:
        raise MutationContractError(f"failed to load mutation catalog: {exc}") from exc
    matches = [record for record in catalog.get("tests", []) if record.get("id") == test_id]
    if len(matches) != 1:
        raise MutationContractError(f"expected one catalog record for {test_id}, found {len(matches)}")
    return mutation_contract_from_record(matches[0], root=root)


def mutation_contract_from_record(record: dict[str, Any], *, root: Path = ROOT) -> MutationContract:
    failures = validate_mutation_test_record(record, root=root)
    if failures:
        raise MutationContractError("; ".join(failures))
    case_data = load_case_data(record, root=root)
    return MutationContract(
        record=record,
        test_id=record["id"],
        shard_id=record["mutation_shard_id"],
        runner=record["mutation_runner"],
        tool=record["mutation_tool"],
        tool_version=record["mutation_tool_version"],
        case_file=record["case_file"],
        case_file_digest=record["case_file_digest"],
        case_ids=tuple(case["id"] for case in case_data["case"]),
        case_semantics=record["mutation_case_semantics"],
        mutations=tuple(mutation_spec(item) for item in record["mutations"]),
        out_of_scope_case_ids=tuple(record["mutation_out_of_scope_case_ids"]),
        out_of_scope_reason=record["mutation_out_of_scope_reason"],
    )


def validate_mutation_test_record(record: dict[str, Any], *, root: Path = ROOT) -> list[str]:
    failures: list[str] = []
    test_id = record.get("id", "<unknown>")
    required = [
        "mutation_shard_id",
        "mutation_runner",
        "mutation_tool",
        "mutation_tool_version",
        "mutation_case_semantics",
        "mutation_out_of_scope_case_ids",
        "mutation_out_of_scope_reason",
        "mutations",
        "case_file",
        "case_file_digest",
    ]
    if record.get("test_class") != "mutation":
        failures.append(f"{test_id} mutation shard must use test_class = mutation")
    for field in required:
        if field not in record:
            failures.append(f"{test_id} mutation shard missing {field}")
    if failures:
        return failures
    if record.get("status") != "mapped":
        failures.append(f"{test_id} mutation shard must use status = mapped")
    if record.get("mutation_case_semantics") != CASE_SEMANTICS:
        failures.append(f"{test_id} mutation shard must use {CASE_SEMANTICS!r} case semantics")
    for field in ("mutation_shard_id", "mutation_runner", "mutation_tool", "mutation_tool_version"):
        if not isinstance(record.get(field), str) or not record[field]:
            failures.append(f"{test_id} {field} must be non-empty text")

    try:
        case_data = load_case_data(record, root=root)
    except MutationContractError as exc:
        failures.append(str(exc))
        return failures
    case_rows = case_data.get("case")
    if not isinstance(case_rows, list) or not case_rows:
        failures.append(f"{test_id} mutation case file has no cases")
        return failures
    case_ids = [case.get("id") for case in case_rows if isinstance(case, dict)]
    if len(case_ids) != len(case_rows) or not all(isinstance(case_id, str) and case_id for case_id in case_ids):
        failures.append(f"{test_id} mutation case file contains a missing/invalid case ID")
        return failures
    known_case_ids = set(case_ids)
    if len(known_case_ids) != len(case_ids):
        failures.append(f"{test_id} mutation case file duplicates case IDs")
        return failures
    for case in case_rows:
        if not isinstance(case, dict):
            failures.append(f"{test_id} mutation association requires object case rows")
            break
        blocked = case.get("state") == "blocked" and "expect" not in case
        runnable = isinstance(case.get("expect"), dict) and case.get("state") != "blocked"
        if blocked == runnable:
            failures.append(
                f"{test_id} mutation association requires every committed case to be blocked or runnable"
            )
            break

    mutations = record.get("mutations")
    if not isinstance(mutations, list) or not mutations:
        failures.append(f"{test_id} mutation shard must define at least one mutation")
        return failures
    seen_mutants: set[str] = set()
    mapped_cases: list[str] = []
    for mutation in mutations:
        if not isinstance(mutation, dict):
            failures.append(f"{test_id} mutation entry is not a table")
            continue
        mutation_id = mutation.get("id")
        if not isinstance(mutation_id, str) or not mutation_id:
            failures.append(f"{test_id} mutation has missing/invalid id")
            continue
        if mutation_id in seen_mutants:
            failures.append(f"{test_id} duplicates mutation ID {mutation_id}")
        seen_mutants.add(mutation_id)
        for field in (
            "source_file",
            "function",
            "genre",
            "replacement",
            "build_command",
            "test_command",
            "related_case_ids",
            "survivor_action",
        ):
            if field not in mutation:
                failures.append(f"{mutation_id} missing {field}")
        source_file = mutation.get("source_file")
        if not isinstance(source_file, str) or not source_file.startswith(SOURCE_PREFIX):
            failures.append(f"{mutation_id} source_file must stay under {SOURCE_PREFIX}")
        else:
            source_path = safe_workspace_path(root, source_file)
            validator_dir = (root / SOURCE_PREFIX).resolve()
            try:
                if source_path is None:
                    raise ValueError
                source_path.relative_to(validator_dir)
            except ValueError:
                failures.append(f"{mutation_id} source_file must resolve inside the validator directory")
            else:
                if not source_path.is_file():
                    failures.append(f"{mutation_id} source_file does not exist: {source_file}")
        for field in ("function", "genre", "replacement", "survivor_action"):
            if not isinstance(mutation.get(field), str) or not mutation[field]:
                failures.append(f"{mutation_id} {field} must be non-empty text")
        for field in ("build_command", "test_command"):
            command = mutation.get(field)
            if not isinstance(command, list) or not command or not all(isinstance(arg, str) and arg for arg in command):
                failures.append(f"{mutation_id} {field} must be a non-empty string array")
            elif command[0] != "cargo":
                failures.append(f"{mutation_id} {field} must invoke cargo directly")
        related = mutation.get("related_case_ids")
        if not isinstance(related, list) or not related:
            failures.append(f"{mutation_id} must name related_case_ids")
        elif not all(isinstance(case_id, str) and case_id for case_id in related):
            failures.append(f"{mutation_id} related_case_ids must contain only non-empty strings")
        else:
            if len(related) != len(set(related)):
                failures.append(f"{mutation_id} duplicates related case IDs")
            for case_id in related:
                if case_id not in known_case_ids:
                    failures.append(f"{mutation_id} references unknown case ID {case_id}")
                mapped_cases.append(case_id)

    out_of_scope = record.get("mutation_out_of_scope_case_ids")
    if not isinstance(out_of_scope, list):
        failures.append(f"{test_id} mutation_out_of_scope_case_ids must be a list")
        out_of_scope_strings: list[str] = []
    elif not all(isinstance(case_id, str) and case_id for case_id in out_of_scope):
        failures.append(f"{test_id} mutation_out_of_scope_case_ids must contain only non-empty strings")
        out_of_scope_strings = []
    else:
        out_of_scope_strings = out_of_scope
    if len(out_of_scope_strings) != len(set(out_of_scope_strings)):
        failures.append(f"{test_id} duplicates out-of-scope case IDs")
    for case_id in out_of_scope_strings:
        if case_id not in known_case_ids:
            failures.append(f"{test_id} references unknown out-of-scope case ID {case_id}")
    if len(mapped_cases) != len(set(mapped_cases)):
        failures.append(f"{test_id} maps a committed case ID to more than one mutant")
    mapped_set = set(mapped_cases)
    out_set = set(out_of_scope_strings)
    if mapped_set & out_set:
        failures.append(f"{test_id} maps case IDs as both measured and out of scope")
    accounted = mapped_set | out_set
    if accounted != known_case_ids:
        missing = sorted(known_case_ids - accounted)
        extra = sorted(accounted - known_case_ids)
        failures.append(f"{test_id} does not account for committed case IDs; missing={missing}, extra={extra}")
    if not isinstance(record.get("mutation_out_of_scope_reason"), str) or not record.get(
        "mutation_out_of_scope_reason"
    ):
        failures.append(f"{test_id} must explain out-of-scope case IDs")
    return failures


def load_case_data(record: dict[str, Any], *, root: Path) -> dict[str, Any]:
    case_file = record.get("case_file")
    if not isinstance(case_file, str):
        raise MutationContractError(f"{record.get('id')} mutation shard has invalid case_file")
    case_path = safe_workspace_path(root, case_file)
    if case_path is None or not case_path.is_file():
        raise MutationContractError(f"{record.get('id')} mutation case file does not exist: {case_file}")
    actual_digest = sha256_file(case_path)
    if record.get("case_file_digest") != actual_digest:
        raise MutationContractError(
            f"{record.get('id')} mutation case_file_digest mismatch: expected {actual_digest}, actual {record.get('case_file_digest')}"
        )
    try:
        return tomllib.loads(case_path.read_text())
    except Exception as exc:
        raise MutationContractError(f"failed to parse mutation case file {case_file}: {exc}") from exc


def mutation_spec(record: dict[str, Any]) -> MutationSpec:
    return MutationSpec(
        id=record["id"],
        source_file=record["source_file"],
        function=record["function"],
        genre=record["genre"],
        replacement=record["replacement"],
        build_command=tuple(record["build_command"]),
        test_command=tuple(record["test_command"]),
        related_case_ids=tuple(record["related_case_ids"]),
        survivor_action=record["survivor_action"],
    )


def safe_workspace_path(root: Path, value: Any) -> Path | None:
    if not isinstance(value, str):
        return None
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        return None
    resolved_root = root.resolve()
    resolved = (resolved_root / relative).resolve()
    try:
        resolved.relative_to(resolved_root)
    except ValueError:
        return None
    return resolved
