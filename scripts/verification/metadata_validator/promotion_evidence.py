"""Evidence-scope and invariant proof-level promotion contracts."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path
from typing import Any, Callable, Mapping

from .constants import PROOF_SCOPES, PROVE_PRODUCER_RE, ROOT


Fail = Callable[[Path, str], None]
RevisionExists = Callable[[str], bool]

CLEAN_FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
TRUST_BUILDER_PLATFORM_RE = re.compile(r"^trust-builder-linux-[a-z0-9_]+$")
CLOSING_PROOF_KINDS = {"green", "lock_compare"}
PROOF_PRODUCING_KINDS = {
    "red",
    "protective_red",
    "green",
    "lock_baseline",
    "lock_compare",
}
BROAD_REMOTE_SUITE_IDS = {"pr", "nightly", "hardware_lab"}
BROAD_REMOTE_EVIDENCE_KINDS = {"committed_file", "ci_artifact", "lab_report"}
PROMOTED_PROOF_LEVELS = {"G1", "G2", "R1"}


def validate_evidence_scope(
    *,
    fail: Fail,
    path: Path,
    record: Mapping[str, Any],
    revision_exists: RevisionExists | None = None,
    approved_producers: set[str] | frozenset[str] = frozenset(),
) -> None:
    """Validate the closed evidence-scope vocabulary and its structural claims."""

    evidence_id = str(record.get("id", "<unknown>"))
    proof_kind = record.get("proof_kind")
    scope = record.get("proof_scope")

    if isinstance(proof_kind, str) and proof_kind in PROOF_PRODUCING_KINDS and scope != "targeted":
        fail(path, f"{evidence_id} {proof_kind} proof must use proof_scope targeted")

    if scope is None:
        return
    if not isinstance(scope, str) or scope not in PROOF_SCOPES:
        fail(path, f"{evidence_id} has unknown proof_scope {scope!r}")
        return

    revision = str(record.get("commit", ""))
    if not CLEAN_FULL_COMMIT_RE.fullmatch(revision):
        fail(path, f"{evidence_id} proof_scope {scope} requires a clean full 40-hex commit")
    elif not (revision_exists or _revision_exists)(revision):
        fail(path, f"{evidence_id} proof_scope {scope} commit {revision} does not resolve")

    if scope == "targeted":
        if not isinstance(proof_kind, str) or proof_kind not in PROOF_PRODUCING_KINDS:
            fail(path, f"{evidence_id} proof_scope targeted requires a proof-producing proof_kind")
        return

    if proof_kind != "none":
        fail(path, f"{evidence_id} proof_scope {scope} requires proof_kind none")
    if str(record.get("producer", "")) not in approved_producers:
        fail(
            path,
            f"{evidence_id} proof_scope {scope} producer must be suite-approved",
        )

    if scope == "broad_remote_gate":
        suite_id = record.get("suite_id")
        if not isinstance(suite_id, str) or suite_id not in BROAD_REMOTE_SUITE_IDS:
            fail(
                path,
                f"{evidence_id} proof_scope broad_remote_gate requires suite_id "
                "pr, nightly, or hardware_lab",
            )
        kind = record.get("kind")
        if not isinstance(kind, str) or kind not in BROAD_REMOTE_EVIDENCE_KINDS:
            fail(
                path,
                f"{evidence_id} proof_scope broad_remote_gate requires "
                "committed_file, ci_artifact, or lab_report evidence",
            )
        if record.get("command_exit_status") != 0:
            fail(
                path,
                f"{evidence_id} proof_scope broad_remote_gate requires "
                "command_exit_status = 0",
            )
        if record.get("kind") == "committed_file":
            platform = record.get("platform")
            if not isinstance(platform, str) or not TRUST_BUILDER_PLATFORM_RE.fullmatch(
                platform
            ):
                fail(
                    path,
                    f"{evidence_id} broad_remote_gate committed_file must use "
                    "an exclusive trust-builder platform",
                )
        return

    if record.get("kind") != "release_object":
        fail(path, f"{evidence_id} proof_scope release_public requires kind release_object")
    if record.get("suite_id") not in (None, "release"):
        fail(path, f"{evidence_id} proof_scope release_public may only name suite_id release")


def validate_invariant_promotion_evidence(
    *,
    fail: Fail,
    path: Path,
    invariant: Mapping[str, Any],
    evidence: Mapping[str, Mapping[str, Any]],
    approved_producers: set[str] | frozenset[str] = frozenset(),
) -> None:
    """Require cumulative, bidirectionally-linked evidence for G1/G2/R1."""

    invariant_id = str(invariant.get("id", "<unknown>"))
    evidence_refs = invariant.get("evidence_refs")
    referenced = (
        [
            evidence[evidence_id]
            for evidence_id in evidence_refs
            if isinstance(evidence_id, str) and evidence_id in evidence
        ]
        if isinstance(evidence_refs, list)
        else []
    )
    status = invariant.get("status")
    if status == "test_written" and not any(
        _is_targeted_red_proof(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            approved_producers=approved_producers,
        )
        for record in referenced
    ):
        fail(
            path,
            f"{invariant_id} status test_written requires targeted "
            "red/protective evidence",
        )

    if status == "implemented" and not any(
        _is_targeted_closing_proof(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            approved_producers=approved_producers,
        )
        for record in referenced
    ):
        fail(
            path,
            f"{invariant_id} status implemented requires targeted green/lock proof",
        )

    proof_level = invariant.get("proof_level")
    if not isinstance(proof_level, str) or proof_level not in PROMOTED_PROOF_LEVELS:
        return

    if not any(
        _is_targeted_closing_proof(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            approved_producers=approved_producers,
        )
        for record in referenced
    ):
        fail(
            path,
            f"{invariant_id} proof_level {proof_level} requires targeted "
            "green/lock proof",
        )

    if proof_level in {"G2", "R1"} and not any(
        _is_broad_remote_gate(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            approved_producers=approved_producers,
        )
        for record in referenced
    ):
        fail(
            path,
            f"{invariant_id} proof_level {proof_level} requires broad remote "
            "gate evidence",
        )

    if proof_level == "R1" and not any(
        _is_release_public(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            approved_producers=approved_producers,
        )
        for record in referenced
    ):
        fail(
            path,
            f"{invariant_id} proof_level R1 requires release/public evidence",
        )


def _is_targeted_closing_proof(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    approved_producers: set[str] | frozenset[str],
) -> bool:
    producer = str(record.get("producer", ""))
    return (
        record.get("proof_scope") == "targeted"
        and isinstance(record.get("proof_kind"), str)
        and record.get("proof_kind") in CLOSING_PROOF_KINDS
        and _has_clean_commit(record)
        and _back_links(record, invariant_id, invariant_tests, require_all_tests=False)
        and (bool(PROVE_PRODUCER_RE.fullmatch(producer)) or producer in approved_producers)
    )


def _is_targeted_red_proof(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    approved_producers: set[str] | frozenset[str],
) -> bool:
    producer = str(record.get("producer", ""))
    red_case_ids = record.get("red_case_ids")
    summary = record.get("per_case_summary")
    command_exit_status = record.get("command_exit_status")
    if (
        not isinstance(red_case_ids, list)
        or not red_case_ids
        or any(not isinstance(case_id, str) or not case_id for case_id in red_case_ids)
        or not isinstance(summary, list)
        or any(not isinstance(item, str) for item in summary)
        or isinstance(command_exit_status, bool)
        or not isinstance(command_exit_status, int)
        or command_exit_status == 0
    ):
        return False
    failed_cases = {
        item.removesuffix(":failed")
        for item in summary
        if item.endswith(":failed")
    }
    return (
        record.get("proof_scope") == "targeted"
        and isinstance(record.get("proof_kind"), str)
        and record.get("proof_kind") in {"red", "protective_red"}
        and record.get("failure_kind") in {"assertion_failure", "expected_rejection"}
        and isinstance(record.get("case_file_digest"), str)
        and bool(record.get("case_file_digest"))
        and set(red_case_ids) <= failed_cases
        and _has_clean_commit(record)
        and _back_links(record, invariant_id, invariant_tests, require_all_tests=False)
        and (bool(PROVE_PRODUCER_RE.fullmatch(producer)) or producer in approved_producers)
    )


def _is_broad_remote_gate(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    approved_producers: set[str] | frozenset[str],
) -> bool:
    if record.get("proof_scope") != "broad_remote_gate":
        return False
    if record.get("proof_kind") != "none" or not _has_clean_commit(record):
        return False
    suite_id = record.get("suite_id")
    if not isinstance(suite_id, str) or suite_id not in BROAD_REMOTE_SUITE_IDS:
        return False
    kind = record.get("kind")
    if not isinstance(kind, str) or kind not in BROAD_REMOTE_EVIDENCE_KINDS:
        return False
    if record.get("command_exit_status") != 0:
        return False
    if str(record.get("producer", "")) not in approved_producers:
        return False
    if record.get("kind") == "committed_file":
        platform = record.get("platform")
        if not isinstance(platform, str) or not TRUST_BUILDER_PLATFORM_RE.fullmatch(
            platform
        ):
            return False
    return _back_links(record, invariant_id, invariant_tests, require_all_tests=True)


def _is_release_public(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    approved_producers: set[str] | frozenset[str],
) -> bool:
    return (
        record.get("proof_scope") == "release_public"
        and record.get("proof_kind") == "none"
        and record.get("kind") == "release_object"
        and record.get("suite_id") in (None, "release")
        and _has_clean_commit(record)
        and str(record.get("producer", "")) in approved_producers
        and _back_links(record, invariant_id, invariant_tests, require_all_tests=True)
    )


def _back_links(
    record: Mapping[str, Any],
    invariant_id: str,
    invariant_tests: Any,
    *,
    require_all_tests: bool,
) -> bool:
    linked_invariants = record.get("linked_invariants")
    if not isinstance(linked_invariants, list) or invariant_id not in linked_invariants:
        return False
    if (
        not isinstance(invariant_tests, list)
        or not invariant_tests
        or any(not isinstance(test_id, str) for test_id in invariant_tests)
    ):
        return False
    linked_tests = record.get("linked_tests")
    if not isinstance(linked_tests, list) or any(
        not isinstance(test_id, str) for test_id in linked_tests
    ):
        return False
    expected = set(invariant_tests)
    actual = set(linked_tests)
    return expected <= actual if require_all_tests else bool(expected & actual)


def _has_clean_commit(record: Mapping[str, Any]) -> bool:
    return bool(CLEAN_FULL_COMMIT_RE.fullmatch(str(record.get("commit", ""))))


def _revision_exists(revision: str) -> bool:
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{revision}^{{commit}}"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0
