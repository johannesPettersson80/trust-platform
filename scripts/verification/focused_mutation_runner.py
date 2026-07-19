"""Execute one reviewed source-only mutation shard in an isolated Git archive."""

from __future__ import annotations

import argparse
import os
import platform
import subprocess
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from .focused_mutation_artifact import (
    BOUNDARIES,
    RUNNER,
    TOOL,
    TOOL_VERSION,
    artifact_contract_digest,
    canonical_json,
    validate_execution_artifact,
)
from .metadata_validator.constants import ROOT
from .metadata_validator.mutation_contracts import MutationContractError
from .mutation_execution import (
    CommandResult,
    apply_generated_mutant,
    archived_head,
    classify_focused_mutant,
    clean_mutation_package,
    command_summary,
    discover_mutants,
    package_from_build_command,
    run_command,
    select_generated_mutant,
)
from .mutation_program_contract import (
    REQUIRED_SHARD_IDS,
    load_mutation_program,
    validate_mutation_program_contract,
)


SOURCE_SHARD_IDS = frozenset(REQUIRED_SHARD_IDS[1:5])


def select_source_shard(program: Mapping[str, Any], shard_id: str) -> dict[str, Any]:
    shards = program.get("shards")
    if not isinstance(shards, list):
        raise ValueError("mutation program shards must be an array")
    selected = [row for row in shards if isinstance(row, Mapping) and row.get("id") == shard_id]
    if not selected:
        raise ValueError(f"unknown mutation shard {shard_id}")
    if len(selected) != 1:
        raise ValueError(f"mutation shard {shard_id} is duplicated")
    if shard_id not in SOURCE_SHARD_IDS:
        if selected[0].get("delivered_build_requirement") == "required_before_execution":
            raise ValueError(f"delivered-build shard {shard_id} requires its dedicated runner")
        raise ValueError(f"mutation shard {shard_id} is outside the reviewed source-runner set")
    return dict(selected[0])


def execute_source_shard(
    *,
    root: Path,
    shard: Mapping[str, Any],
    target_dir: Path,
    build_timeout: float,
    test_timeout: float,
) -> dict[str, Any]:
    _require_clean_commit(root)
    _require_sources_match_head(root, shard)
    tool_version = _command_output(root, ["cargo", "mutants", "--version"])
    if tool_version != TOOL_VERSION:
        raise MutationContractError(
            f"installed cargo-mutants version {tool_version!r} does not match {TOOL_VERSION!r}"
        )
    mutations = _mapping_rows(shard.get("mutations"), "mutations")
    associated = _mapping_rows(shard.get("associated_tests"), "associated_tests")
    if len(associated) != 1 or not isinstance(associated[0].get("name"), str):
        raise MutationContractError("focused source shard must bind exactly one selected test")
    selected_test_name = associated[0]["name"]
    packages = {package_from_build_command(row["build_command"]) for row in mutations}
    if len(packages) != 1:
        raise MutationContractError("focused source shard must build exactly one package")
    package = next(iter(packages))
    target_dir.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target_dir.resolve())

    started_at = _timestamp()
    source_commit = _command_output(root, ["git", "rev-parse", "HEAD"])
    with archived_head(root, prefix="trust-focused-mutation-") as scratch:
        clean_mutation_package(package, scratch, environment, build_timeout)
        baselines: list[dict[str, Any]] = []
        seen: set[tuple[str, ...]] = set()
        for mutation in mutations:
            command = tuple(mutation["test_command"])
            if command in seen:
                continue
            seen.add(command)
            baseline = run_command(command, cwd=scratch, env=environment, timeout=test_timeout)
            baselines.append(command_summary(baseline))
            if baseline.timed_out or baseline.returncode != 0:
                raise MutationContractError(
                    f"baseline command failed for {mutation['id']}: {list(command)}"
                )

        generated_by_file: dict[str, list[dict[str, Any]]] = {}
        outcomes: list[dict[str, Any]] = []
        for mutation in mutations:
            clean_mutation_package(package, scratch, environment, build_timeout)
            source_path = scratch / mutation["source_file"]
            original = source_path.read_text()
            candidates = generated_by_file.get(mutation["source_file"])
            if candidates is None:
                candidates = discover_mutants(source_path, scratch, environment)
                generated_by_file[mutation["source_file"]] = candidates
            candidate = select_generated_mutant(candidates, mutation)
            source_path.write_text(apply_generated_mutant(original, candidate))
            try:
                build = run_command(
                    mutation["build_command"],
                    cwd=scratch,
                    env=environment,
                    timeout=build_timeout,
                )
                test = None
                if not build.timed_out and build.returncode == 0:
                    test = run_command(
                        mutation["test_command"],
                        cwd=scratch,
                        env=environment,
                        timeout=test_timeout,
                    )
                result = classify_focused_mutant(
                    source_file=mutation["source_file"],
                    selected_test_name=selected_test_name,
                    build=build,
                    test=test,
                )
                outcomes.append(_outcome_record(mutation, candidate, build, test, result))
                if result == "error":
                    raise MutationContractError(
                        f"unclassified or infrastructure failure while executing {mutation['id']}"
                    )
            finally:
                source_path.write_text(original)
                clean_mutation_package(package, scratch, environment, build_timeout)

    summary = {result: 0 for result in ("caught", "survived", "unviable", "timeout", "error")}
    for outcome in outcomes:
        summary[outcome["result"]] += 1
    payload = {
        "schema_version": 1,
        "id": f"MUTATION_EXECUTION_{str(shard['id']).removeprefix('MUTATION_SHARD_')}",
        "status": "complete",
        "runner": RUNNER,
        "tool": TOOL,
        "tool_version": tool_version,
        "source_commit": source_commit,
        "platform": f"{platform.system().lower()}-{platform.machine()}",
        "started_at": started_at,
        "finished_at": _timestamp(),
        "shard_id": shard["id"],
        "contract_digest": artifact_contract_digest(shard),
        "baseline_commands": baselines,
        "mutations": outcomes,
        "summary": {"total": len(outcomes), **summary},
        "boundaries": BOUNDARIES,
    }
    failures = validate_execution_artifact(root, payload, shard)
    if failures:
        raise MutationContractError("; ".join(failures))
    return payload


