"""Evidence-scope and invariant proof-level promotion contracts."""

from __future__ import annotations

import hashlib
import re
import subprocess
from pathlib import Path, PurePosixPath
from typing import Any, Callable, Mapping

from ..proof_case_artifacts import CaseArtifactContractError, load_case_contract
from .constants import PROOF_SCOPES, PROVE_PRODUCER_RE, ROOT
from .integrity import RUNNABLE_TEST_STATUSES


Fail = Callable[[Path, str], None]
RevisionExists = Callable[[str], bool]
IsAncestor = Callable[[str, str], bool]
Suites = Mapping[str, Mapping[str, Any]]
Tests = Mapping[str, Mapping[str, Any]]
IgnoredTests = Mapping[str, Mapping[str, Any]]

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
BROAD_REMOTE_CASE_PRODUCER = "broad-remote-gate.py v1"


def validate_evidence_scope(
    *,
    fail: Fail,
    path: Path,
    record: Mapping[str, Any],
    revision_exists: RevisionExists | None = None,
    suites: Suites | None = None,
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
        producer = str(record.get("producer", ""))
        authorization_suite = _authorization_suite_id(record, scope)
        if not (
            PROVE_PRODUCER_RE.fullmatch(producer)
            or _producer_is_approved_by_suite(
                record,
                suite_id=authorization_suite,
                suites=suites or {},
            )
        ):
            fail(
                path,
                f"{evidence_id} proof_scope targeted producer must be approved by "
                f"suite {authorization_suite or '<invalid>'}",
            )
        return

    if proof_kind != "none":
        fail(path, f"{evidence_id} proof_scope {scope} requires proof_kind none")
    authorization_suite = _authorization_suite_id(record, scope)
    if not _producer_is_approved_by_suite(
        record,
        suite_id=authorization_suite,
        suites=suites or {},
    ):
        fail(
            path,
            f"{evidence_id} proof_scope {scope} producer must be approved by "
            f"suite {authorization_suite or '<invalid>'}",
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
    suites: Suites | None = None,
    tests: Tests | None = None,
    ignored_tests: IgnoredTests | None = None,
    root: Path | None = None,
    is_ancestor: IsAncestor | None = None,
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
            suites=suites or {},
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
            suites=suites or {},
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

    targeted = [
        record
        for record in referenced
        if _is_targeted_closing_proof(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            suites=suites or {},
        )
    ]
    if not targeted:
        fail(
            path,
            f"{invariant_id} proof_level {proof_level} requires targeted "
            "green/lock proof",
        )

    if proof_level == "G1":
        return

    broad = [
        record
        for record in referenced
        if _is_broad_remote_gate(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            suites=suites or {},
            tests=tests or {},
            ignored_tests=ignored_tests or {},
            root=root or ROOT,
        )
    ]
    ancestry = is_ancestor or _is_ancestor
    causal_broad = [
        broad_record
        for broad_record in broad
        if any(
            _same_or_descendant(
                targeted_record,
                broad_record,
                is_ancestor=ancestry,
            )
            for targeted_record in targeted
        )
    ]
    if not causal_broad:
        fail(
            path,
            f"{invariant_id} proof_level {proof_level} requires broad remote "
            "gate evidence at or after targeted proof",
        )

    if proof_level != "R1":
        return

    release = [
        record
        for record in referenced
        if _is_release_public(
            record,
            invariant_id=invariant_id,
            invariant_tests=invariant.get("tests", []),
            suites=suites or {},
        )
    ]
    if not any(
        _same_or_descendant(
            broad_record,
            release_record,
            is_ancestor=ancestry,
        )
        for broad_record in causal_broad
        for release_record in release
    ):
        fail(
            path,
            f"{invariant_id} proof_level R1 requires release/public evidence "
            "at or after broad gate",
        )


def _is_targeted_closing_proof(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    suites: Suites,
) -> bool:
    producer = str(record.get("producer", ""))
    return (
        record.get("proof_scope") == "targeted"
        and isinstance(record.get("proof_kind"), str)
        and record.get("proof_kind") in CLOSING_PROOF_KINDS
        and _has_clean_commit(record)
        and _back_links(record, invariant_id, invariant_tests, require_all_tests=False)
        and (
            bool(PROVE_PRODUCER_RE.fullmatch(producer))
            or _producer_is_approved_by_record_suite(record, suites=suites)
        )
    )


def _is_targeted_red_proof(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    suites: Suites,
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
        and (
            bool(PROVE_PRODUCER_RE.fullmatch(producer))
            or _producer_is_approved_by_record_suite(record, suites=suites)
        )
    )


def _is_broad_remote_gate(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    suites: Suites,
    tests: Tests,
    ignored_tests: IgnoredTests,
    root: Path,
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
    if not _producer_is_approved_by_record_suite(record, suites=suites):
        return False
    if record.get("kind") == "committed_file":
        platform = record.get("platform")
        if not isinstance(platform, str) or not TRUST_BUILDER_PLATFORM_RE.fullmatch(
            platform
        ):
            return False
    if not _back_links(record, invariant_id, invariant_tests, require_all_tests=True):
        return False
    if record.get("producer") == BROAD_REMOTE_CASE_PRODUCER:
        return _case_execution_matches_current_tests(
            record,
            invariant_tests=invariant_tests,
            tests=tests,
            ignored_tests=ignored_tests,
            suite_id=suite_id,
            root=root,
        )
    return True


def _case_execution_matches_current_tests(
    record: Mapping[str, Any],
    *,
    invariant_tests: Any,
    tests: Tests,
    ignored_tests: IgnoredTests,
    suite_id: str,
    root: Path,
) -> bool:
    executed = record.get("executed_tests")
    if not isinstance(executed, list):
        return False
    by_id = {
        entry.get("test_id"): entry
        for entry in executed
        if isinstance(entry, Mapping) and isinstance(entry.get("test_id"), str)
    }
    if not isinstance(invariant_tests, list) or any(
        not isinstance(test_id, str) for test_id in invariant_tests
    ):
        return False
    linked_tests = record.get("linked_tests")
    if (
        not isinstance(linked_tests, list)
        or not linked_tests
        or any(not isinstance(test_id, str) for test_id in linked_tests)
        or len(linked_tests) != len(set(linked_tests))
        or not set(invariant_tests) <= set(linked_tests)
        or len(by_id) != len(executed)
        or set(by_id) != set(linked_tests)
    ):
        return False
    for test_id in linked_tests:
        test = tests.get(test_id) if isinstance(test_id, str) else None
        entry = by_id.get(test_id)
        if not isinstance(test, Mapping) or not isinstance(entry, Mapping):
            return False
        suite_tiers = test.get("suite_tiers")
        discovery_id = test.get("discovery_id")
        if (
            test.get("status") not in RUNNABLE_TEST_STATUSES
            or not isinstance(suite_tiers, list)
            or any(not isinstance(tier, str) for tier in suite_tiers)
            or suite_id not in suite_tiers
            or test.get("discovery_source_kind")
            not in {"rust_integration_test", "rust_unit_test"}
            or not isinstance(discovery_id, str)
            or not discovery_id
            or _is_currently_ignored(
                test_id=test_id,
                discovery_id=discovery_id,
                ignored_tests=ignored_tests,
            )
            or entry.get("discovery_id") != discovery_id
            or entry.get("discovery_source_kind")
            != test.get("discovery_source_kind")
            or entry.get("command") != test.get("command")
            or entry.get("case_file_digest") != test.get("case_file_digest")
            or entry.get("exit_status") != 0
            or not _matches_current_case_contract(entry=entry, test=test, root=root)
        ):
            return False
    return True


def _is_currently_ignored(
    *,
    test_id: str,
    discovery_id: str,
    ignored_tests: IgnoredTests,
) -> bool:
    return any(
        isinstance(record, Mapping)
        and (
            record.get("test_id") == test_id
            or record.get("discovery_id") == discovery_id
        )
        for record in ignored_tests.values()
    )


def _matches_current_case_contract(
    *,
    entry: Mapping[str, Any],
    test: Mapping[str, Any],
    root: Path,
) -> bool:
    relative = test.get("case_file")
    expected_digest = test.get("case_file_digest")
    if (
        not isinstance(relative, str)
        or not relative
        or "\\" in relative
        or not isinstance(expected_digest, str)
    ):
        return False
    parsed = PurePosixPath(relative)
    if parsed.is_absolute() or ".." in parsed.parts or "." in parsed.parts:
        return False
    case_path = root / parsed
    try:
        case_path.resolve().relative_to(root.resolve())
        actual_digest = "sha256:" + hashlib.sha256(case_path.read_bytes()).hexdigest()
        contract = load_case_contract(case_path)
    except (OSError, ValueError, CaseArtifactContractError):
        return False
    if actual_digest != expected_digest or entry.get("case_file_digest") != actual_digest:
        return False
    if not contract.case_ids or len(contract.case_ids) != len(set(contract.case_ids)):
        return False
    summary = entry.get("per_case_summary")
    if not isinstance(summary, list) or not summary:
        return False
    summary_ids: list[str] = []
    for item in summary:
        if not isinstance(item, str) or not item.endswith(":passed"):
            return False
        case_id = item.removesuffix(":passed")
        if not case_id:
            return False
        summary_ids.append(case_id)
    return len(summary_ids) == len(set(summary_ids)) and set(summary_ids) == set(
        contract.case_ids
    )


def _is_release_public(
    record: Mapping[str, Any],
    *,
    invariant_id: str,
    invariant_tests: Any,
    suites: Suites,
) -> bool:
    return (
        record.get("proof_scope") == "release_public"
        and record.get("proof_kind") == "none"
        and record.get("kind") == "release_object"
        and record.get("suite_id") in (None, "release")
        and _has_clean_commit(record)
        and _producer_is_approved_by_suite(
            record,
            suite_id=_authorization_suite_id(record, "release_public"),
            suites=suites,
        )
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


def _authorization_suite_id(
    record: Mapping[str, Any], proof_scope: str
) -> str | None:
    suite_id = record.get("suite_id")
    if proof_scope == "release_public" and suite_id is None:
        return "release"
    return suite_id if isinstance(suite_id, str) and suite_id else None


def _producer_is_approved_by_record_suite(
    record: Mapping[str, Any], *, suites: Suites
) -> bool:
    suite_id = record.get("suite_id")
    return _producer_is_approved_by_suite(
        record,
        suite_id=suite_id if isinstance(suite_id, str) and suite_id else None,
        suites=suites,
    )


def _producer_is_approved_by_suite(
    record: Mapping[str, Any],
    *,
    suite_id: str | None,
    suites: Suites,
) -> bool:
    if suite_id is None:
        return False
    suite = suites.get(suite_id)
    if not isinstance(suite, Mapping):
        return False
    approved = suite.get("approved_proof_producers")
    if not isinstance(approved, list) or any(
        not isinstance(producer, str) for producer in approved
    ):
        return False
    return str(record.get("producer", "")) in approved


def _same_or_descendant(
    earlier: Mapping[str, Any],
    later: Mapping[str, Any],
    *,
    is_ancestor: IsAncestor,
) -> bool:
    earlier_commit = str(earlier.get("commit", ""))
    later_commit = str(later.get("commit", ""))
    if not (
        CLEAN_FULL_COMMIT_RE.fullmatch(earlier_commit)
        and CLEAN_FULL_COMMIT_RE.fullmatch(later_commit)
    ):
        return False
    return earlier_commit == later_commit or is_ancestor(earlier_commit, later_commit)


def _revision_exists(revision: str) -> bool:
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{revision}^{{commit}}"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def _is_ancestor(ancestor: str, descendant: str) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0
