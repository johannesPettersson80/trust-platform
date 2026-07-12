"""Proof-evidence relationship checks for verification metadata."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path
from typing import Any, Callable

from .constants import PROVE_PRODUCER_RE, ROOT
from ..proof_contract import (
    PROOF_CONTRACT_VERSION,
    ProofContractError,
    proof_contract_digest,
)


Fail = Callable[[Path, str], None]
RevisionExists = Callable[[str], bool]
IsAncestor = Callable[[str, str], bool]
FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
PROOF_KINDS = {"red", "green", "protective_red", "lock_baseline", "lock_compare"}
CANONICAL_PROOF_PATH = "verification/evidence-index.toml"


def validate_proof_contract_binding(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    tests: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
) -> None:
    """Bind every proof-producing record to its complete live metadata contract."""

    proof_kind = record.get("proof_kind")
    if proof_kind not in PROOF_KINDS:
        return
    evidence_id = str(record.get("id", "<unknown>"))
    version = record.get("proof_contract_version")
    if version is None:
        fail(path, f"{evidence_id} missing proof_contract_version")
    elif version != PROOF_CONTRACT_VERSION:
        fail(
            path,
            f"{evidence_id} unsupported proof_contract_version {version!r}; "
            f"expected {PROOF_CONTRACT_VERSION!r}",
        )
    _current_contract_digest(
        fail=fail,
        path=path,
        record=record,
        tests=tests,
        invariants=invariants,
    )


def validate_proof_provenance(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    evidence: dict[str, dict[str, Any]],
    revision_exists: RevisionExists | None = None,
    is_ancestor: IsAncestor | None = None,
) -> None:
    """Require durable clean revisions and a real red-before-green history."""

    if record.get("proof_kind") not in PROOF_KINDS:
        return
    evidence_id = str(record.get("id", "<unknown>"))
    revision = record.get("commit")
    if not isinstance(revision, str) or not FULL_COMMIT_RE.fullmatch(revision):
        fail(path, f"{evidence_id} proof requires a clean full 40-hex commit")
        return
    exists = revision_exists or _revision_exists
    ancestor = is_ancestor or _is_ancestor
    if not exists(revision):
        fail(path, f"{evidence_id} proof commit {revision} does not resolve to a commit")
        return

    producer = str(record.get("producer", ""))
    if PROVE_PRODUCER_RE.match(producer):
        if record.get("kind") != "committed_file":
            fail(path, f"{evidence_id} prove.py proof kind must be committed_file")
        if record.get("path") != CANONICAL_PROOF_PATH:
            fail(path, f"{evidence_id} prove.py proof path must be {CANONICAL_PROOF_PATH}")

    if record.get("proof_kind") != "green":
        return
    paired = evidence.get(record.get("paired_red_evidence"))
    if not isinstance(paired, dict):
        return
    red_revision = paired.get("commit")
    if not isinstance(red_revision, str) or not FULL_COMMIT_RE.fullmatch(red_revision):
        return
    if red_revision == revision:
        fail(path, f"{evidence_id} red and green proof must use distinct commits")
    elif not ancestor(red_revision, revision):
        fail(path, f"{evidence_id} red commit {red_revision} is not an ancestor of green commit {revision}")


def _revision_exists(revision: str) -> bool:
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{revision}^{{commit}}"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def _is_ancestor(before: str, after: str) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", before, after],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def validate_green_pairing(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    evidence: dict[str, dict[str, Any]],
    tests: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    approved_producers: set[str],
) -> None:
    if record.get("proof_kind") != "green":
        return

    for field in ("paired_red_evidence", "formerly_red_case_ids", "case_file_digest"):
        if field not in record:
            fail(path, f"{record['id']} green proof missing pairing field {field}")

    paired_id = record.get("paired_red_evidence")
    paired = evidence.get(paired_id)
    if paired is None:
        if paired_id:
            fail(path, f"{record['id']} pairs unknown red evidence {paired_id}")
        return

    if paired.get("proof_kind") not in {"red", "protective_red"}:
        fail(path, f"{record['id']} pairs to proof_kind {paired.get('proof_kind')!r}")
    producer = str(paired.get("producer", ""))
    if not (PROVE_PRODUCER_RE.match(producer) or producer in approved_producers):
        fail(path, f"{record['id']} paired red producer {producer!r} is not allowlisted")
    if paired.get("failure_kind") not in {"assertion_failure", "expected_rejection"}:
        fail(path, f"{record['id']} paired red failure_kind {paired.get('failure_kind')!r} cannot feed green")
    if paired.get("linked_tests") != record.get("linked_tests"):
        fail(path, f"{record['id']} linked_tests do not match paired red evidence")
    linked_tests = record.get("linked_tests")
    if not isinstance(linked_tests, list) or len(linked_tests) != 1:
        fail(path, f"{record['id']} green proof must link exactly one test")
        catalog_test = None
    else:
        catalog_test = tests.get(linked_tests[0])
        if catalog_test is None:
            fail(path, f"{record['id']} links unknown catalog test {linked_tests[0]}")
        elif record.get("case_file_digest") != catalog_test.get("case_file_digest"):
            fail(
                path,
                f"{record['id']} case_file_digest does not match catalog test {linked_tests[0]}",
            )
    if paired.get("case_file_digest") != record.get("case_file_digest"):
        fail(path, f"{record['id']} case_file_digest does not match paired red evidence")

    red_case_ids = paired.get("red_case_ids")
    if not isinstance(red_case_ids, list) or not red_case_ids:
        fail(path, f"{record['id']} paired red evidence has no red_case_ids")
        return
    formerly_red = record.get("formerly_red_case_ids")
    if formerly_red != red_case_ids:
        fail(path, f"{record['id']} formerly_red_case_ids do not match paired red red_case_ids")

    if not paired.get("per_case_summary"):
        fail(path, f"{record['id']} paired red evidence has no per_case_summary")
    if not record.get("per_case_summary"):
        fail(path, f"{record['id']} green evidence has no per_case_summary")
    if record.get("command_exit_status") != 0:
        fail(path, f"{record['id']} green proof command_exit_status must be 0")
    validate_green_summary(
        fail=fail,
        path=path,
        evidence_id=record["id"],
        summary=record.get("per_case_summary", []),
    )

    passed_cases = passed_case_ids(record.get("per_case_summary", []))
    for case_id in red_case_ids:
        if case_id not in passed_cases:
            fail(path, f"{record['id']} formerly red case {case_id} is not recorded as passed")
    _validate_paired_contract(
        fail=fail,
        path=path,
        record=record,
        paired=paired,
        tests=tests,
        invariants=invariants,
        pair_label="paired red",
    )


def validate_lock_pairing(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    evidence: dict[str, dict[str, Any]],
    tests: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    approved_producers: set[str],
) -> None:
    if record.get("proof_kind") != "lock_compare":
        return

    for field in (
        "paired_lock_baseline",
        "case_file_digest",
        "case_result_digest",
        "command_exit_status",
        "per_case_summary",
    ):
        if field not in record:
            fail(path, f"{record['id']} lock_compare proof missing field {field}")

    paired_id = record.get("paired_lock_baseline")
    paired = evidence.get(paired_id)
    if paired is None:
        if paired_id:
            fail(path, f"{record['id']} pairs unknown lock baseline {paired_id}")
        return

    if paired.get("proof_kind") != "lock_baseline":
        fail(path, f"{record['id']} pairs to proof_kind {paired.get('proof_kind')!r}")
    producer = str(paired.get("producer", ""))
    if not (PROVE_PRODUCER_RE.match(producer) or producer in approved_producers):
        fail(path, f"{record['id']} lock baseline producer {producer!r} is not allowlisted")
    if paired.get("linked_tests") != record.get("linked_tests"):
        fail(path, f"{record['id']} linked_tests do not match lock baseline")
    linked_tests = record.get("linked_tests")
    if not isinstance(linked_tests, list) or len(linked_tests) != 1:
        fail(path, f"{record['id']} lock_compare proof must link exactly one test")
    else:
        catalog_test = tests.get(linked_tests[0])
        if catalog_test is None:
            fail(path, f"{record['id']} links unknown catalog test {linked_tests[0]}")
        else:
            if record.get("case_file_digest") != catalog_test.get("case_file_digest"):
                fail(
                    path,
                    f"{record['id']} case_file_digest does not match catalog test {linked_tests[0]}",
                )
            if record.get("command") != catalog_test.get("command"):
                fail(path, f"{record['id']} command does not match catalog test {linked_tests[0]}")
            if paired.get("command") != catalog_test.get("command"):
                fail(path, f"{record['id']} lock baseline command does not match catalog test {linked_tests[0]}")
    if paired.get("case_file_digest") != record.get("case_file_digest"):
        fail(path, f"{record['id']} case_file_digest does not match lock baseline")
    if paired.get("command") != record.get("command"):
        fail(path, f"{record['id']} command does not match lock baseline")
    if paired.get("case_result_digest") != record.get("case_result_digest"):
        fail(path, f"{record['id']} case_result_digest does not match lock baseline")
    if paired.get("command_exit_status") != record.get("command_exit_status"):
        fail(path, f"{record['id']} command_exit_status does not match lock baseline")
    if paired.get("command_exit_status") != 0:
        fail(path, f"{record['id']} lock baseline command_exit_status must be 0")
    if record.get("command_exit_status") != 0:
        fail(path, f"{record['id']} lock_compare proof command_exit_status must be 0")
    if paired.get("per_case_summary") != record.get("per_case_summary"):
        fail(path, f"{record['id']} per_case_summary does not match lock baseline")
    validate_case_result_digest(
        fail=fail,
        path=path,
        evidence_id=record["id"],
        record=record,
    )
    validate_case_result_digest(
        fail=fail,
        path=path,
        evidence_id=str(paired.get("id", paired_id)),
        record=paired,
    )
    validate_lock_summary(
        fail=fail,
        path=path,
        evidence_id=record["id"],
        summary=record.get("per_case_summary", []),
    )
    _validate_paired_contract(
        fail=fail,
        path=path,
        record=record,
        paired=paired,
        tests=tests,
        invariants=invariants,
        pair_label="lock baseline",
    )


def _validate_paired_contract(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    paired: dict[str, Any],
    tests: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    pair_label: str,
) -> None:
    current = _current_contract_digest(
        fail=fail,
        path=path,
        record=record,
        tests=tests,
        invariants=invariants,
    )
    paired_current = _current_contract_digest(
        fail=fail,
        path=path,
        record=paired,
        tests=tests,
        invariants=invariants,
    )
    if record.get("proof_contract_digest") != paired.get("proof_contract_digest"):
        fail(
            path,
            f"{record.get('id', '<unknown>')} proof_contract_digest does not match {pair_label}",
        )
    if current is not None and paired_current is not None and current != paired_current:
        fail(
            path,
            f"{record.get('id', '<unknown>')} and {pair_label} resolve different current proof contracts",
        )


def _current_contract_digest(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    tests: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
) -> str | None:
    evidence_id = str(record.get("id", "<unknown>"))
    linked_tests = record.get("linked_tests")
    if not isinstance(linked_tests, list) or len(linked_tests) != 1:
        fail(path, f"{evidence_id} proof contract must link exactly one test")
        return None
    test_id = linked_tests[0]
    if not isinstance(test_id, str) or test_id not in tests:
        fail(path, f"{evidence_id} proof contract links unknown catalog test {test_id!r}")
        return None
    test = tests[test_id]
    current_invariants = test.get("invariants")
    if record.get("linked_invariants") != current_invariants:
        fail(path, f"{evidence_id} linked_invariants do not match current catalog row")
    try:
        expected = proof_contract_digest(test=test, invariants=invariants)
    except ProofContractError as exc:
        fail(path, f"{evidence_id} proof contract is invalid: {exc}")
        return None
    actual = record.get("proof_contract_digest")
    if actual is None:
        fail(path, f"{evidence_id} missing proof_contract_digest")
    elif actual != expected:
        fail(path, f"{evidence_id} proof_contract_digest does not match current catalog and invariants")
    return expected


def passed_case_ids(summary: Any) -> set[str]:
    if not isinstance(summary, list):
        return set()
    result: set[str] = set()
    for item in summary:
        if not isinstance(item, str):
            continue
        case_id, sep, status = item.partition(":")
        if sep and status == "passed":
            result.add(case_id)
    return result


def validate_green_summary(
    *,
    fail: Fail,
    path: Path,
    evidence_id: str,
    summary: Any,
) -> None:
    if not isinstance(summary, list):
        fail(path, f"{evidence_id} green per_case_summary must be a list")
        return
    for item in summary:
        if not isinstance(item, str):
            fail(path, f"{evidence_id} green per_case_summary has non-string item")
            continue
        case_id, sep, status = item.partition(":")
        if not sep:
            fail(path, f"{evidence_id} green per_case_summary item {item!r} is not case:result")
            continue
        if status not in {"passed", "failed", "skipped", "blocked"}:
            fail(path, f"{evidence_id} unknown case result {status!r} for case {case_id}")
        elif status != "passed":
            fail(path, f"{evidence_id} non-passing case {case_id}:{status} cannot close green proof")


def validate_lock_summary(
    *,
    fail: Fail,
    path: Path,
    evidence_id: str,
    summary: Any,
) -> None:
    if not isinstance(summary, list):
        fail(path, f"{evidence_id} lock per_case_summary must be a list")
        return
    if not summary:
        fail(path, f"{evidence_id} lock evidence has no per_case_summary")
        return
    for item in summary:
        if not isinstance(item, str):
            fail(path, f"{evidence_id} lock per_case_summary has non-string item")
            continue
        case_id, sep, status = item.partition(":")
        if not sep:
            fail(path, f"{evidence_id} lock per_case_summary item {item!r} is not case:result")
            continue
        if status not in {"passed", "failed", "skipped", "blocked"}:
            fail(path, f"{evidence_id} unknown case result {status!r} for case {case_id}")
        elif status != "passed":
            fail(path, f"{evidence_id} non-passing case {case_id}:{status} cannot close lock proof")


def validate_case_result_digest(
    *,
    fail: Fail,
    path: Path,
    evidence_id: str,
    record: dict[str, Any],
) -> None:
    command_exit_status = record.get("command_exit_status")
    summary = record.get("per_case_summary")
    if not isinstance(command_exit_status, int) or not isinstance(summary, list):
        return
    expected = case_result_digest(
        command_exit_status=command_exit_status,
        per_case_summary=summary,
    )
    if record.get("case_result_digest") != expected:
        fail(path, f"{evidence_id} case_result_digest does not match command_exit_status and per_case_summary")


def case_result_digest(*, command_exit_status: int, per_case_summary: list[Any]) -> str:
    payload = {
        "command_exit_status": command_exit_status,
        "per_case_summary": per_case_summary,
    }
    raw = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"
