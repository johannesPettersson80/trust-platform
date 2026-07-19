"""Closed semantic contract for focused source-mutation execution artifacts."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .mutation_execution import CommandResult, classify_focused_mutant
from .test_catalog_validation import check_supported_schema_keywords
from .test_catalog_json_schema import validate_json_schema_instance


SCHEMA_PATH = "verification/schemas/focused-mutation-execution.schema.json"
EXECUTION_INPUT_PATHS = (
    "scripts/run_focused_mutation_shard.py",
    "scripts/verification/focused_mutation_artifact.py",
    "scripts/verification/focused_mutation_runner.py",
    "scripts/verification/mutation_execution.py",
    SCHEMA_PATH,
)
SCHEMA_SEMANTIC_DIGEST = "841ba9d185173fb56e9da7fe59d6307f4364b16274004f6fdd2b67a96159a02d"
RUNNER = "focused-mutation-shard.py v1"
TOOL = "cargo-mutants-single-file-adapter"
TOOL_VERSION = "cargo-mutants 27.0.0"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
RESULTS = ("caught", "survived", "unviable", "timeout", "error")
BOUNDARIES = {
    "association_ids_are_execution_claims": False,
    "artifact_creates_proof": False,
    "artifact_closes_spec_gaps": False,
    "artifact_promotes_invariants": False,
}
CONTRACT_FIELDS = (
    "id",
    "title",
    "area",
    "invariant_ids",
    "association_semantics",
    "delivered_build_requirement",
    "owner",
    "mutations",
    "associated_tests",
)


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def artifact_contract_digest(shard: Mapping[str, Any]) -> str:
    contract = {field: shard.get(field) for field in CONTRACT_FIELDS}
    encoded = json.dumps(contract, sort_keys=True, separators=(",", ":")).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def validate_artifact_schema(schema: Any) -> list[str]:
    failures: list[str] = []
    if not isinstance(schema, dict):
        return ["focused mutation artifact schema root must be an object"]
    check_supported_schema_keywords(schema, "$", failures)
    semantic = hashlib.sha256(
        json.dumps(schema, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    if semantic != SCHEMA_SEMANTIC_DIGEST:
        failures.append("focused mutation artifact schema semantic digest drifted")
    if schema.get("additionalProperties") is not False:
        failures.append("focused mutation artifact schema root must be closed")
    definitions = schema.get("$defs")
    if not isinstance(definitions, Mapping):
        failures.append("focused mutation artifact schema definitions are missing")
    else:
        for name in ("baseline", "result", "summary", "boundaries"):
            definition = definitions.get(name)
            if not isinstance(definition, Mapping) or definition.get("additionalProperties") is not False:
                failures.append(f"focused mutation artifact schema {name} must be closed")
    return sorted(set(failures))


def validate_execution_artifact(
    root: Path, payload: Any, shard: Mapping[str, Any]
) -> list[str]:
    failures: list[str] = []
    if not isinstance(payload, Mapping):
        return ["focused mutation artifact root must be an object"]
    try:
        schema = json.loads((root / SCHEMA_PATH).read_text())
        failures.extend(validate_artifact_schema(schema))
        failures.extend(validate_json_schema_instance(dict(payload), schema))
    except Exception as exc:
        failures.append(f"focused mutation artifact schema validation failed safely: {exc}")
    if payload.get("runner") != RUNNER or payload.get("tool") != TOOL:
        failures.append("focused mutation artifact runner/tool binding mismatch")
    if payload.get("tool_version") != TOOL_VERSION:
        failures.append("focused mutation artifact tool version mismatch")
    if not COMMIT_RE.fullmatch(str(payload.get("source_commit", ""))):
        failures.append("focused mutation artifact source_commit must be a full Git SHA")
    if payload.get("shard_id") != shard.get("id"):
        failures.append("focused mutation artifact shard binding mismatch")
    if payload.get("contract_digest") != artifact_contract_digest(shard):
        failures.append("focused mutation artifact contract digest mismatch")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("focused mutation artifact honesty boundaries drifted")

    definitions = _mapping_by_id(shard.get("mutations"), "configured mutation", failures)
    outcomes = _mapping_by_id(payload.get("mutations"), "artifact mutation", failures)
    if set(outcomes) != set(definitions):
        failures.append("focused mutation artifact outcome set does not match shard")
    associated = shard.get("associated_tests")
    selected_name = None
    if isinstance(associated, list) and len(associated) == 1 and isinstance(associated[0], Mapping):
        value = associated[0].get("name")
        if isinstance(value, str) and value:
            selected_name = value
    if selected_name is None:
        failures.append("focused mutation shard must bind exactly one selected test name")

    counts = {result: 0 for result in RESULTS}
    for mutation_id, outcome in outcomes.items():
        definition = definitions.get(mutation_id)
        if definition is None:
            continue
        for field in (
            "source_file",
            "function",
            "genre",
            "replacement",
            "build_command",
            "test_command",
            "association_ids",
        ):
            if outcome.get(field) != definition.get(field):
                failures.append(f"{mutation_id} artifact {field} drifts from shard")
        if outcome.get("generated_mutant_name") != definition.get("selector_name"):
            failures.append(f"{mutation_id} artifact generated mutant selector drifted")
        _validate_phase_consistency(outcome, "build", mutation_id, failures)
        _validate_phase_consistency(outcome, "test", mutation_id, failures)
        try:
            build = _command_result(outcome, "build")
            test = _command_result(outcome, "test")
            if build.timed_out or build.returncode != 0:
                if test is not None:
                    failures.append(
                        f"{mutation_id} artifact records a test result after an unsuccessful build"
                    )
            expected_duration = build.duration_seconds + (
                test.duration_seconds if test is not None else 0.0
            )
            declared_duration = outcome.get("duration_seconds")
            if (
                not isinstance(declared_duration, (int, float))
                or isinstance(declared_duration, bool)
                or abs(float(declared_duration) - expected_duration) > 0.002
            ):
                failures.append(f"{mutation_id} artifact duration does not match its phases")
            derived = classify_focused_mutant(
                source_file=str(definition.get("source_file", "")),
                selected_test_name=selected_name or "",
                build=build,
                test=test,
            )
        except (KeyError, TypeError, ValueError) as exc:
            failures.append(f"{mutation_id} artifact outcome is invalid: {type(exc).__name__}")
            continue
        declared = outcome.get("result")
        if declared != derived:
            failures.append(f"{mutation_id} artifact result must derive to {derived}")
        if isinstance(declared, str) and declared in counts:
            counts[declared] += 1

    expected_summary = {"total": len(definitions), **counts}
    if payload.get("summary") != expected_summary:
        failures.append("focused mutation artifact summary mismatch")
    if counts["error"]:
        failures.append("complete focused mutation artifact cannot contain infrastructure errors")
    _validate_baselines(payload.get("baseline_commands"), definitions, failures)
    return sorted(set(failures))


def _validate_phase_consistency(
    outcome: Mapping[str, Any],
    prefix: str,
    mutation_id: str,
    failures: list[str],
) -> None:
    timed_out = outcome.get(f"{prefix}_timed_out")
    exit_status = outcome.get(f"{prefix}_exit_status")
    if timed_out is True and exit_status is not None:
        failures.append(
            f"{mutation_id} artifact {prefix} timeout must have a null exit status"
        )
    if prefix == "build" and timed_out is False and not isinstance(exit_status, int):
        failures.append(
            f"{mutation_id} artifact non-timeout build must have an integer exit status"
        )
    if prefix == "test" and timed_out is False and exit_status is None:
        if any(
            (
                outcome.get("test_stdout") != "",
                outcome.get("test_stderr") != "",
                outcome.get("test_duration_seconds") != 0.0,
            )
        ):
            failures.append(
                f"{mutation_id} artifact absent test phase must have empty output and zero duration"
            )


def validate_execution_artifact_source(
    root: Path, payload: Mapping[str, Any], shard: Mapping[str, Any]
) -> list[str]:
    """Bind an artifact to an input-identical clean commit in the current history."""

    failures: list[str] = []
    commit = payload.get("source_commit")
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        return ["focused mutation artifact source_commit must be a full Git SHA"]
    resolved = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--verify", f"{commit}^{{commit}}"],
        check=False,
        capture_output=True,
        text=True,
    )
    if resolved.returncode != 0 or resolved.stdout.strip() != commit:
        return ["focused mutation artifact source_commit does not resolve exactly"]
    ancestry = subprocess.run(
        ["git", "-C", str(root), "merge-base", "--is-ancestor", commit, "HEAD"],
        check=False,
        capture_output=True,
    )
    if ancestry.returncode != 0:
        failures.append("focused mutation artifact source_commit must be an ancestor of HEAD")

    paths = set(EXECUTION_INPUT_PATHS)
    for collection_name in ("mutations", "associated_tests"):
        value = shard.get(collection_name)
        if not isinstance(value, list):
            failures.append(f"focused mutation shard {collection_name} must be an array")
            continue
        field = "source_file" if collection_name == "mutations" else "path"
        for row in value:
            if isinstance(row, Mapping) and isinstance(row.get(field), str):
                paths.add(row[field])
            else:
                failures.append(
                    f"focused mutation shard {collection_name} contains an invalid {field}"
                )

    for path in sorted(paths):
        failures.extend(_validate_source_path(root, commit, path))
    return sorted(set(failures))


def _validate_source_path(root: Path, commit: str, path: str) -> list[str]:
    failures: list[str] = []
    candidate = Path(path)
    if candidate.is_absolute() or "\\" in path or ".." in candidate.parts:
        return [f"focused mutation artifact input path is unsafe: {path}"]
    workspace = root.resolve()
    current = workspace / candidate
    walk = workspace
    for part in candidate.parts:
        walk /= part
        if walk.is_symlink():
            return [f"focused mutation artifact input path contains a symlink: {path}"]
    if not current.is_file():
        return [f"focused mutation artifact input path is not a regular file: {path}"]
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", path],
        check=False,
        capture_output=True,
    )
    if tracked.returncode != 0:
        failures.append(f"focused mutation artifact input path is not tracked: {path}")
    historical = subprocess.run(
        ["git", "-C", str(root), "show", f"{commit}:{path}"],
        check=False,
        capture_output=True,
    )
    if historical.returncode != 0:
        failures.append(f"focused mutation artifact input path is absent at source_commit: {path}")
    elif historical.stdout != current.read_bytes():
        failures.append(f"focused mutation artifact input differs from source_commit: {path}")
    return failures


def _mapping_by_id(value: Any, label: str, failures: list[str]) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        failures.append(f"{label} rows must be an array")
        return {}
    rows: dict[str, Mapping[str, Any]] = {}
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            failures.append(f"{label}[{index}] must be an object")
            continue
        identity = item.get("id")
        if not isinstance(identity, str) or not identity:
            failures.append(f"{label}[{index}] id must be non-empty text")
            continue
        if identity in rows:
            failures.append(f"{label} duplicates {identity}")
        rows[identity] = item
    return rows


def _command_result(value: Mapping[str, Any], prefix: str) -> CommandResult | None:
    if prefix == "test" and value.get("test_exit_status") is None and not value.get("test_timed_out"):
        return None
    command = value[f"{prefix}_command"]
    if not isinstance(command, list) or not all(isinstance(item, str) for item in command):
        raise TypeError(f"{prefix}_command")
    duration = value.get(f"{prefix}_duration_seconds", 0.0)
    if not isinstance(duration, (int, float)) or isinstance(duration, bool):
        raise TypeError(f"{prefix}_duration_seconds")
    return CommandResult(
        command=tuple(command),
        returncode=value[f"{prefix}_exit_status"],
        stdout=value[f"{prefix}_stdout"],
        stderr=value[f"{prefix}_stderr"],
        timed_out=value[f"{prefix}_timed_out"],
        duration_seconds=float(duration),
    )


def _validate_baselines(
    value: Any,
    definitions: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    if not isinstance(value, list):
        failures.append("focused mutation artifact baselines must be an array")
        return
    expected: list[list[str]] = []
    for definition in definitions.values():
        command = definition.get("test_command")
        if isinstance(command, list) and command not in expected:
            expected.append(command)
    actual = [item.get("command") if isinstance(item, Mapping) else None for item in value]
    if actual != expected:
        failures.append("focused mutation artifact baseline commands drift from shard")
    for item in value:
        if not isinstance(item, Mapping):
            failures.append("focused mutation artifact baseline row must be an object")
        elif item.get("exit_status") != 0 or item.get("timed_out") is not False:
            failures.append("focused mutation artifact baseline did not pass")
