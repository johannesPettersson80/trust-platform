"""Case-file and same-run artifact binding for verification proof."""

from __future__ import annotations

import json
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .case_contract_fields import (
    CASE_ARTIFACT_CASE_FIELDS,
    CASE_ARTIFACT_HELPER_VERSION,
    CASE_ARTIFACT_ROOT_FIELDS,
)


CASE_RESULTS = {"passed", "failed", "skipped", "blocked"}
CASE_PROVENANCE_KINDS = {
    "generated_decision_table_v1",
    "hand_authored_state_machine_v1",
}


class CaseArtifactContractError(RuntimeError):
    pass


@dataclass(frozen=True)
class CaseProofContract:
    case_ids: list[str]
    provenance_kind: str
    trace_definition_digest: str | None


def load_json_artifact(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except Exception as exc:
        raise CaseArtifactContractError(
            f"failed to parse case artifact {path}: {exc}"
        ) from exc
    if not isinstance(value, dict):
        raise CaseArtifactContractError(f"case artifact {path} is not an object")
    return value


def load_case_contract(path: Path) -> CaseProofContract:
    try:
        data = tomllib.loads(path.read_text())
    except Exception as exc:
        raise CaseArtifactContractError(f"failed to parse case file {path}: {exc}") from exc
    cases = data.get("case", [])
    if not isinstance(cases, list):
        raise CaseArtifactContractError(f"case file {path} has no [[case]] table")
    case_ids: list[str] = []
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            raise CaseArtifactContractError(f"case file {path} has a case without id")
        case_ids.append(case["id"])

    provenance_kind = data.get(
        "case_provenance_kind", "generated_decision_table_v1"
    )
    if provenance_kind not in CASE_PROVENANCE_KINDS:
        raise CaseArtifactContractError(
            f"case file {path} has unknown case_provenance_kind {provenance_kind!r}"
        )
    trace_digest = data.get("trace_definition_digest")
    if provenance_kind == "generated_decision_table_v1":
        if trace_digest is not None:
            raise CaseArtifactContractError(
                f"generated case file {path} must not name trace_definition_digest"
            )
    elif not isinstance(trace_digest, str):
        raise CaseArtifactContractError(
            f"hand-authored case file {path} requires trace_definition_digest"
        )
    return CaseProofContract(
        case_ids=case_ids,
        provenance_kind=provenance_kind,
        trace_definition_digest=trace_digest,
    )


def validate_case_artifact(
    *,
    artifact: dict[str, Any],
    expected_test_id: str,
    expected_case_file: str,
    expected_run_id: str,
    expected_artifact_dir: str,
    expected_case_file_digest: str,
    expected_case_ids: list[str],
    expected_case_provenance_kind: str,
    expected_trace_definition_digest: str | None,
) -> tuple[list[str], list[str], list[str]]:
    require_exact_fields(artifact, CASE_ARTIFACT_ROOT_FIELDS, "root")
    require_equal(artifact, "schema_version", 1)
    require_equal(artifact, "test_id", expected_test_id)
    require_equal(artifact, "case_file", expected_case_file)
    require_equal(artifact, "case_file_digest", expected_case_file_digest)
    require_equal(artifact, "helper_version", CASE_ARTIFACT_HELPER_VERSION)
    require_equal(
        artifact, "case_provenance_kind", expected_case_provenance_kind
    )
    require_equal(
        artifact, "trace_definition_digest", expected_trace_definition_digest
    )
    require_equal(artifact, "trust_verify_test_id", expected_test_id)
    require_equal(artifact, "trust_verify_run_id", expected_run_id)
    require_equal(
        artifact, "trust_verify_case_file_digest", expected_case_file_digest
    )
    require_equal(artifact, "trust_verify_artifact_dir", expected_artifact_dir)
    cases = artifact.get("cases")
    if not isinstance(cases, list):
        raise CaseArtifactContractError("case artifact cases field is not an array")

    expected = set(expected_case_ids)
    seen: set[str] = set()
    failed: list[str] = []
    blocked: list[str] = []
    summary: list[str] = []
    for case in cases:
        if not isinstance(case, dict):
            raise CaseArtifactContractError(
                "case artifact contains a non-object case"
            )
        require_exact_fields(case, CASE_ARTIFACT_CASE_FIELDS, "case")
        case_id = case.get("id")
        result = case.get("result")
        if not isinstance(case_id, str):
            raise CaseArtifactContractError(
                "case artifact contains a case without id"
            )
        if case_id in seen:
            raise CaseArtifactContractError(f"duplicate case artifact id {case_id}")
        if case_id not in expected:
            raise CaseArtifactContractError(f"unknown case artifact id {case_id}")
        seen.add(case_id)
        if result not in CASE_RESULTS:
            raise CaseArtifactContractError(
                f"unknown case result {result!r} for case {case_id}"
            )
        if result == "skipped":
            raise CaseArtifactContractError(
                f"case {case_id} was skipped without waiver"
            )
        if result == "failed":
            failed.append(case_id)
        if result == "blocked":
            blocked.append(case_id)
        summary.append(f"{case_id}:{result}")
    missing = sorted(expected - seen)
    if missing:
        raise CaseArtifactContractError(f"case artifact missing cases {missing}")
    return failed, blocked, summary


def require_exact_fields(
    value: dict[str, Any], expected: frozenset[str], label: str
) -> None:
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        raise CaseArtifactContractError(
            f"case artifact {label} fields must be exactly {sorted(expected)}; "
            f"missing={missing}, unknown={unknown}"
        )


def require_equal(artifact: dict[str, Any], field: str, expected: Any) -> None:
    actual = artifact.get(field)
    if actual != expected:
        label = {
            "trust_verify_test_id": "TRUST_VERIFY_TEST_ID",
            "trust_verify_run_id": "TRUST_VERIFY_RUN_ID",
            "trust_verify_case_file_digest": "TRUST_VERIFY_CASE_FILE_DIGEST",
            "trust_verify_artifact_dir": "TRUST_VERIFY_ARTIFACT_DIR",
        }.get(field, field)
        raise CaseArtifactContractError(
            f"case artifact {label} mismatch: expected {expected!r}, actual {actual!r}"
        )
