"""Closed contract for bounded fuzz campaigns and crash regressions."""

from __future__ import annotations

import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import PurePosixPath
from typing import Any

from .fuzz_crash_regressions import campaign_regressions


DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
TOP_FIELDS = {
    "schema_version",
    "generator",
    "generator_version",
    "source_commit",
    "started_at",
    "finished_at",
    "platform",
    "requested_runs",
    "max_total_time_seconds",
    "timeout_seconds",
    "results",
    "regressions",
    "summary",
}
RESULT_FIELDS = {
    "target_id",
    "target_kind",
    "command",
    "exit_status",
    "timed_out",
    "executions",
    "log_sha256",
    "artifact_files",
}
ARTIFACT_FIELDS = {"path", "sha256", "size"}
REGRESSION_FIELDS = {"target_id", "artifact_sha256", "test_id", "rationale"}
SUMMARY_FIELDS = {
    "targets",
    "passed",
    "infrastructure_failures",
    "crash_artifacts",
    "regressions",
}


def validate_campaign_payload(
    payload: Mapping[str, Any],
    *,
    program: Mapping[str, Any],
    tests: Mapping[str, Mapping[str, Any]],
    regression_registry: Mapping[str, Any],
) -> list[str]:
    failures: list[str] = []
    _fields(payload, TOP_FIELDS, "campaign", failures)
    for field, expected in (
        ("schema_version", 1),
        ("generator", "bounded-fuzz-campaign"),
        ("generator_version", 1),
    ):
        if payload.get(field) != expected:
            failures.append(f"campaign {field} must equal {expected!r}")
    if not isinstance(payload.get("source_commit"), str) or not COMMIT_RE.fullmatch(
        str(payload.get("source_commit", ""))
    ):
        failures.append("campaign source_commit must be a clean full Git SHA")
    for field in ("started_at", "finished_at"):
        if not _timestamp(payload.get(field)):
            failures.append(f"campaign {field} must be timezone-aware ISO-8601")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("campaign platform must be a non-empty string")
    for field in ("requested_runs", "max_total_time_seconds", "timeout_seconds"):
        if not _positive_int(payload.get(field)):
            failures.append(f"campaign {field} must be a positive integer")

    raw_targets = program.get("targets")
    targets = raw_targets if isinstance(raw_targets, list) else []
    target_by_id = {
        row.get("id"): row
        for row in targets
        if isinstance(row, Mapping) and isinstance(row.get("id"), str)
    }
    results = _rows(payload.get("results"), RESULT_FIELDS, "results", failures)
    expected_ids = [row.get("id") for row in targets if isinstance(row, Mapping)]
    actual_ids = [row.get("target_id") for row in results]
    if actual_ids != expected_ids or len(set(actual_ids)) != len(actual_ids):
        failures.append("campaign results must exactly match registered targets in order")

    artifacts: dict[tuple[str, str], dict[str, Any]] = {}
    passed = 0
    infrastructure_failures = 0
    for row in results:
        target_id = row.get("target_id")
        target = target_by_id.get(target_id)
        if not isinstance(target, Mapping):
            continue
        target_kind = target.get("target_kind")
        if row.get("target_kind") != target_kind:
            failures.append(f"{target_id}: target_kind does not match the program")
        expected_command = _expected_command(payload, target)
        if row.get("command") != expected_command:
            failures.append(f"{target_id}: command does not match the bounded program")
        exit_status = row.get("exit_status")
        timed_out = row.get("timed_out")
        executions = row.get("executions")
        if not isinstance(exit_status, int) or isinstance(exit_status, bool):
            failures.append(f"{target_id}: exit_status must be an integer")
        if not isinstance(timed_out, bool):
            failures.append(f"{target_id}: timed_out must be boolean")
        if not _positive_int(executions):
            failures.append(f"{target_id}: executions must be a positive integer")
        if not isinstance(row.get("log_sha256"), str) or not DIGEST_RE.fullmatch(
            str(row.get("log_sha256", ""))
        ):
            failures.append(f"{target_id}: log_sha256 must be sha256:<64 lowercase hex>")
        artifact_rows = _rows(
            row.get("artifact_files"), ARTIFACT_FIELDS, f"{target_id}.artifact_files", failures
        )
        for artifact in artifact_rows:
            path = artifact.get("path")
            digest = artifact.get("sha256")
            if not _safe_path(path):
                failures.append(f"{target_id}: artifact path must be normalized and relative")
            expected_root = target.get("artifact_path")
            if isinstance(expected_root, str) and isinstance(path, str) and not (
                path == expected_root or path.startswith(expected_root + "/")
            ):
                failures.append(f"{target_id}: artifact path escapes the registered directory")
            if not isinstance(digest, str) or not DIGEST_RE.fullmatch(digest):
                failures.append(f"{target_id}: artifact digest must be sha256:<64 lowercase hex>")
            if not isinstance(artifact.get("size"), int) or artifact.get("size", -1) < 0:
                failures.append(f"{target_id}: artifact size must be a non-negative integer")
            if isinstance(digest, str):
                key = (str(target_id), digest)
                if key in artifacts:
                    failures.append(f"{target_id}: duplicate artifact digest {digest}")
                artifacts[key] = artifact
        if exit_status == 0 and timed_out is False and not artifact_rows:
            passed += 1
        elif not artifact_rows:
            infrastructure_failures += 1
            failures.append(f"{target_id}: infrastructure failure aborted the campaign")

    regressions = _rows(
        payload.get("regressions"), REGRESSION_FIELDS, "regressions", failures
    )
    if regressions != campaign_regressions(regression_registry, results):
        failures.append("campaign regression rows do not match committed registry")
    regression_keys: set[tuple[str, str]] = set()
    for row in regressions:
        target_id = row.get("target_id")
        digest = row.get("artifact_sha256")
        key = (str(target_id), str(digest))
        if key in regression_keys:
            failures.append(f"duplicate regression handoff for {target_id} {digest}")
        regression_keys.add(key)
        if key not in artifacts:
            failures.append(f"regression {target_id} {digest} has no campaign crash artifact")
        test_id = row.get("test_id")
        test = tests.get(test_id) if isinstance(test_id, str) else None
        if not isinstance(test, Mapping) or test.get("status") != "mapped" or not test.get(
            "command"
        ):
            failures.append(f"{target_id}: deterministic regression {test_id!r} is not mapped")
        if not isinstance(row.get("rationale"), str) or not row.get("rationale"):
            failures.append(f"{target_id}: regression rationale must be non-empty")
    for key in sorted(set(artifacts) - regression_keys):
        failures.append(
            f"{key[0]} crash artifact {key[1]} requires a mapped deterministic regression"
        )

    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("campaign summary must be an object")
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
        expected_summary = {
            "targets": len(results),
            "passed": passed,
            "infrastructure_failures": infrastructure_failures,
            "crash_artifacts": len(artifacts),
            "regressions": len(regressions),
        }
        if dict(summary) != expected_summary:
            failures.append("campaign summary does not match result and regression rows")
    return sorted(set(failures))


def _expected_command(payload: Mapping[str, Any], target: Mapping[str, Any]) -> str:
    if target.get("target_kind") == "bounded_rust_smoke":
        return str(target.get("command"))
    return (
        f"cargo +nightly fuzz run {target.get('name')} -- "
        f"-runs={payload.get('requested_runs')} "
        f"-max_total_time={payload.get('max_total_time_seconds')} "
        f"-timeout={payload.get('timeout_seconds')} -max_len=65536"
    )


def _rows(
    value: Any,
    fields: set[str],
    label: str,
    failures: list[str],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return []
    rows: list[dict[str, Any]] = []
    for index, row in enumerate(value):
        if not isinstance(row, Mapping):
            failures.append(f"{label}[{index}] must be an object")
            continue
        _fields(row, fields, f"{label}[{index}]", failures)
        rows.append(dict(row))
    return rows


def _fields(
    value: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    if set(value) != expected:
        failures.append(f"{label} fields must equal {sorted(expected)}")


def _timestamp(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).tzinfo is not None
    except ValueError:
        return False


def _positive_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def _safe_path(value: Any) -> bool:
    if not isinstance(value, str) or not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts
