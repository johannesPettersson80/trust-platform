"""Before/after behavior-lock rules for completed test refactors."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import tomllib
from collections import defaultdict
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .test_refactor_file_metrics import read_workspace_bytes


PROPOSAL_EVIDENCE_FIELDS = {
    "before_behavior_lock_evidence",
    "after_behavior_lock_evidence",
}
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


def validate_behavior_lock_pair(
    root: Path,
    proposal: Mapping[str, Any],
    catalog: Mapping[str, Any] | None,
    evidence: Mapping[str, Mapping[str, Any]],
    label: str,
    failures: list[str],
) -> None:
    before_id = proposal.get("before_behavior_lock_evidence")
    after_id = proposal.get("after_behavior_lock_evidence")
    before = evidence.get(before_id) if isinstance(before_id, str) else None
    after = evidence.get(after_id) if isinstance(after_id, str) else None
    if before is None:
        failures.append(f"{label} references unknown before behavior-lock evidence {before_id!r}")
    if after is None:
        failures.append(f"{label} references unknown after behavior-lock evidence {after_id!r}")
    if before is None or after is None:
        return

    expected_case_digest = catalog.get("case_file_digest") if catalog is not None else None
    expected_case_ids = _case_ids(root, catalog, label, failures)
    if before.get("proof_kind") != "lock_baseline":
        failures.append(f"{label} before evidence must use proof_kind lock_baseline")
    if after.get("proof_kind") != "lock_compare":
        failures.append(f"{label} after evidence must use proof_kind lock_compare")
    if after.get("paired_lock_baseline") != before_id:
        failures.append(f"{label} after evidence does not pair to before evidence")
    expected_tests = [proposal.get("test_id")]
    if before.get("linked_tests") != expected_tests or after.get("linked_tests") != expected_tests:
        failures.append(f"{label} behavior-lock evidence must link exactly the proposal test")
    if before.get("command") != proposal.get("before_command"):
        failures.append(f"{label} before evidence command does not match proposal")
    if after.get("command") != proposal.get("after_command"):
        failures.append(f"{label} after evidence command does not match proposal")
    if before.get("linked_invariants") != proposal.get("invariant_ids") or after.get(
        "linked_invariants"
    ) != proposal.get("invariant_ids"):
        failures.append(
            f"{label} behavior-lock evidence must link exactly the proposal invariants"
        )
    before_run_id = before.get("trust_verify_run_id")
    after_run_id = after.get("trust_verify_run_id")
    if not isinstance(before_run_id, str) or not before_run_id:
        failures.append(f"{label} before behavior-lock evidence requires a run ID")
    if not isinstance(after_run_id, str) or not after_run_id:
        failures.append(f"{label} after behavior-lock evidence requires a run ID")
    if before_run_id == after_run_id:
        failures.append(f"{label} behavior-lock evidence must use distinct run IDs")
    _validate_revisions(root, before, after, label, failures)

    for evidence_label, record in (("before", before), ("after", after)):
        if record.get("case_file_digest") != expected_case_digest:
            failures.append(
                f"{label} {evidence_label} evidence case_file_digest does not match catalog"
            )
        if not DIGEST_RE.fullmatch(str(record.get("case_artifact_digest", ""))):
            failures.append(
                f"{label} {evidence_label} evidence requires a case_artifact_digest"
            )
        if record.get("command_exit_status") != 0:
            failures.append(f"{label} {evidence_label} evidence command_exit_status must be 0")
        summary = record.get("per_case_summary")
        if not isinstance(summary, list) or not summary or not all(
            isinstance(item, str) and item.endswith(":passed") for item in summary
        ):
            failures.append(f"{label} {evidence_label} evidence must contain only passing case summaries")
        elif record.get("case_result_digest") != case_result_digest(
            command_exit_status=record.get("command_exit_status"),
            per_case_summary=summary,
        ):
            failures.append(
                f"{label} {evidence_label} evidence case_result_digest does not bind its result"
            )
        if isinstance(summary, list):
            reported_ids = [
                item.partition(":")[0] for item in summary if isinstance(item, str)
            ]
            if len(reported_ids) != len(set(reported_ids)):
                failures.append(
                    f"{label} {evidence_label} evidence repeats case IDs"
                )
            if set(reported_ids) != expected_case_ids:
                failures.append(
                    f"{label} {evidence_label} evidence case IDs do not match the committed case file"
                )
    if before.get("case_result_digest") != after.get("case_result_digest"):
        failures.append(f"{label} behavior-lock result digest does not match")
    if before.get("per_case_summary") != after.get("per_case_summary"):
        failures.append(f"{label} behavior-lock case summaries do not match")


def validate_behavior_lock_evidence_reuse(
    proposals: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    references: dict[str, list[str]] = defaultdict(list)
    for proposal_id, proposal in proposals.items():
        for field in PROPOSAL_EVIDENCE_FIELDS:
            evidence_id = proposal.get(field)
            if isinstance(evidence_id, str):
                references[evidence_id].append(f"{proposal_id}.{field}")
    for evidence_id, owners in sorted(references.items()):
        if len(owners) > 1:
            failures.append(
                f"behavior-lock evidence {evidence_id} is reused by {', '.join(sorted(owners))}"
            )


def case_result_digest(*, command_exit_status: Any, per_case_summary: list[Any]) -> str:
    raw = json.dumps(
        {
            "command_exit_status": command_exit_status,
            "per_case_summary": per_case_summary,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def _case_ids(
    root: Path,
    catalog: Mapping[str, Any] | None,
    label: str,
    failures: list[str],
) -> set[str]:
    case_file = catalog.get("case_file") if catalog is not None else None
    if not isinstance(case_file, str):
        return set()
    try:
        payload = tomllib.loads(read_workspace_bytes(root, case_file).decode("utf-8"))
    except (UnicodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        failures.append(f"{label} case file cannot be read for lock proof: {exc}")
        return set()
    cases = payload.get("case")
    if not isinstance(cases, list):
        failures.append(f"{label} case file must contain [[case]] records")
        return set()
    ids = [case.get("id") for case in cases if isinstance(case, Mapping)]
    if len(ids) != len(cases) or any(not isinstance(case_id, str) for case_id in ids):
        failures.append(f"{label} case file contains a case without a string id")
        return set()
    if len(ids) != len(set(ids)):
        failures.append(f"{label} case file duplicates case IDs")
    return {str(case_id) for case_id in ids}


def _validate_revisions(
    root: Path,
    before: Mapping[str, Any],
    after: Mapping[str, Any],
    label: str,
    failures: list[str],
) -> None:
    revisions: list[str] = []
    for evidence_label, record in (("before", before), ("after", after)):
        match = COMMIT_RE.fullmatch(str(record.get("commit", "")))
        if match is None:
            failures.append(
                f"{label} {evidence_label} evidence commit must be a clean full Git SHA"
            )
            return
        resolved = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--verify", f"{match.group(0)}^{{commit}}"],
            check=False,
            capture_output=True,
            text=True,
        )
        if resolved.returncode != 0:
            failures.append(
                f"{label} {evidence_label} evidence commit does not resolve"
            )
            return
        revisions.append(resolved.stdout.strip())
    if revisions[0] == revisions[1]:
        failures.append(f"{label} behavior locks require distinct source revisions")
        return
    ancestor = subprocess.run(
        ["git", "-C", str(root), "merge-base", "--is-ancestor", revisions[0], revisions[1]],
        check=False,
    )
    if ancestor.returncode != 0:
        failures.append(
            f"{label} before behavior-lock revision must be an ancestor of after revision"
        )