def _outcome_record(
    mutation: Mapping[str, Any],
    candidate: Mapping[str, Any],
    build: CommandResult,
    test: CommandResult | None,
    result: str,
) -> dict[str, Any]:
    return {
        "id": mutation["id"],
        "source_file": mutation["source_file"],
        "function": mutation["function"],
        "genre": candidate.get("genre"),
        "replacement": candidate.get("replacement"),
        "generated_mutant_name": candidate.get("name"),
        "build_command": list(build.command),
        "build_exit_status": build.returncode,
        "build_stdout": build.stdout,
        "build_stderr": build.stderr,
        "build_timed_out": build.timed_out,
        "build_duration_seconds": round(build.duration_seconds, 3),
        "test_command": list(test.command) if test else list(mutation["test_command"]),
        "test_exit_status": test.returncode if test else None,
        "test_stdout": test.stdout if test else "",
        "test_stderr": test.stderr if test else "",
        "test_timed_out": test.timed_out if test else False,
        "test_duration_seconds": round(test.duration_seconds, 3) if test else 0.0,
        "duration_seconds": round(
            build.duration_seconds + (test.duration_seconds if test else 0.0), 3
        ),
        "result": result,
        "association_ids": list(mutation["association_ids"]),
    }


def _require_clean_commit(root: Path) -> None:
    status = _command_output(root, ["git", "status", "--porcelain", "--untracked-files=all"])
    if status:
        raise MutationContractError("focused mutation execution requires a clean Git worktree")


def _require_sources_match_head(root: Path, shard: Mapping[str, Any]) -> None:
    for mutation in _mapping_rows(shard.get("mutations"), "mutations"):
        source_file = mutation["source_file"]
        result = subprocess.run(
            ["git", "diff", "--quiet", "HEAD", "--", source_file], cwd=root, check=False
        )
        if result.returncode != 0:
            raise MutationContractError(f"mutation source differs from HEAD: {source_file}")


def _mapping_rows(value: Any, label: str) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not all(isinstance(item, Mapping) for item in value):
        raise MutationContractError(f"focused mutation shard {label} must be an object array")
    return [dict(item) for item in value]


def _safe_output_path(root: Path, value: Path) -> Path:
    root = root.resolve()
    if value.is_absolute():
        try:
            relative = value.resolve().relative_to(root)
        except ValueError as exc:
            raise MutationContractError("focused mutation output must stay inside the workspace") from exc
    else:
        relative = value
    if "\\" in relative.as_posix() or ".." in PurePosixPath(relative.as_posix()).parts:
        raise MutationContractError("focused mutation output path is unsafe")
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.exists() and candidate.is_symlink():
            raise MutationContractError("focused mutation output path contains a symlink")
    resolved_parent = candidate.parent.resolve()
    try:
        resolved_parent.relative_to(root)
    except ValueError as exc:
        raise MutationContractError("focused mutation output escapes the workspace") from exc
    return candidate


def artifact_output_path(
    root: Path, shard: Mapping[str, Any], requested: Path
) -> Path:
    reserved = shard.get("result_artifact_path")
    if not isinstance(reserved, str) or not reserved:
        raise ValueError("focused source shard lacks a reserved result_artifact_path")
    try:
        expected = _safe_output_path(root, Path(reserved))
        actual = _safe_output_path(root, requested)
    except MutationContractError as exc:
        raise ValueError(
            "focused mutation output must equal the reserved result_artifact_path"
        ) from exc
    if actual != expected:
        raise ValueError(
            "focused mutation output must equal the reserved result_artifact_path"
        )
    return actual


def _write_atomic(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(text)
    temporary.replace(path)


def _command_output(cwd: Path, command: list[str]) -> str:
    completed = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        raise MutationContractError(f"command failed: {command}: {completed.stderr.strip()}")
    return completed.stdout.strip()


def _timestamp() -> str:
    from datetime import UTC, datetime

    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard-id", required=True)
    parser.add_argument("--json-out", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--build-timeout", type=float, default=1800.0)
    parser.add_argument("--test-timeout", type=float, default=900.0)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        program = load_mutation_program(ROOT)
        failures = validate_mutation_program_contract(ROOT, program)
        if failures:
            raise MutationContractError("; ".join(failures))
        shard = select_source_shard(program, args.shard_id)
        output = artifact_output_path(ROOT, shard, args.json_out)
        payload = execute_source_shard(
            root=ROOT,
            shard=shard,
            target_dir=args.target_dir,
            build_timeout=args.build_timeout,
            test_timeout=args.test_timeout,
        )
        _write_atomic(output, canonical_json(payload))
        summary = payload["summary"]
        print(
            f"focused mutation shard {args.shard_id}: "
            f"{summary['caught']} caught, {summary['survived']} survived, "
            f"{summary['unviable']} unviable, {summary['timeout']} timeout"
        )
        return 0
    except (MutationContractError, ValueError) as exc:
        print(f"focused mutation shard failed: {exc}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())


__all__ = [
    "CommandResult",
    "classify_focused_mutant",
    "package_from_build_command",
    "select_source_shard",
]
