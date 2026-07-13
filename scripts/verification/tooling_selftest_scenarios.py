"""Executable Phase 6A fixtures routed through their production catchers."""

from __future__ import annotations

import copy
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any, NamedTuple
from unittest import mock

from .metadata_validator.case_files import validate_case_record
from .metadata_validator.constants import ROOT
from .metadata_validator.core import Validator
from .metadata_validator.evidence_proof import validate_green_pairing
from .planner import risk_changes_from_matrices
from .prover import CommandRun, ProofError, ProofProducer, validate_case_artifact
from .test_catalog_rust import scan_rust_file
from .test_catalog_staleness import validate_catalog_staleness
from .tooling_selftest_spec_sources import SPEC_SOURCE_SCENARIO_HANDLERS


class ScenarioResult(NamedTuple):
    case_id: str
    assigned_layer: str
    expected_disposition: str
    actual_disposition: str
    expected_signal: str
    actual_signal: str
    matched: bool
    full_wiring_matched: bool


class RawResult(NamedTuple):
    disposition: str
    signal: str
    full_wiring_signal: str = ""
    forbidden_side_effect: bool = False


Mutation = Callable[[Validator], None]
Handler = Callable[[], RawResult]
_RESULT_CACHE: dict[str, ScenarioResult] = {}
_BASELINE_VALIDATOR: Validator | None = None


def execute_bypass_case(row: dict[str, Any]) -> ScenarioResult:
    cached = _RESULT_CACHE.get(row["id"])
    if cached is not None:
        return cached
    raw = SCENARIO_HANDLERS[row["executor"]]()
    signal_matched = row["expected_signal"] in raw.signal
    full_wiring_matched = (
        row["expected_signal"] in raw.full_wiring_signal
        if row["assigned_layer"].startswith("metadata_validator")
        else True
    )
    matched = (
        raw.disposition == row["expected_disposition"]
        and signal_matched
        and full_wiring_matched
        and not raw.forbidden_side_effect
    )
    result = ScenarioResult(
        case_id=row["id"],
        assigned_layer=row["assigned_layer"],
        expected_disposition=row["expected_disposition"],
        actual_disposition=raw.disposition,
        expected_signal=row["expected_signal"],
        actual_signal=raw.signal,
        matched=matched,
        full_wiring_matched=full_wiring_matched,
    )
    _RESULT_CACHE[row["id"]] = result
    return result


def metadata_known_good() -> RawResult:
    validator = _loaded_validator()
    validator.validate()
    messages = _messages(validator)
    if messages:
        return RawResult("reject", messages, messages)
    return RawResult("accept", "no validation failures", "no validation failures")


def metadata_missing_required_field() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].pop("owner"),
        "validate_tests",
    )


def metadata_unknown_status() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].update(
            status="fixture_unknown"
        ),
        "validate_tests",
    )


def metadata_stale_runnable_path() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].update(
            path="crates/trust-runtime/tests/p6a_missing.rs"
        ),
        "validate_tests",
    )


def metadata_unknown_invariant() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].update(
            invariants=["INV_UNKNOWN_P6A"]
        ),
        "validate_tests",
    )


def metadata_unknown_suite() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].update(
            suite_tiers=["SUITE_UNKNOWN_P6A"]
        ),
        "validate_tests",
    )


def metadata_schema_version() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].update(
            schema_version=999
        ),
        "validate_tests",
    )


def metadata_public_claim_without_proof_or_gap() -> RawResult:
    def mutate(validator: Validator) -> None:
        claim_id = "PUBLIC_CLAIM_P6A_WITHOUT_PROOF_OR_GAP"
        claim = copy.deepcopy(validator.spec_sources["PUBLIC_CLAIM_RUNTIME_WIRE_001"])
        claim["id"] = claim_id
        validator.spec_sources[claim_id] = claim
        invariant = validator.invariants["RELEASE_PLATFORM_MATRIX_001"]
        invariant["status"] = "unproven"
        invariant["spec"]["status"] = "specified"
        invariant["spec"]["source_refs"] = ["SPEC_RUNTIME_ENGINE_001", claim_id]
        invariant["oracle"] = {
            "kind": "trust_contract",
            "ref": "SPEC_RUNTIME_ENGINE_001",
        }
        invariant["spec_gap_refs"] = []
        invariant["tests"] = ["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"]
        invariant["evidence_refs"] = []
        invariant["coverage"]["cells"][0]["state"] = "covered"
        invariant["coverage"]["cells"][0].pop("spec_gap_ref", None)

    return _metadata_case(mutate, "validate_public_claim_links")


def metadata_ignored_durable_evidence() -> RawResult:
    relative = "target/gate-artifacts/verification/P6A_IGNORED_EVIDENCE.md"
    path = ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("ignored self-test fixture\n")

    def mutate(validator: Validator) -> None:
        evidence = validator.evidence["EVID_P1B_ADVERSARIAL_SELFTESTS_20260709"]
        evidence["path"] = relative

    try:
        return _metadata_case(mutate, "validate_evidence")
    finally:
        path.unlink(missing_ok=True)


def metadata_unknown_evidence() -> RawResult:
    return _metadata_case(
        lambda validator: validator.invariants["RT_SAFE_IO_WORKER_001"].update(
            evidence_refs=["EVID_UNKNOWN_P6A"]
        ),
        "validate_invariants",
    )


def metadata_mapped_empty_invariants() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"].update(
            invariants=[]
        ),
        "validate_tests",
    )


def catalog_stale_test_name() -> RawResult:
    validator = _loaded_validator()
    record = copy.deepcopy(validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"])
    batch = scan_rust_file(
        ROOT,
        ROOT / record["path"],
        package="trust-runtime",
        source_kind="rust_integration_test",
        command_prefix="cargo test -p trust-runtime --test bytecode_container",
        command_authority="conservative",
    )
    facts = [fact for fact in batch.facts if fact.stable_id == record["discovery_id"]]
    record["name"] = "stale_fixture_name"
    failures = validate_catalog_staleness(
        root=ROOT,
        tests={record["id"]: record},
        facts=facts,
    )
    return RawResult("reject" if failures else "accept", "\n".join(failures))


def metadata_validated_empty_evidence() -> RawResult:
    def mutate(validator: Validator) -> None:
        _promote_for_fixture(validator, "RELEASE_SOURCE_BUILD_OPENOT_001")
        validator.invariants["RELEASE_SOURCE_BUILD_OPENOT_001"]["evidence_refs"] = []

    return _metadata_case(mutate, "validate_invariants")


def metadata_safety_validated_gap_open() -> RawResult:
    return _metadata_case(
        lambda validator: _promote_for_fixture(validator, "RT_SAFE_DEADLINE_001"),
        "validate_invariants",
    )


def metadata_safety_validated_spec_gap() -> RawResult:
    return _metadata_case(
        lambda validator: _promote_for_fixture(validator, "RT_SAFE_FORCE_001"),
        "validate_invariants",
    )


def metadata_validated_low_proof() -> RawResult:
    def mutate(validator: Validator) -> None:
        _promote_for_fixture(validator, "RELEASE_SOURCE_BUILD_OPENOT_001")
        validator.invariants["RELEASE_SOURCE_BUILD_OPENOT_001"]["proof_level"] = "S1"

    return _metadata_case(mutate, "validate_invariants")


def metadata_decision_table_missing_behavior() -> RawResult:
    def mutate(validator: Validator) -> None:
        invariant = validator.invariants["VM_SEAM_SUBRANGE_001"]
        invariant["behavior"] = []
        invariant["tests"] = ["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"]
        invariant["coverage"]["cells"][0]["state"] = "covered"
        invariant["coverage"]["cells"][0].pop("spec_gap_ref", None)

    return _metadata_case(mutate, "validate_invariants")


def case_unknown_family() -> RawResult:
    failures: list[str] = []
    validate_case_record(
        fail=lambda _path, message: failures.append(message),
        path=ROOT / "verification/cases/bytecode_vm/P6A_FIXTURE.toml",
        test_id="TEST_P6A_CASE_FAMILY",
        case={
            "id": "CASE_P6A_UNKNOWN_FAMILY",
            "family": "fixture_unknown",
            "input": {"scenario": "FIXTURE"},
            "state": "blocked",
            "spec_gap_ref": "SPEC_GAP_BYTECODE_VALIDATOR_001",
        },
        invariant={"id": "INV_P6A", "behavior": [], "input": {}},
        spec_sources={},
        spec_gaps={"SPEC_GAP_BYTECODE_VALIDATOR_001": {"id": "SPEC_GAP_BYTECODE_VALIDATOR_001"}},
        seen_case_ids=set(),
    )
    return RawResult("reject" if failures else "accept", "\n".join(failures))


def metadata_stale_case_digest() -> RawResult:
    return _metadata_case(
        lambda validator: validator.tests["TEST_CASE_TABLE_VM_SEAM_VALID_001"].update(
            case_file_digest="sha256:stale"
        ),
        "validate_tests",
    )


def proof_skipped_case_artifact() -> RawResult:
    artifact = {
        "schema_version": 1,
        "test_id": "TEST_P6A",
        "case_file": "verification/cases/bytecode_vm/P6A.toml",
        "case_file_digest": "sha256:cases",
        "helper_version": "verification-cases v1",
        "case_provenance_kind": "generated_decision_table_v1",
        "trace_definition_digest": None,
        "trust_verify_test_id": "TEST_P6A",
        "trust_verify_run_id": "run-p6a",
        "trust_verify_case_file_digest": "sha256:cases",
        "trust_verify_artifact_dir": "target/p6a",
        "cases": [
            {
                "id": "CASE_P6A",
                "family": "happy_path",
                "result": "skipped",
                "spec_gap_ref": None,
                "observed_error": None,
                "observed_status": None,
                "state_delta": "unchanged",
                "before": None,
                "after": None,
            }
        ],
    }
    try:
        validate_case_artifact(
            artifact=artifact,
            expected_test_id="TEST_P6A",
            expected_case_file="verification/cases/bytecode_vm/P6A.toml",
            expected_run_id="run-p6a",
            expected_artifact_dir="target/p6a",
            expected_case_file_digest="sha256:cases",
            expected_case_ids=["CASE_P6A"],
            expected_case_provenance_kind="generated_decision_table_v1",
            expected_trace_definition_digest=None,
        )
    except ProofError as exc:
        return RawResult("reject", str(exc))
    return RawResult("accept", "no failure")


def evidence_high_risk_red_producer() -> RawResult:
    return _high_risk_producer_case("red")


def evidence_high_risk_green_producer() -> RawResult:
    return _high_risk_producer_case("green")


def evidence_green_missing_red_pair() -> RawResult:
    failures: list[str] = []
    record = {
        "id": "EVID_P6A_GREEN",
        "proof_kind": "green",
        "producer": "prove.py v1",
        "linked_tests": ["TEST_P6A"],
        "case_file_digest": "sha256:cases",
        "formerly_red_case_ids": ["CASE_P6A"],
        "per_case_summary": ["CASE_P6A:passed"],
        "command_exit_status": 0,
    }
    validate_green_pairing(
        fail=lambda _path, message: failures.append(message),
        path=ROOT / "verification/evidence-index.toml",
        record=record,
        evidence={},
        tests={"TEST_P6A": {"id": "TEST_P6A", "case_file_digest": "sha256:cases"}},
        invariants={},
        approved_producers=set(),
    )
    return RawResult("reject" if failures else "accept", "\n".join(failures))


def planner_risk_downgrade_without_decision() -> RawResult:
    changes = risk_changes_from_matrices(
        {"bytecode_vm"},
        {
            "bytecode_vm": {
                "id": "bytecode_vm",
                "risk_default": "maintenance",
                "high_risks": [],
            }
        },
        {
            "bytecode_vm": {
                "id": "bytecode_vm",
                "risk_default": "wrong_result",
                "high_risks": ["wrong_result", "silent_corruption"],
            }
        },
    )
    return RawResult("report" if changes else "accept", "\n".join(changes))


def proof_compile_error_as_red() -> RawResult:
    return _proof_red_classification(returncode=1, artifact=None, expected="compile_error")


def proof_harness_panic_as_red() -> RawResult:
    return _proof_red_classification(
        returncode=1,
        artifact={"cases": []},
        expected="harness_panic",
    )


def proof_assert_nothing_red() -> RawResult:
    return _proof_red_classification(returncode=0, artifact={}, expected="none")


def _metadata_case(mutate: Mutation, phase: str) -> RawResult:
    direct = _loaded_validator()
    mutate(direct)
    getattr(direct, phase)()
    direct_messages = _messages(direct)

    full = _loaded_validator()
    mutate(full)
    full.validate()
    full_messages = _messages(full)
    return RawResult(
        "reject" if direct_messages else "accept",
        direct_messages,
        full_messages,
    )


def _loaded_validator() -> Validator:
    global _BASELINE_VALIDATOR
    if _BASELINE_VALIDATOR is None:
        _BASELINE_VALIDATOR = Validator()
        _BASELINE_VALIDATOR.load_records()
        if _BASELINE_VALIDATOR.failures:
            raise AssertionError(
                "committed metadata failed during fixture load: "
                f"{_messages(_BASELINE_VALIDATOR)}"
            )
    return copy.deepcopy(_BASELINE_VALIDATOR)


def _messages(validator: Validator) -> str:
    return "\n".join(failure.message for failure in validator.failures)


def _promote_for_fixture(validator: Validator, invariant_id: str) -> None:
    record = validator.invariants[invariant_id]
    record["status"] = "validated"
    record["proof_level"] = "G1"
    record["tests"] = ["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"]
    record["evidence_refs"] = ["EVID_SOURCE_BUILD_OPENOT_ISSUE_93_20260708"]
    record["spec"]["status"] = "specified"


def _high_risk_producer_case(proof_kind: str) -> RawResult:
    def mutate(validator: Validator) -> None:
        record = validator.evidence["EVID_P1B_BYTECODE_VALIDATOR_MUTATION_SHARD_20260709"]
        record["proof_kind"] = proof_kind
        record["producer"] = "codex"

    return _metadata_case(mutate, "validate_evidence")


def _proof_red_classification(
    *, returncode: int, artifact: dict[str, Any] | None, expected: str
) -> RawResult:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        evidence_dir = root / "evidence"
        producer = ProofProducer(
            root=root,
            tests={
                "TEST_P6A": {
                    "id": "TEST_P6A",
                    "status": "mapped",
                    "command": "fixture-command",
                    "test_class": "failing_regression",
                }
            },
            ignored_tests={},
            evidence={},
            artifact_dir=root / "artifacts",
            evidence_dir=evidence_dir,
            revision_provider=lambda: "a" * 40,
            ancestry_checker=lambda _before, _after: True,
            validate_metadata=False,
        )
        run = CommandRun(
            returncode=returncode,
            trust_verify_run_id="run-p6a",
            artifact_path=None,
            artifact=artifact,
            case_artifact_digest=None,
            case_file_digest=None,
            case_result_digest="sha256:fixture",
            failed_case_ids=[],
            blocked_case_ids=[],
            per_case_summary=[],
        )
        with mock.patch.object(producer, "run_cataloged_command", return_value=run):
            try:
                producer.red("TEST_P6A")
            except ProofError as exc:
                no_evidence = not evidence_dir.exists() or not any(evidence_dir.iterdir())
                signal = exc.failure_kind if no_evidence else f"{exc.failure_kind}; evidence-created"
                return RawResult(
                    "reject",
                    signal,
                    forbidden_side_effect=not no_evidence,
                )
    return RawResult("accept", f"expected {expected} but proof was written")


SCENARIO_HANDLERS: dict[str, Handler] = {
    "metadata_known_good": metadata_known_good,
    "metadata_missing_required_field": metadata_missing_required_field,
    "metadata_unknown_status": metadata_unknown_status,
    "metadata_stale_runnable_path": metadata_stale_runnable_path,
    "metadata_unknown_invariant": metadata_unknown_invariant,
    "metadata_unknown_suite": metadata_unknown_suite,
    "metadata_schema_version": metadata_schema_version,
    "metadata_public_claim_without_proof_or_gap": metadata_public_claim_without_proof_or_gap,
    "metadata_ignored_durable_evidence": metadata_ignored_durable_evidence,
    "metadata_unknown_evidence": metadata_unknown_evidence,
    "metadata_mapped_empty_invariants": metadata_mapped_empty_invariants,
    "catalog_stale_test_name": catalog_stale_test_name,
    "metadata_validated_empty_evidence": metadata_validated_empty_evidence,
    "metadata_safety_validated_gap_open": metadata_safety_validated_gap_open,
    "metadata_safety_validated_spec_gap": metadata_safety_validated_spec_gap,
    "metadata_validated_low_proof": metadata_validated_low_proof,
    "metadata_decision_table_missing_behavior": metadata_decision_table_missing_behavior,
    "case_unknown_family": case_unknown_family,
    "metadata_stale_case_digest": metadata_stale_case_digest,
    "proof_skipped_case_artifact": proof_skipped_case_artifact,
    "evidence_high_risk_red_producer": evidence_high_risk_red_producer,
    "evidence_high_risk_green_producer": evidence_high_risk_green_producer,
    "evidence_green_missing_red_pair": evidence_green_missing_red_pair,
    "planner_risk_downgrade_without_decision": planner_risk_downgrade_without_decision,
    "proof_compile_error_as_red": proof_compile_error_as_red,
    "proof_harness_panic_as_red": proof_harness_panic_as_red,
    "proof_assert_nothing_red": proof_assert_nothing_red,
    **SPEC_SOURCE_SCENARIO_HANDLERS,
}
