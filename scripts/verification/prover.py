"""Red/green/lock proof producer for verification metadata.

P1B scope implements `prove.py red`, `prove.py green`, and behavior-lock
baseline/compare evidence.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import subprocess
import sys
import tomllib
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .metadata_validator.constants import PROVE_PRODUCER_RE, ROOT, VERIFICATION
from .metadata_validator.core import Validator
from .metadata_validator.integrity import test_counts_as_runnable
from .proof_case_artifacts import (
    CaseArtifactContractError,
    CaseProofContract,
    load_case_contract as load_case_contract_value,
    load_json_artifact as load_json_artifact_value,
    validate_case_artifact as validate_case_artifact_value,
)
from .proof_contract import (
    PROOF_CONTRACT_VERSION,
    ProofContractError,
    proof_contract_digest,
)
from .proof_output import (
    CANONICAL_EVIDENCE_INDEX,
    ProofOutputError,
    ProofRevisionSession,
    append_evidence_record,
    render_evidence_record,
)


EXIT_OK = 0
EXIT_NOT_RED = 2
EXIT_USAGE = 5
EXIT_METADATA_INVALID = 6
EXIT_PROOF_ERROR = 7
PRODUCER = "prove.py v1"


class MetadataValidationError(RuntimeError):
    pass


class ProofError(RuntimeError):
    def __init__(self, message: str, *, failure_kind: str) -> None:
        super().__init__(message)
        self.failure_kind = failure_kind


@dataclass(frozen=True)
class ProofResult:
    record: dict[str, Any]
    evidence_path: Path
    artifact_path: Path | None


@dataclass(frozen=True)
class CommandRun:
    returncode: int
    trust_verify_run_id: str
    artifact_path: Path | None
    artifact: dict[str, Any] | None
    case_artifact_digest: str | None
    case_file_digest: str | None
    case_result_digest: str
    failed_case_ids: list[str]
    blocked_case_ids: list[str]
    per_case_summary: list[str]


class ProofProducer:
    def __init__(
        self,
        *,
        root: Path = ROOT,
        tests: dict[str, dict[str, Any]] | None = None,
        invariants: dict[str, dict[str, Any]] | None = None,
        ignored_tests: dict[str, dict[str, Any]] | None = None,
        evidence: dict[str, dict[str, Any]] | None = None,
        approved_producers: set[str] | None = None,
        artifact_dir: Path | None = None,
        evidence_dir: Path | None = None,
        evidence_index_path: Path | None = None,
        run_id_factory: Any | None = None,
        revision_provider: Any | None = None,
        ancestry_checker: Any | None = None,
        command_timeout_seconds: float = 1800,
        validate_metadata: bool = True,
    ) -> None:
        self.root = root
        if validate_metadata:
            validator = load_validated_metadata()
            tests = validator.tests
            invariants = validator.invariants
            ignored_tests = validator.ignored_tests
            evidence = validator.evidence
            approved_producers = validator.approved_producers()
        self.tests = tests or {}
        self.invariants = invariants or {}
        self.ignored_tests = ignored_tests or {}
        self.ignored_tests_by_test_id = index_ignored_tests_by_test_id(self.ignored_tests)
        self.evidence = evidence or {}
        self.approved_producers = approved_producers or set()
        self.artifact_dir = artifact_dir or root / "target/gate-artifacts/cases"
        self.evidence_dir = evidence_dir or root / "target/gate-artifacts/prove"
        standalone_output = evidence_dir is not None and evidence_index_path is None
        self.evidence_index_path = (
            None
            if standalone_output
            else evidence_index_path or root / CANONICAL_EVIDENCE_INDEX
        )
        self.run_id_factory = run_id_factory or (lambda: uuid.uuid4().hex)
        self.proof_revision = ProofRevisionSession(
            root=root,
            revision_provider=revision_provider,
            ancestry_checker=ancestry_checker,
        )
        self.command_timeout_seconds = command_timeout_seconds

    def red(self, test_id: str) -> ProofResult:
        self.run_provenance_check(self.proof_revision.begin)
        test = self.lookup_runnable_test(test_id)
        run = self.run_cataloged_command(test_id, test)

        if run.returncode != 0 and run.failed_case_ids:
            return self.write_red_record(
                test=test,
                proof_kind=proof_kind_for_test(test),
                failure_kind="assertion_failure",
                run_id=run.trust_verify_run_id,
                command_exit_status=run.returncode,
                artifact_path=run.artifact_path,
                case_artifact_digest=run.case_artifact_digest,
                case_file_digest=str(run.case_file_digest),
                red_case_ids=run.failed_case_ids,
                per_case_summary=run.per_case_summary,
            )

        if run.returncode == 0 and run.failed_case_ids:
            raise ProofError(
                f"{test_id} reported failed cases but command exited 0",
                failure_kind="metadata_error",
            )
        if run.returncode != 0:
            raise ProofError(
                f"{test_id} command failed without a failed case artifact",
                failure_kind="harness_panic" if run.artifact else "compile_error",
            )
        raise ProofError(f"{test_id} is not red", failure_kind="none")

    def green(self, test_id: str, red_evidence_id: str) -> ProofResult:
        self.run_provenance_check(self.proof_revision.begin)
        test = self.lookup_runnable_test(test_id)
        red_evidence = self.lookup_red_evidence(red_evidence_id, test_id, test)
        self.run_provenance_check(self.proof_revision.require_red_before_current, red_evidence)
        run = self.run_cataloged_command(test_id, test)

        formerly_red_case_ids = list(red_evidence["red_case_ids"])
        if run.returncode == 0 and run.failed_case_ids:
            raise ProofError(
                f"{test_id} reported failed cases but command exited 0",
                failure_kind="metadata_error",
            )
        if run.returncode != 0 and run.failed_case_ids:
            raise ProofError(
                f"{test_id} still has failed cases {run.failed_case_ids}",
                failure_kind="assertion_failure",
            )
        if run.returncode != 0:
            raise ProofError(
                f"{test_id} command failed while proving green",
                failure_kind="harness_panic" if run.artifact else "compile_error",
            )
        if run.blocked_case_ids:
            raise ProofError(
                f"{test_id} cannot close green proof with blocked cases {run.blocked_case_ids}",
                failure_kind="metadata_error",
            )

        passed = passed_case_ids(run.per_case_summary)
        missing_passes = [case_id for case_id in formerly_red_case_ids if case_id not in passed]
        if missing_passes:
            raise ProofError(
                f"{test_id} formerly red cases are not green: {missing_passes}",
                failure_kind="metadata_error",
            )

        return self.write_green_record(
            test=test,
            red_evidence=red_evidence,
            run_id=run.trust_verify_run_id,
            command_exit_status=run.returncode,
            artifact_path=run.artifact_path,
            case_artifact_digest=run.case_artifact_digest,
            case_file_digest=str(run.case_file_digest),
            formerly_red_case_ids=formerly_red_case_ids,
            per_case_summary=run.per_case_summary,
        )

    def lock_baseline(self, test_id: str) -> ProofResult:
        self.run_provenance_check(self.proof_revision.begin)
        test = self.lookup_runnable_test(test_id)
        self.require_case_file_backed_test(test_id, test)
        run = self.run_cataloged_command(test_id, test)
        self.require_lock_clean_run(test_id, run)
        return self.write_lock_record(
            test=test,
            proof_kind="lock_baseline",
            paired_lock_baseline=None,
            run_id=run.trust_verify_run_id,
            command_exit_status=run.returncode,
            artifact_path=run.artifact_path,
            case_artifact_digest=run.case_artifact_digest,
            case_file_digest=run.case_file_digest,
            case_result_digest=run.case_result_digest,
            per_case_summary=run.per_case_summary,
        )

    def lock_compare(self, test_id: str, baseline_evidence_id: str) -> ProofResult:
        self.run_provenance_check(self.proof_revision.begin)
        test = self.lookup_runnable_test(test_id)
        self.require_case_file_backed_test(test_id, test)
        baseline = self.lookup_lock_baseline(baseline_evidence_id, test_id, test)
        run = self.run_cataloged_command(test_id, test)

        if run.returncode != baseline.get("command_exit_status"):
            raise ProofError(
                f"{test_id} command_exit_status changed from "
                f"{baseline.get('command_exit_status')!r} to {run.returncode!r}",
                failure_kind="metadata_error",
            )
        if run.blocked_case_ids:
            raise ProofError(
                f"{test_id} cannot close lock proof with blocked cases {run.blocked_case_ids}",
                failure_kind="metadata_error",
            )
        if run.case_result_digest != baseline.get("case_result_digest"):
            raise ProofError(
                f"{test_id} case result digest changed from "
                f"{baseline.get('case_result_digest')!r} to {run.case_result_digest!r}",
                failure_kind="metadata_error",
            )
        if run.per_case_summary != baseline.get("per_case_summary"):
            raise ProofError(
                f"{test_id} per-case results changed from "
                f"{baseline.get('per_case_summary')!r} to {run.per_case_summary!r}",
                failure_kind="metadata_error",
            )
        self.require_lock_clean_run(test_id, run)
        return self.write_lock_record(
            test=test,
            proof_kind="lock_compare",
            paired_lock_baseline=baseline["id"],
            run_id=run.trust_verify_run_id,
            command_exit_status=run.returncode,
            artifact_path=run.artifact_path,
            case_artifact_digest=run.case_artifact_digest,
            case_file_digest=run.case_file_digest,
            case_result_digest=run.case_result_digest,
            per_case_summary=run.per_case_summary,
        )

    def run_cataloged_command(self, test_id: str, test: dict[str, Any]) -> CommandRun:
        case_file = test.get("case_file")
        case_file_digest = test.get("case_file_digest")
        if bool(case_file) != bool(case_file_digest):
            raise ProofError(
                f"{test_id} must name both case_file and case_file_digest for proof",
                failure_kind="metadata_error",
            )

        case_contract = (
            load_case_contract(self.root / str(case_file)) if case_file else None
        )
        artifact_path = self.artifact_dir / f"{test_id}.json" if case_file else None
        if artifact_path and artifact_path.exists():
            artifact_path.unlink()

        run_id = str(self.run_id_factory())
        if not run_id:
            raise ProofError("run_id_factory returned an empty run id", failure_kind="metadata_error")

        env = os.environ.copy()
        env.update(
            {
                "TRUST_VERIFY_TEST_ID": test_id,
                "TRUST_VERIFY_RUN_ID": run_id,
                "TRUST_VERIFY_ARTIFACT_DIR": str(self.artifact_dir),
            }
        )
        if case_file_digest:
            env["TRUST_VERIFY_CASE_FILE_DIGEST"] = str(case_file_digest)

        command = str(test["command"])
        try:
            completed = subprocess.run(
                command,
                cwd=self.root,
                env=env,
                shell=True,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                timeout=self.command_timeout_seconds,
            )
        except subprocess.TimeoutExpired as exc:
            raise ProofError(
                f"{test_id} command timed out after {self.command_timeout_seconds:g}s",
                failure_kind="timeout",
            ) from exc

        artifact: dict[str, Any] | None = None
        failed_case_ids: list[str] = []
        blocked_case_ids: list[str] = []
        per_case_summary: list[str] = []
        case_artifact_digest: str | None = None
        if artifact_path and artifact_path.exists():
            case_artifact_digest = sha256_file(artifact_path)
            artifact = load_json_artifact(artifact_path)
            if case_contract is None:
                raise ProofError(
                    f"{test_id} case proof contract was not loaded",
                    failure_kind="metadata_error",
                )
            failed_case_ids, blocked_case_ids, per_case_summary = validate_case_artifact(
                artifact=artifact,
                expected_test_id=test_id,
                expected_case_file=str(case_file),
                expected_run_id=run_id,
                expected_artifact_dir=str(self.artifact_dir),
                expected_case_file_digest=str(case_file_digest),
                expected_case_ids=case_contract.case_ids,
                expected_case_provenance_kind=case_contract.provenance_kind,
                expected_trace_definition_digest=case_contract.trace_definition_digest,
            )

        if case_file and artifact is None:
            failure = "compile_error" if completed.returncode != 0 else "metadata_error"
            raise ProofError(
                f"{test_id} did not produce required case artifact {artifact_path}",
                failure_kind=failure,
            )

        return CommandRun(
            returncode=completed.returncode,
            trust_verify_run_id=run_id,
            artifact_path=artifact_path,
            artifact=artifact,
            case_artifact_digest=case_artifact_digest,
            case_file_digest=str(case_file_digest) if case_file_digest else None,
            case_result_digest=case_result_digest(
                command_exit_status=completed.returncode,
                per_case_summary=per_case_summary,
            ),
            failed_case_ids=failed_case_ids,
            blocked_case_ids=blocked_case_ids,
            per_case_summary=per_case_summary,
        )

    def require_lock_clean_run(self, test_id: str, run: CommandRun) -> None:
        if run.returncode != 0 and run.failed_case_ids:
            raise ProofError(
                f"{test_id} cannot close lock proof with failed cases {run.failed_case_ids}",
                failure_kind="metadata_error",
            )
        if run.returncode != 0:
            raise ProofError(
                f"{test_id} command failed while proving lock",
                failure_kind="harness_panic" if run.artifact else "compile_error",
            )
        if run.failed_case_ids:
            raise ProofError(
                f"{test_id} cannot close lock proof with failed cases {run.failed_case_ids}",
                failure_kind="metadata_error",
            )
        if run.blocked_case_ids:
            raise ProofError(
                f"{test_id} cannot close lock proof with blocked cases {run.blocked_case_ids}",
                failure_kind="metadata_error",
            )

    def lookup_runnable_test(self, test_id: str) -> dict[str, Any]:
        test = self.tests.get(test_id)
        if test is None:
            raise ProofError(f"unknown test {test_id}", failure_kind="metadata_error")
        ignored = self.ignored_tests_by_test_id.get(test_id)
        if ignored is not None:
            raise ProofError(
                f"{test_id} is listed in ignored-tests by {ignored.get('id', '<unknown>')}",
                failure_kind="metadata_error",
            )
        if not test_counts_as_runnable(test):
            raise ProofError(
                f"{test_id} is not runnable proof at status {test.get('status')!r}",
                failure_kind="metadata_error",
            )
        if "expected_red_failure_kind" in test:
            raise ProofError(
                "expected_red_failure_kind is reserved until expected-rejection "
                "proof has a validator-backed catalog contract",
                failure_kind="metadata_error",
            )
        return test

    def require_case_file_backed_test(self, test_id: str, test: dict[str, Any]) -> None:
        if not test.get("case_file") or not test.get("case_file_digest"):
            raise ProofError(
                f"{test_id} lock proof requires a catalog case_file and case_file_digest",
                failure_kind="metadata_error",
            )

    def lookup_red_evidence(
        self,
        red_evidence_id: str,
        test_id: str,
        test: dict[str, Any],
    ) -> dict[str, Any]:
        record = self.evidence.get(red_evidence_id)
        if record is None:
            record = load_generated_evidence(self.generated_evidence_path(red_evidence_id), red_evidence_id)
        if record.get("proof_kind") not in {"red", "protective_red"}:
            raise ProofError(
                f"{red_evidence_id} is not red/protective_red proof",
                failure_kind="metadata_error",
            )
        producer = str(record.get("producer", ""))
        if not (PROVE_PRODUCER_RE.match(producer) or producer in self.approved_producers):
            raise ProofError(
                f"{red_evidence_id} producer {producer!r} is not accepted for green proof",
                failure_kind="metadata_error",
            )
        if record.get("linked_tests") != [test_id]:
            raise ProofError(
                f"{red_evidence_id} linked_tests must be exactly [{test_id!r}]",
                failure_kind="metadata_error",
            )
        self.require_current_proof_contract(red_evidence_id, record, test)
        if record.get("case_file_digest") != test.get("case_file_digest"):
            raise ProofError(
                f"{red_evidence_id} case_file_digest does not match catalog row",
                failure_kind="metadata_error",
            )
        if record.get("failure_kind") not in {"assertion_failure", "expected_rejection"}:
            raise ProofError(
                f"{red_evidence_id} failure_kind {record.get('failure_kind')!r} cannot feed green",
                failure_kind="metadata_error",
            )
        red_case_ids = record.get("red_case_ids")
        if not isinstance(red_case_ids, list) or not red_case_ids:
            raise ProofError(f"{red_evidence_id} has no red_case_ids", failure_kind="metadata_error")
        if not record.get("per_case_summary"):
            raise ProofError(f"{red_evidence_id} has no per_case_summary", failure_kind="metadata_error")
        return record

    def lookup_lock_baseline(
        self,
        baseline_evidence_id: str,
        test_id: str,
        test: dict[str, Any],
    ) -> dict[str, Any]:
        record = self.evidence.get(baseline_evidence_id)
        if record is None:
            record = load_generated_evidence(
                self.generated_evidence_path(baseline_evidence_id),
                baseline_evidence_id,
            )
        if record.get("proof_kind") != "lock_baseline":
            raise ProofError(
                f"{baseline_evidence_id} is not lock_baseline proof",
                failure_kind="metadata_error",
            )
        producer = str(record.get("producer", ""))
        if not (PROVE_PRODUCER_RE.match(producer) or producer in self.approved_producers):
            raise ProofError(
                f"{baseline_evidence_id} producer {producer!r} is not accepted for lock proof",
                failure_kind="metadata_error",
            )
        if record.get("linked_tests") != [test_id]:
            raise ProofError(
                f"{baseline_evidence_id} linked_tests must be exactly [{test_id!r}]",
                failure_kind="metadata_error",
            )
        self.require_current_proof_contract(baseline_evidence_id, record, test)
        if record.get("command") != test.get("command"):
            raise ProofError(
                f"{baseline_evidence_id} command does not match catalog row",
                failure_kind="metadata_error",
            )
        if record.get("case_file_digest") != test.get("case_file_digest"):
            raise ProofError(
                f"{baseline_evidence_id} case_file_digest does not match catalog row",
                failure_kind="metadata_error",
            )
        if not record.get("per_case_summary"):
            raise ProofError(
                f"{baseline_evidence_id} has no per_case_summary",
                failure_kind="metadata_error",
            )
        if "case_result_digest" not in record:
            raise ProofError(
                f"{baseline_evidence_id} has no case_result_digest",
                failure_kind="metadata_error",
            )
        if "command_exit_status" not in record:
            raise ProofError(
                f"{baseline_evidence_id} has no command_exit_status",
                failure_kind="metadata_error",
            )
        if record.get("command_exit_status") != 0:
            raise ProofError(
                f"{baseline_evidence_id} command_exit_status must be 0",
                failure_kind="metadata_error",
            )
        expected_digest = case_result_digest(
            command_exit_status=int(record["command_exit_status"]),
            per_case_summary=list(record["per_case_summary"]),
        )
        if record.get("case_result_digest") != expected_digest:
            raise ProofError(
                f"{baseline_evidence_id} case_result_digest does not match command_exit_status and per_case_summary",
                failure_kind="metadata_error",
            )
        return record

    def require_current_proof_contract(
        self,
        evidence_id: str,
        record: dict[str, Any],
        test: dict[str, Any],
    ) -> None:
        version = record.get("proof_contract_version")
        if version != PROOF_CONTRACT_VERSION:
            raise ProofError(
                f"{evidence_id} proof_contract_version {version!r} does not match "
                f"{PROOF_CONTRACT_VERSION!r}",
                failure_kind="metadata_error",
            )
        linked_invariants = list(test.get("invariants", []))
        if record.get("linked_invariants") != linked_invariants:
            raise ProofError(
                f"{evidence_id} linked_invariants do not match current catalog row",
                failure_kind="metadata_error",
            )
        expected = self.current_proof_contract_digest(test)
        if record.get("proof_contract_digest") != expected:
            raise ProofError(
                f"{evidence_id} proof_contract_digest does not match current catalog and invariant records",
                failure_kind="metadata_error",
            )

    def current_proof_contract_digest(self, test: dict[str, Any]) -> str:
        try:
            return proof_contract_digest(test=test, invariants=self.invariants)
        except ProofContractError as exc:
            raise ProofError(str(exc), failure_kind="metadata_error") from exc

    def write_red_record(
        self,
        *,
        test: dict[str, Any],
        proof_kind: str,
        failure_kind: str,
        run_id: str,
        command_exit_status: int,
        artifact_path: Path | None,
        case_artifact_digest: str | None,
        case_file_digest: str | None,
        red_case_ids: list[str],
        per_case_summary: list[str],
    ) -> ProofResult:
        record_id = f"EVID_{test['id']}_RED"
        record, evidence_path = self.base_proof_record(
            test=test,
            record_id=record_id,
            title=f"Red proof for {test['id']}",
            generated_report_version="prove-red-v1",
            proof_kind=proof_kind,
            failure_kind=failure_kind,
            run_id=run_id,
            command_exit_status=command_exit_status,
            artifact_path=artifact_path,
            case_artifact_digest=case_artifact_digest,
            case_file_digest=case_file_digest,
        )
        record["red_case_ids"] = red_case_ids
        record["per_case_summary"] = per_case_summary
        return self.write_proof_record(record, evidence_path, artifact_path)

    def write_green_record(
        self,
        *,
        test: dict[str, Any],
        red_evidence: dict[str, Any],
        run_id: str,
        command_exit_status: int,
        artifact_path: Path | None,
        case_artifact_digest: str | None,
        case_file_digest: str | None,
        formerly_red_case_ids: list[str],
        per_case_summary: list[str],
    ) -> ProofResult:
        record_id = f"EVID_{test['id']}_GREEN"
        record, evidence_path = self.base_proof_record(
            test=test,
            record_id=record_id,
            title=f"Green proof for {test['id']}",
            generated_report_version="prove-green-v1",
            proof_kind="green",
            failure_kind="none",
            run_id=run_id,
            command_exit_status=command_exit_status,
            artifact_path=artifact_path,
            case_artifact_digest=case_artifact_digest,
            case_file_digest=case_file_digest,
        )
        record["paired_red_evidence"] = red_evidence["id"]
        record["formerly_red_case_ids"] = formerly_red_case_ids
        record["per_case_summary"] = per_case_summary
        return self.write_proof_record(record, evidence_path, artifact_path)

    def write_lock_record(
        self,
        *,
        test: dict[str, Any],
        proof_kind: str,
        paired_lock_baseline: str | None,
        run_id: str,
        command_exit_status: int,
        artifact_path: Path | None,
        case_artifact_digest: str | None,
        case_file_digest: str | None,
        case_result_digest: str,
        per_case_summary: list[str],
    ) -> ProofResult:
        suffix = "LOCK_BASELINE" if proof_kind == "lock_baseline" else "LOCK_COMPARE"
        record_id = f"EVID_{test['id']}_{suffix}"
        record, evidence_path = self.base_proof_record(
            test=test,
            record_id=record_id,
            title=f"{proof_kind} proof for {test['id']}",
            generated_report_version="prove-lock-v1",
            proof_kind=proof_kind,
            failure_kind="none",
            run_id=run_id,
            command_exit_status=command_exit_status,
            artifact_path=artifact_path,
            case_artifact_digest=case_artifact_digest,
            case_file_digest=case_file_digest,
        )
        record["per_case_summary"] = per_case_summary
        record["case_result_digest"] = case_result_digest
        if paired_lock_baseline is not None:
            record["paired_lock_baseline"] = paired_lock_baseline
        return self.write_proof_record(record, evidence_path, artifact_path)

    def base_proof_record(
        self,
        *,
        test: dict[str, Any],
        record_id: str,
        title: str,
        generated_report_version: str,
        proof_kind: str,
        failure_kind: str,
        run_id: str,
        command_exit_status: int,
        artifact_path: Path | None,
        case_artifact_digest: str | None,
        case_file_digest: str | None,
    ) -> tuple[dict[str, Any], Path]:
        evidence_path = self.evidence_index_path or self.evidence_dir / f"{record_id}.toml"
        revision = self.proof_revision.active_revision
        if revision is None:
            raise ProofError("proof source revision was not acquired", failure_kind="metadata_error")
        record: dict[str, Any] = {
            "schema_version": 1,
            "id": record_id,
            "title": title,
            "area": test.get("area", "verification"),
            "owner": test.get("owner", "verification"),
            "status": "mapped",
            "kind": "committed_file",
            "path": str(evidence_path.relative_to(self.root)),
            "command": test["command"],
            "commit": revision,
            "platform": platform.platform(),
            "date": dt.date.today().isoformat(),
            "suite_id": first_or_default(test.get("suite_tiers"), "veryquick"),
            "producer": PRODUCER,
            "generated_report_version": generated_report_version,
            "linked_invariants": list(test.get("invariants", [])),
            "linked_tests": [test["id"]],
            "last_reviewed": dt.date.today().isoformat(),
            "proof_kind": proof_kind,
            "proof_scope": "targeted",
            "failure_kind": failure_kind,
            "trust_verify_run_id": run_id,
            "command_exit_status": command_exit_status,
            "proof_contract_digest": self.current_proof_contract_digest(test),
            "proof_contract_version": PROOF_CONTRACT_VERSION,
        }
        if artifact_path is not None:
            record["case_artifact_path"] = str(artifact_path.relative_to(self.root))
        if case_artifact_digest is not None:
            record["case_artifact_digest"] = case_artifact_digest
        if case_file_digest is not None:
            record["case_file_digest"] = case_file_digest
        return record, evidence_path

    def write_proof_record(
        self,
        record: dict[str, Any],
        evidence_path: Path,
        artifact_path: Path | None,
    ) -> ProofResult:
        self.run_provenance_check(self.proof_revision.confirm)
        try:
            if self.evidence_index_path is not None:
                append_evidence_record(
                    root=self.root,
                    evidence_index_path=self.evidence_index_path,
                    record=record,
                )
            else:
                self.evidence_dir.mkdir(parents=True, exist_ok=True)
                evidence_path.write_text(render_evidence_record(record))
        except ProofOutputError as exc:
            raise ProofError(str(exc), failure_kind="metadata_error") from exc
        return ProofResult(record=record, evidence_path=evidence_path, artifact_path=artifact_path)

    def run_provenance_check(self, check: Any, *args: Any) -> Any:
        try:
            return check(*args)
        except ProofOutputError as exc:
            raise ProofError(str(exc), failure_kind="metadata_error") from exc

    def generated_evidence_path(self, evidence_id: str) -> Path:
        if self.evidence_index_path is not None:
            return self.evidence_index_path
        return self.evidence_dir / f"{evidence_id}.toml"


def load_validated_metadata() -> Validator:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if not validator.failures:
        return validator
    details = "\n".join(
        f"- {failure.path}: {failure.message}" for failure in validator.failures[:20]
    )
    more = "" if len(validator.failures) <= 20 else f"\n- ... {len(validator.failures) - 20} more"
    raise MetadataValidationError(f"prove.py metadata validation failed:\n{details}{more}")


def load_json_artifact(path: Path) -> dict[str, Any]:
    try:
        return load_json_artifact_value(path)
    except CaseArtifactContractError as exc:
        raise ProofError(str(exc), failure_kind="metadata_error") from exc


def load_generated_evidence(path: Path, evidence_id: str) -> dict[str, Any]:
    if not path.exists():
        raise ProofError(f"paired red evidence {evidence_id} not found at {path}", failure_kind="metadata_error")
    try:
        data = tomllib.loads(path.read_text())
    except Exception as exc:
        raise ProofError(f"failed to parse paired red evidence {path}: {exc}", failure_kind="metadata_error") from exc
    records = data.get("evidence")
    if not isinstance(records, list):
        raise ProofError(f"paired red evidence file {path} has no [[evidence]] table", failure_kind="metadata_error")
    for record in records:
        if isinstance(record, dict) and record.get("id") == evidence_id:
            return record
    raise ProofError(f"paired red evidence file {path} does not contain {evidence_id}", failure_kind="metadata_error")


def load_case_contract(path: Path) -> CaseProofContract:
    try:
        return load_case_contract_value(path)
    except CaseArtifactContractError as exc:
        raise ProofError(str(exc), failure_kind="metadata_error") from exc


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
    try:
        return validate_case_artifact_value(
            artifact=artifact,
            expected_test_id=expected_test_id,
            expected_case_file=expected_case_file,
            expected_run_id=expected_run_id,
            expected_artifact_dir=expected_artifact_dir,
            expected_case_file_digest=expected_case_file_digest,
            expected_case_ids=expected_case_ids,
            expected_case_provenance_kind=expected_case_provenance_kind,
            expected_trace_definition_digest=expected_trace_definition_digest,
        )
    except CaseArtifactContractError as exc:
        raise ProofError(str(exc), failure_kind="metadata_error") from exc


def passed_case_ids(summary: list[str]) -> set[str]:
    result: set[str] = set()
    for item in summary:
        case_id, sep, status = item.partition(":")
        if sep and status == "passed":
            result.add(case_id)
    return result


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return f"sha256:{digest}"


def case_result_digest(*, command_exit_status: int, per_case_summary: list[str]) -> str:
    payload = {
        "command_exit_status": command_exit_status,
        "per_case_summary": per_case_summary,
    }
    raw = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def proof_kind_for_test(test: dict[str, Any]) -> str:
    return "protective_red" if test.get("test_class") == "protective_red" else "red"


def index_ignored_tests_by_test_id(
    ignored_tests: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    """Index only explicit catalog mappings; registry IDs are not test IDs."""

    result: dict[str, dict[str, Any]] = {}
    for record_id, record in ignored_tests.items():
        test_id = record.get("test_id")
        if test_id is None:
            continue
        if not isinstance(test_id, str) or not test_id:
            raise MetadataValidationError(
                f"ignored-test record {record_id} has invalid optional test_id"
            )
        previous = result.get(test_id)
        if previous is not None:
            raise MetadataValidationError(
                f"ignored-test records {previous.get('id')} and {record_id} duplicate test_id {test_id}"
            )
        result[test_id] = record
    return result


def first_or_default(value: Any, default: str) -> str:
    if isinstance(value, list) and value:
        return str(value[0])
    return default


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Produce truST verification proof evidence.")
    subcommands = parser.add_subparsers(dest="command", required=True)
    red = subcommands.add_parser("red", help="run a cataloged test and record red proof")
    red.add_argument("--test", required=True, dest="test_id")
    green = subcommands.add_parser("green", help="run a cataloged test and record green proof")
    green.add_argument("--test", required=True, dest="test_id")
    green.add_argument("--red-evidence", required=True, dest="red_evidence_id")
    lock = subcommands.add_parser("lock", help="run or compare behavior-lock proof")
    lock.add_argument("--test", required=True, dest="test_id")
    lock_mode = lock.add_mutually_exclusive_group(required=True)
    lock_mode.add_argument("--baseline", action="store_true")
    lock_mode.add_argument("--compare", dest="baseline_evidence_id")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv or sys.argv[1:])
    except SystemExit as exc:
        return int(exc.code) if exc.code == 0 else EXIT_USAGE
    try:
        prover = ProofProducer()
        if args.command == "red":
            result = prover.red(args.test_id)
            print(f"red proof written: {result.evidence_path.relative_to(ROOT)}")
            return EXIT_OK
        if args.command == "green":
            result = prover.green(args.test_id, args.red_evidence_id)
            print(f"green proof written: {result.evidence_path.relative_to(ROOT)}")
            return EXIT_OK
        if args.baseline:
            result = prover.lock_baseline(args.test_id)
            print(f"lock baseline proof written: {result.evidence_path.relative_to(ROOT)}")
            return EXIT_OK
        result = prover.lock_compare(args.test_id, args.baseline_evidence_id)
    except MetadataValidationError as exc:
        print(str(exc), file=sys.stderr)
        return EXIT_METADATA_INVALID
    except ProofError as exc:
        print(f"prove.py {args.command} failed ({exc.failure_kind}): {exc}", file=sys.stderr)
        return EXIT_NOT_RED if exc.failure_kind == "none" else EXIT_PROOF_ERROR
    print(f"lock compare proof written: {result.evidence_path.relative_to(ROOT)}")
    return EXIT_OK
