"""At-rest validation for bytecode-validator mutation reports."""

from __future__ import annotations

import re
from typing import Any

from .mutation_contracts import CASE_SEMANTICS, MutationContract


RESULTS = {"caught", "survived", "unviable", "timeout", "error"}
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
INFRASTRUCTURE_FAILURE_MARKERS = (
    "no space left on device",
    "disk quota exceeded",
    "mold: failed to write",
    "couldn't create a temp dir",
    "could not create a temp dir",
    "permission denied",
)


def validate_mutation_report(report: Any, contract: MutationContract) -> list[str]:
    if not isinstance(report, dict):
        return ["mutation report JSON root must be an object"]
    failures: list[str] = []
    required = [
        "schema_version",
        "id",
        "status",
        "shard_id",
        "test_id",
        "runner",
        "tool",
        "tool_version",
        "source_commit",
        "platform",
        "started_at",
        "finished_at",
        "case_file",
        "case_file_digest",
        "case_semantics",
        "blocked_case_ids_executed",
        "baseline_commands",
        "mutations",
        "summary",
        "survivors",
        "out_of_scope_case_ids",
        "out_of_scope_reason",
    ]
    for field in required:
        if field not in report:
            failures.append(f"mutation report missing {field}")
    if failures:
        return failures
    if report.get("schema_version") != 1 or report.get("status") != "complete":
        failures.append("mutation report must use schema_version 1 and status complete")
    if report.get("shard_id") != contract.shard_id or report.get("test_id") != contract.test_id:
        failures.append("mutation report shard/test binding mismatch")
    if (
        report.get("runner") != contract.runner
        or report.get("tool") != contract.tool
        or report.get("tool_version") != contract.tool_version
    ):
        failures.append("mutation report runner/tool binding mismatch")
    if report.get("case_file") != contract.case_file or report.get("case_file_digest") != contract.case_file_digest:
        failures.append("mutation report case-file binding mismatch")
    if report.get("case_semantics") != CASE_SEMANTICS or report.get("blocked_case_ids_executed") is not False:
        failures.append("mutation report must state that blocked case IDs were not executed")
    if not COMMIT_RE.match(str(report.get("source_commit", ""))):
        failures.append("mutation report source_commit must be a full Git commit")
    validate_baselines(report.get("baseline_commands"), contract, failures)

    configured = {mutation.id: mutation for mutation in contract.mutations}
    outcomes = report.get("mutations")
    if not isinstance(outcomes, list):
        failures.append("mutation report mutations must be a list")
        outcomes = []
    outcome_ids: list[str] = []
    for outcome in outcomes:
        if isinstance(outcome, dict) and isinstance(outcome.get("id"), str):
            outcome_ids.append(outcome["id"])
        elif isinstance(outcome, dict):
            failures.append("mutation report has outcome with missing/invalid ID")
    if len(outcome_ids) != len(set(outcome_ids)):
        failures.append("mutation report duplicates mutant outcomes")
    if set(outcome_ids) != set(configured):
        failures.append("mutation report outcome set does not match configured shard")
    counts = {result: 0 for result in RESULTS}
    expected_survivors: list[dict[str, Any]] = []
    for outcome in outcomes:
        if not isinstance(outcome, dict):
            failures.append("mutation report has non-object outcome")
            continue
        mutation_id = outcome.get("id")
        if not isinstance(mutation_id, str) or not mutation_id:
            continue
        mutation = configured.get(mutation_id)
        if not mutation:
            continue
        result = outcome.get("result")
        if not isinstance(result, str) or result not in RESULTS:
            failures.append(f"{mutation.id} has unknown mutation result {result!r}")
            continue
        counts[result] += 1
        if outcome.get("source_file") != mutation.source_file or outcome.get("function") != mutation.function:
            failures.append(f"{mutation.id} source/function binding mismatch")
        if outcome.get("genre") != mutation.genre or outcome.get("replacement") != mutation.replacement:
            failures.append(f"{mutation.id} selector binding mismatch")
        if not isinstance(outcome.get("generated_mutant_name"), str) or not outcome["generated_mutant_name"]:
            failures.append(f"{mutation.id} generated mutant name is missing")
        if outcome.get("build_command") != list(mutation.build_command):
            failures.append(f"{mutation.id} build command drift")
        if outcome.get("test_command") != list(mutation.test_command):
            failures.append(f"{mutation.id} test command drift")
        if outcome.get("related_case_ids") != list(mutation.related_case_ids):
            failures.append(f"{mutation.id} related case mapping drift")
        if outcome.get("survivor_action") != mutation.survivor_action:
            failures.append(f"{mutation.id} survivor action drift")
        derived_result, derivation_failure = derive_reported_result(outcome)
        if derivation_failure:
            failures.append(f"{mutation.id} {derivation_failure}")
        elif derived_result != result:
            failures.append(
                f"{mutation.id} exit status and timeout fields imply {derived_result}, not {result}"
            )
        if result == "survived":
            expected_survivors.append(
                {
                    "id": mutation.id,
                    "related_case_ids": list(mutation.related_case_ids),
                    "action": mutation.survivor_action,
                }
            )
    summary = report.get("summary")
    expected_summary = {"total": len(contract.mutations), **counts}
    if summary != expected_summary:
        failures.append(f"mutation report summary mismatch: expected {expected_summary}, actual {summary}")
    if counts["error"] != 0:
        failures.append("complete mutation reports cannot contain infrastructure errors")
    if report.get("survivors") != expected_survivors:
        failures.append("mutation report survivor mapping does not match outcomes and configured case IDs")
    if report.get("out_of_scope_case_ids") != list(contract.out_of_scope_case_ids):
        failures.append("mutation report out-of-scope case mapping drift")
    if report.get("out_of_scope_reason") != contract.out_of_scope_reason:
        failures.append("mutation report out-of-scope rationale drift")
    if contains_forbidden_execution_claim(report):
        failures.append("mutation report uses forbidden executed/killed case claim fields")
    return failures


def validate_baselines(value: Any, contract: MutationContract, failures: list[str]) -> None:
    if not isinstance(value, list) or not value:
        failures.append("mutation report must include passing baseline commands")
        return
    if not all(isinstance(item, dict) for item in value):
        failures.append("mutation report baseline command entries must be objects")
        return
    expected_commands: list[list[str]] = []
    for mutation in contract.mutations:
        command = list(mutation.test_command)
        if command not in expected_commands:
            expected_commands.append(command)
    actual_commands = [item.get("command") for item in value]
    if actual_commands != expected_commands:
        failures.append("mutation report baseline command set does not match configured test commands")
    for item in value:
        if item.get("exit_status") != 0 or item.get("timed_out") is not False:
            failures.append("mutation report baseline command did not pass without timeout")


def derive_reported_result(outcome: dict[str, Any]) -> tuple[str | None, str | None]:
    for field in ("build_timed_out", "test_timed_out"):
        if not isinstance(outcome.get(field), bool):
            return None, f"{field} must be boolean"
    for field in ("build_exit_status", "test_exit_status"):
        value = outcome.get(field)
        if value is not None and (not isinstance(value, int) or isinstance(value, bool)):
            return None, f"{field} must be an integer or null"
    build_status = outcome.get("build_exit_status")
    test_status = outcome.get("test_exit_status")
    if outcome["build_timed_out"] and build_status is not None:
        return None, "build timeout must have a null exit status"
    if outcome["test_timed_out"] and test_status is not None:
        return None, "test timeout must have a null exit status"
    if (outcome["build_timed_out"] or build_status is None or build_status != 0) and (
        test_status is not None
        or outcome["test_timed_out"]
        or bool(outcome.get("test_output_tail"))
    ):
        return None, "records a test result after build did not complete successfully"
    if has_infrastructure_failure(
        build_status,
        str(outcome.get("build_output_tail", "")),
    ) or has_infrastructure_failure(
        test_status,
        str(outcome.get("test_output_tail", "")),
    ):
        return "error", None
    if outcome["build_timed_out"] or outcome["test_timed_out"]:
        return "timeout", None
    if build_status is None:
        return "error", None
    if build_status != 0:
        return "unviable", None
    if test_status is None:
        return "error", None
    return ("survived" if test_status == 0 else "caught"), None


def has_infrastructure_failure(returncode: Any, output: str) -> bool:
    if isinstance(returncode, int) and not isinstance(returncode, bool) and returncode < 0:
        return True
    lowered = output.lower()
    return any(marker in lowered for marker in INFRASTRUCTURE_FAILURE_MARKERS)


def contains_forbidden_execution_claim(value: Any) -> bool:
    if isinstance(value, dict):
        if {"killed_by_case_ids", "executed_case_ids"} & set(value):
            return True
        return any(contains_forbidden_execution_claim(item) for item in value.values())
    if isinstance(value, list):
        return any(contains_forbidden_execution_claim(item) for item in value)
    return False
