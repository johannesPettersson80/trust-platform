"""Evidence-record structure, durability, proof, and promotion validation."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable, Mapping

from .constants import COMMIT_RE, EVIDENCE_KINDS, PROOF_KINDS, PROVE_PRODUCER_RE
from .evidence_proof import (
    validate_green_pairing,
    validate_lock_pairing,
    validate_proof_contract_binding,
    validate_proof_provenance,
)
from .promotion_evidence import validate_evidence_scope


Fail = Callable[[Path, str], None]
Require = Callable[[Path, dict[str, Any], list[str], str], None]
CheckCommon = Callable[[Path, dict[str, Any]], None]
CheckRefs = Callable[
    [Path, list[str], dict[str, dict[str, Any]], str, str], None
]
ValidateDurablePath = Callable[[Path, str, str], None]
LinksHighRisk = Callable[[dict[str, Any]], bool]


REQUIRED_EVIDENCE_FIELDS = [
    "schema_version",
    "id",
    "title",
    "area",
    "owner",
    "status",
    "kind",
    "commit",
    "platform",
    "date",
    "producer",
    "generated_report_version",
    "linked_invariants",
    "linked_tests",
    "last_reviewed",
]


def validate_evidence_records(
    *,
    fail: Fail,
    require: Require,
    check_common: CheckCommon,
    check_refs: CheckRefs,
    validate_durable_path: ValidateDurablePath,
    links_high_risk: LinksHighRisk,
    evidence: Mapping[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    tests: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
    suites: dict[str, dict[str, Any]],
    approved_producers: set[str],
) -> None:
    """Validate every evidence record through its owning contracts."""

    evidence_records = dict(evidence)
    for record in evidence_records.values():
        path = record["_path"]
        require(path, record, REQUIRED_EVIDENCE_FIELDS, "evidence")
        check_common(path, record)
        evidence_id = str(record.get("id", "<unknown>"))
        if record.get("kind") not in EVIDENCE_KINDS:
            fail(path, f"{evidence_id} has unknown evidence kind {record.get('kind')!r}")
        if not COMMIT_RE.match(str(record.get("commit", ""))):
            fail(path, f"{evidence_id} has invalid commit marker {record.get('commit')!r}")
        if record.get("proof_kind") and record["proof_kind"] not in PROOF_KINDS:
            fail(path, f"{evidence_id} has unknown proof_kind {record['proof_kind']!r}")
        if not record.get("suite_id") and not record.get("release_object"):
            fail(path, f"{evidence_id} must name suite_id or release_object")
        check_refs(
            path,
            record.get("linked_invariants", []),
            invariants,
            "invariant",
            evidence_id,
        )
        check_refs(path, record.get("linked_tests", []), tests, "test", evidence_id)
        check_refs(
            path,
            record.get("linked_spec_gaps", []),
            spec_gaps,
            "spec gap",
            evidence_id,
        )
        if record.get("suite_id") and record["suite_id"] not in suites:
            fail(path, f"{evidence_id} references unknown suite_id {record['suite_id']}")
        _validate_kind_fields(
            fail=fail,
            path=path,
            record=record,
            validate_durable_path=validate_durable_path,
        )
        if record.get("proof_kind") in {"red", "green", "lock_compare"} and links_high_risk(record):
            producer = record.get("producer")
            if not (
                PROVE_PRODUCER_RE.match(str(producer))
                or producer in approved_producers
            ):
                fail(
                    path,
                    f"{evidence_id} high-risk red/green proof producer "
                    f"{producer!r} is not allowlisted",
                )
        validate_green_pairing(
            fail=fail,
            path=path,
            record=record,
            evidence=evidence_records,
            tests=tests,
            invariants=invariants,
            approved_producers=approved_producers,
        )
        validate_lock_pairing(
            fail=fail,
            path=path,
            record=record,
            evidence=evidence_records,
            tests=tests,
            invariants=invariants,
            approved_producers=approved_producers,
        )
        validate_proof_contract_binding(
            fail=fail,
            path=path,
            record=record,
            tests=tests,
            invariants=invariants,
        )
        validate_proof_provenance(
            fail=fail,
            path=path,
            record=record,
            evidence=evidence_records,
        )
        validate_evidence_scope(
            fail=fail,
            path=path,
            record=record,
            suites=suites,
        )


def _validate_kind_fields(
    *,
    fail: Fail,
    path: Path,
    record: dict[str, Any],
    validate_durable_path: ValidateDurablePath,
) -> None:
    evidence_id = str(record.get("id", "<unknown>"))
    kind = record.get("kind")
    if kind == "committed_file":
        evidence_path = record.get("path")
        if not isinstance(evidence_path, str) or not evidence_path:
            fail(path, f"{evidence_id} committed_file evidence missing path")
        else:
            validate_durable_path(path, evidence_id, evidence_path)
    elif kind == "ci_artifact":
        _require_kind_fields(
            fail, path, evidence_id, record, ("workflow", "run_id", "artifact", "retention_days")
        )
    elif kind == "release_object":
        _require_kind_fields(fail, path, evidence_id, record, ("release_object", "url"))
    elif kind == "lab_report":
        _require_kind_fields(
            fail,
            path,
            evidence_id,
            record,
            ("path", "device_model", "firmware", "topology", "env_vars", "environment"),
        )


def _require_kind_fields(
    fail: Fail,
    path: Path,
    evidence_id: str,
    record: dict[str, Any],
    fields: tuple[str, ...],
) -> None:
    for field in fields:
        if field not in record:
            fail(path, f"{evidence_id} {record.get('kind')} evidence missing {field}")
