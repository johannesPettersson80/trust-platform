"""At-rest contract for producer-authentic broad remote gate evidence."""

from __future__ import annotations

import datetime as dt
import re
from pathlib import Path
from typing import Any, Callable, Mapping

Fail = Callable[[Path, str], None]

PRODUCER = "broad-remote-gate.py v1"
SUITE_ID = "pr"
GENERATED_REPORT_VERSION = "broad-remote-gate-v1"
REMOTE_GATE_SHELL = (
    'cd "$HOME/projects/trust-platform" && '
    'mkdir -p "$HOME/.cache/codex-targets/trust-platform-gate" '
    '"$HOME/.cache/codex-targets/trust-platform-gate-tmp" && '
    'export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" '
    'TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp" && '
    "just fmt && just clippy && just test-all"
)
REVIEWED_GATE_COMMAND = "ssh trust-builder " + repr(REMOTE_GATE_SHELL)
PLATFORM = "trust-builder-linux-x86_64"
MIN_HOME_AVAILABLE_KIB = 60 * 1024 * 1024
MIN_TMP_AVAILABLE_KIB = 3 * 1024 * 1024
RUST_SOURCE_KINDS = {"rust_integration_test", "rust_unit_test"}
CANONICAL_PATH = "verification/evidence-index.toml"
EVIDENCE_ID_RE = re.compile(r"^EVID_BROAD_REMOTE_PR_([0-9]{8})_[A-F0-9]{12}$")
FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")

EXACT_FIELDS: dict[str, Any] = {
    "schema_version": 1,
    "owner": "verification",
    "status": "mapped",
    "kind": "committed_file",
    "path": CANONICAL_PATH,
    "command": REVIEWED_GATE_COMMAND,
    "platform": PLATFORM,
    "suite_id": SUITE_ID,
    "producer": PRODUCER,
    "generated_report_version": GENERATED_REPORT_VERSION,
    "proof_kind": "none",
    "proof_scope": "broad_remote_gate",
    "command_exit_status": 0,
    "linked_spec_gaps": [],
    "local_source_clean_before": True,
    "local_source_clean_after": True,
    "remote_source_clean_before": True,
    "remote_source_clean_after": True,
    "disk_preflight_passed": True,
}
PRODUCER_FIELDS = {
    "schema_version",
    "id",
    "title",
    "area",
    "owner",
    "status",
    "kind",
    "path",
    "command",
    "commit",
    "remote_commit",
    "platform",
    "date",
    "suite_id",
    "producer",
    "generated_report_version",
    "linked_invariants",
    "linked_tests",
    "linked_spec_gaps",
    "last_reviewed",
    "proof_kind",
    "proof_scope",
    "command_exit_status",
    "executed_tests",
    "gate_started_at",
    "gate_finished_at",
    "gate_duration_milliseconds",
    "local_source_clean_before",
    "local_source_clean_after",
    "remote_source_clean_before",
    "remote_source_clean_after",
    "disk_preflight_passed",
    "home_available_kib",
    "tmp_available_kib",
}
EXECUTED_TEST_FIELDS = {
    "test_id",
    "discovery_id",
    "discovery_source_kind",
    "command",
    "run_id",
    "case_file_digest",
    "case_artifact_digest",
    "per_case_summary",
    "exit_status",
}


def validate_broad_remote_gate_evidence(
    *,
    fail: Fail,
    path: Path,
    record: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    ignored_tests: Mapping[str, Mapping[str, Any]],
) -> None:
    """Validate records owned by the reviewed broad-gate producer."""

    if record.get("producer") != PRODUCER:
        return
    evidence_id = str(record.get("id", "<unknown>"))
    actual_fields = set(record) - {"_path"}
    if actual_fields != PRODUCER_FIELDS:
        missing = sorted(PRODUCER_FIELDS - actual_fields)
        extra = sorted(actual_fields - PRODUCER_FIELDS)
        fail(
            path,
            f"{evidence_id} broad remote producer fields drifted: "
            f"missing={missing}, extra={extra}",
        )
    for field, expected in EXACT_FIELDS.items():
        if record.get(field) != expected:
            if field == "command":
                fail(path, f"{evidence_id} must use the reviewed command")
                continue
            fail(
                path,
                f"{evidence_id} broad remote producer {field} must equal {expected!r}",
            )

    identifier = EVIDENCE_ID_RE.fullmatch(evidence_id)
    if identifier is None:
        fail(path, f"{evidence_id} broad remote producer has an invalid evidence id")
    elif record.get("date") != _date_from_identifier(identifier.group(1)):
        fail(path, f"{evidence_id} date must match its evidence id")
    if record.get("last_reviewed") != record.get("date"):
        fail(path, f"{evidence_id} last_reviewed must equal the execution date")

    commit = record.get("commit")
    remote_commit = record.get("remote_commit")
    if not isinstance(commit, str) or FULL_COMMIT_RE.fullmatch(commit) is None:
        fail(path, f"{evidence_id} broad remote producer requires a clean full commit")
    if remote_commit != commit:
        fail(path, f"{evidence_id} remote_commit must equal commit")

    started = _timestamp(record.get("gate_started_at"))
    finished = _timestamp(record.get("gate_finished_at"))
    if started is None:
        fail(path, f"{evidence_id} gate_started_at must be an ISO-8601 UTC timestamp")
    if finished is None:
        fail(path, f"{evidence_id} gate_finished_at must be an ISO-8601 UTC timestamp")
    if started is not None and finished is not None and finished < started:
        fail(path, f"{evidence_id} gate_finished_at precedes gate_started_at")
    duration = record.get("gate_duration_milliseconds")
    if isinstance(duration, bool) or not isinstance(duration, int) or duration < 0:
        fail(path, f"{evidence_id} requires a non-negative integer duration")
    if started is not None and record.get("date") != started.date().isoformat():
        fail(path, f"{evidence_id} date must match gate_started_at")
    if started is not None and finished is not None and isinstance(duration, int):
        elapsed = round((finished - started).total_seconds() * 1000)
        if abs(duration - elapsed) > 1000:
            fail(path, f"{evidence_id} duration does not match gate timestamps")
    for field, minimum in (
        ("home_available_kib", MIN_HOME_AVAILABLE_KIB),
        ("tmp_available_kib", MIN_TMP_AVAILABLE_KIB),
    ):
        value = record.get(field)
        if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
            fail(
                path,
                f"{evidence_id} {field} must meet the reviewed minimum {minimum}",
            )

    invariant_ids = _canonical_ids(record.get("linked_invariants"))
    linked_test_ids = _canonical_ids(record.get("linked_tests"))
    if invariant_ids is None or not invariant_ids:
        fail(path, f"{evidence_id} linked_invariants must be a non-empty canonical list")
        return
    if linked_test_ids is None or not linked_test_ids:
        fail(path, f"{evidence_id} linked_tests must be a non-empty canonical list")
        return

    areas: set[str] = set()
    for invariant_id in invariant_ids:
        invariant = invariants.get(invariant_id)
        if not isinstance(invariant, Mapping):
            fail(path, f"{evidence_id} references unknown invariant {invariant_id}")
            continue
        area = invariant.get("area")
        if isinstance(area, str):
            areas.add(area)
    _validate_executed_tests(
        fail=fail,
        path=path,
        evidence_id=evidence_id,
        value=record.get("executed_tests"),
        linked_test_ids=linked_test_ids,
    )
    if len(areas) != 1 or record.get("area") not in areas:
        fail(path, f"{evidence_id} linked invariants must share and match one area")
    expected_title = "Reviewed PR broad gate for " + ", ".join(invariant_ids)
    if record.get("title") != expected_title:
        fail(path, f"{evidence_id} title does not match linked invariants")


def _validate_executed_tests(
    *,
    fail: Fail,
    path: Path,
    evidence_id: str,
    value: Any,
    linked_test_ids: list[str],
) -> None:
    if not isinstance(value, list) or not value:
        fail(path, f"{evidence_id} executed_tests must be a non-empty array")
        return
    ids: list[str] = []
    for index, entry in enumerate(value):
        if not isinstance(entry, Mapping):
            fail(path, f"{evidence_id} executed_tests[{index}] must be an object")
            continue
        if set(entry) != EXECUTED_TEST_FIELDS:
            fail(path, f"{evidence_id} executed_tests[{index}] fields drifted")
        test_id = entry.get("test_id")
        if isinstance(test_id, str) and test_id:
            ids.append(test_id)
        else:
            fail(path, f"{evidence_id} executed_tests[{index}].test_id is invalid")
        for field in ("discovery_id", "command", "run_id"):
            if not isinstance(entry.get(field), str) or not entry.get(field):
                fail(path, f"{evidence_id} executed_tests[{index}].{field} is invalid")
        if entry.get("discovery_source_kind") not in RUST_SOURCE_KINDS:
            fail(path, f"{evidence_id} executed_tests[{index}] source kind is invalid")
        for field in ("case_file_digest", "case_artifact_digest"):
            value = entry.get(field)
            if not isinstance(value, str) or re.fullmatch(r"^sha256:[0-9a-f]{64}$", value) is None:
                fail(path, f"{evidence_id} executed_tests[{index}].{field} is invalid")
        summary = entry.get("per_case_summary")
        if (
            not isinstance(summary, list)
            or not summary
            or any(not isinstance(item, str) or not item.endswith(":passed") for item in summary)
        ):
            fail(path, f"{evidence_id} executed_tests[{index}] must record only passed cases")
        elif len(summary) != len(
            {item.removesuffix(":passed") for item in summary}
        ):
            fail(path, f"{evidence_id} executed_tests[{index}] has duplicate case ids")
        if entry.get("exit_status") != 0:
            fail(path, f"{evidence_id} executed_tests[{index}].exit_status must equal 0")
    if ids != sorted(set(ids)):
        fail(path, f"{evidence_id} executed_tests must be canonical by test_id")
    if ids != linked_test_ids:
        fail(path, f"{evidence_id} linked_tests must exactly match executed_tests")


def _canonical_ids(value: Any) -> list[str] | None:
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        return None
    if value != sorted(set(value)):
        return None
    return value


def _timestamp(value: Any) -> dt.datetime | None:
    if not isinstance(value, str) or not value.endswith("Z"):
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.removesuffix("Z") + "+00:00")
    except ValueError:
        return None
    return parsed if parsed.tzinfo == dt.timezone.utc else None


def _date_from_identifier(compact: str) -> str:
    return f"{compact[:4]}-{compact[4:6]}-{compact[6:]}"
