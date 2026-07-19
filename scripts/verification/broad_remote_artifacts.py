"""Same-run case-artifact contract for broad remote evidence."""

from __future__ import annotations

import hashlib
import json
import re
import shlex
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .proof_case_artifacts import (
    CaseArtifactContractError,
    load_case_contract,
    validate_case_artifact,
)


REMOTE_ROOT = "/home/johannes/projects/trust-platform"
REMOTE_ARTIFACT_DIR = f"{REMOTE_ROOT}/target/gate-artifacts/cases"
REMOTE_CARGO_TARGET_DIR = "/home/johannes/.cache/codex-targets/trust-platform-gate"
REMOTE_TMPDIR = "/home/johannes/.cache/codex-targets/trust-platform-gate-tmp"
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


class BroadRemoteArtifactError(RuntimeError):
    pass


def execution_shell(test_id: str, test: Mapping[str, Any], run_id: str) -> str:
    """Build the fixed-root remote command with a fresh verification stamp."""

    case_file, case_file_digest, command = _test_contract(test_id, test)
    artifact_path = remote_artifact_path(test_id)
    assignments = {
        "CARGO_TARGET_DIR": REMOTE_CARGO_TARGET_DIR,
        "TMPDIR": REMOTE_TMPDIR,
        "TRUST_VERIFY_TEST_ID": test_id,
        "TRUST_VERIFY_RUN_ID": run_id,
        "TRUST_VERIFY_CASE_FILE_DIGEST": case_file_digest,
        "TRUST_VERIFY_ARTIFACT_DIR": REMOTE_ARTIFACT_DIR,
    }
    environment = " ".join(
        f"{name}={shlex.quote(value)}" for name, value in assignments.items()
    )
    del case_file
    return (
        f"cd {shlex.quote(REMOTE_ROOT)} && "
        f"rm -f -- {shlex.quote(artifact_path)} && "
        f"{environment} {command}"
    )


def artifact_read_shell(test_id: str) -> str:
    return (
        f"cd {shlex.quote(REMOTE_ROOT)} && "
        f"cat -- {shlex.quote(remote_artifact_path(test_id))}"
    )


def artifact_cleanup_shell(test_ids: list[str]) -> str:
    paths = " ".join(shlex.quote(remote_artifact_path(test_id)) for test_id in test_ids)
    return f"cd {shlex.quote(REMOTE_ROOT)} && rm -f -- {paths}"


def remote_artifact_path(test_id: str) -> str:
    if not test_id or not re.fullmatch(r"[A-Z0-9_]+", test_id):
        raise BroadRemoteArtifactError(f"invalid catalog test id {test_id!r}")
    return f"{REMOTE_ARTIFACT_DIR}/{test_id}.json"


def validate_execution_artifact(
    *,
    root: Path,
    test_id: str,
    test: Mapping[str, Any],
    run_id: str,
    raw_artifact: str,
) -> dict[str, Any]:
    """Validate positive execution and return the closed evidence entry."""

    case_file, case_file_digest, command = _test_contract(test_id, test)
    try:
        artifact = json.loads(raw_artifact)
    except (TypeError, json.JSONDecodeError) as exc:
        raise BroadRemoteArtifactError(
            f"{test_id} did not produce a readable case artifact: {exc}"
        ) from exc
    if not isinstance(artifact, dict):
        raise BroadRemoteArtifactError(f"{test_id} case artifact is not an object")
    try:
        case_contract = load_case_contract(root / case_file)
        failed, blocked, summary = validate_case_artifact(
            artifact=artifact,
            expected_test_id=test_id,
            expected_case_file=case_file,
            expected_run_id=run_id,
            expected_artifact_dir=REMOTE_ARTIFACT_DIR,
            expected_case_file_digest=case_file_digest,
            expected_case_ids=case_contract.case_ids,
            expected_case_provenance_kind=case_contract.provenance_kind,
            expected_trace_definition_digest=case_contract.trace_definition_digest,
        )
    except CaseArtifactContractError as exc:
        raise BroadRemoteArtifactError(f"{test_id} case artifact is invalid: {exc}") from exc
    if failed or blocked or any(not item.endswith(":passed") for item in summary):
        raise BroadRemoteArtifactError(
            f"{test_id} broad execution did not pass every committed case"
        )
    discovery_id = test.get("discovery_id")
    source_kind = test.get("discovery_source_kind")
    if not isinstance(discovery_id, str) or not isinstance(source_kind, str):
        raise BroadRemoteArtifactError(f"{test_id} lacks reviewed discovery identity")
    return {
        "test_id": test_id,
        "discovery_id": discovery_id,
        "discovery_source_kind": source_kind,
        "command": command,
        "run_id": run_id,
        "case_file_digest": case_file_digest,
        "case_artifact_digest": "sha256:"
        + hashlib.sha256(raw_artifact.encode("utf-8")).hexdigest(),
        "per_case_summary": summary,
        "exit_status": 0,
    }


def validate_case_backed_test(
    *, root: Path, test_id: str, test: Mapping[str, Any]
) -> None:
    """Reject tests that cannot emit positive same-run execution identity."""

    case_file, case_file_digest, _command = _test_contract(test_id, test)
    path = root / case_file
    try:
        actual_digest = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
        load_case_contract(path)
    except (OSError, CaseArtifactContractError) as exc:
        raise BroadRemoteArtifactError(
            f"{test_id} case contract cannot be loaded: {exc}"
        ) from exc
    if actual_digest != case_file_digest:
        raise BroadRemoteArtifactError(
            f"{test_id} case_file_digest does not match its committed case file"
        )


def _test_contract(test_id: str, test: Mapping[str, Any]) -> tuple[str, str, str]:
    case_file = test.get("case_file")
    case_file_digest = test.get("case_file_digest")
    command = test.get("command")
    if not isinstance(case_file, str) or not case_file:
        raise BroadRemoteArtifactError(
            f"{test_id} broad evidence requires a case-file-backed test"
        )
    if not isinstance(case_file_digest, str) or not DIGEST_RE.fullmatch(
        case_file_digest
    ):
        raise BroadRemoteArtifactError(f"{test_id} has an invalid case_file_digest")
    if (
        not isinstance(command, str)
        or not command
        or "\n" in command
        or "\r" in command
        or "\0" in command
    ):
        raise BroadRemoteArtifactError(f"{test_id} has an unsafe catalog command")
    return case_file, case_file_digest, command
