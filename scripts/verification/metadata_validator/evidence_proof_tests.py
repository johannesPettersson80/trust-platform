"""Unit tests for evidence proof pairing validation."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.metadata_validator.evidence_proof import (
    validate_green_pairing,
    validate_lock_pairing,
    validate_proof_contract_binding,
    validate_proof_provenance,
)
from scripts.verification.proof_contract import PROOF_CONTRACT_VERSION, proof_contract_digest
from scripts.verification.prover import case_result_digest


class EvidenceProofTests(unittest.TestCase):
    def test_valid_green_pairing_has_no_failures(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record()})

        self.assertEqual(failures, [])

    def test_green_pair_rejects_mismatched_proof_contract_digests(self) -> None:
        failures = validate(
            green_record(proof_contract_digest="sha256:" + "f" * 64),
            {"EVID_RED": red_record()},
        )

        self.assert_contains_failure(failures, "proof_contract_digest does not match paired red")

    def test_green_rejects_non_red_pair(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(proof_kind="none")})

        self.assertIn("pairs to proof_kind 'none'", failures[0])

    def test_green_rejects_red_without_failed_case_ids(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(red_case_ids=[])})

        self.assertIn("paired red evidence has no red_case_ids", failures[0])

    def test_green_rejects_formerly_red_ids_that_do_not_pass(self) -> None:
        green = green_record(per_case_summary=["CASE_FAIL:failed"])

        failures = validate(green, {"EVID_RED": red_record()})

        self.assertIn("non-passing case CASE_FAIL:failed", failures[0])

    def test_green_rejects_missing_pairing_field(self) -> None:
        green = green_record()
        del green["paired_red_evidence"]

        failures = validate(green, {"EVID_RED": red_record()})

        self.assertIn("green proof missing pairing field paired_red_evidence", failures[0])

    def test_green_rejects_unknown_pair(self) -> None:
        failures = validate(green_record(), {})

        self.assertIn("pairs unknown red evidence EVID_RED", failures[0])

    def test_green_rejects_bad_paired_producer(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(producer="codex")})

        self.assertIn("paired red producer 'codex' is not allowlisted", failures[0])

    def test_green_rejects_bad_paired_failure_kind(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(failure_kind="compile_error")})

        self.assertIn("paired red failure_kind 'compile_error' cannot feed green", failures[0])

    def test_green_rejects_linked_test_mismatch(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(linked_tests=["OTHER_TEST"])})

        self.assertIn("linked_tests do not match paired red evidence", failures[0])

    def test_green_rejects_case_digest_mismatch(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(case_file_digest="sha256:other")})

        self.assertIn("case_file_digest does not match paired red evidence", failures[0])

    def test_green_rejects_formerly_red_ids_mismatch(self) -> None:
        failures = validate(green_record(formerly_red_case_ids=["OTHER_CASE"]), {"EVID_RED": red_record()})

        self.assertIn("formerly_red_case_ids do not match", failures[0])

    def test_green_rejects_missing_green_summary(self) -> None:
        failures = validate(green_record(per_case_summary=[]), {"EVID_RED": red_record()})

        self.assertIn("green evidence has no per_case_summary", failures[0])

    def test_green_rejects_missing_red_summary(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record(per_case_summary=[])})

        self.assertIn("paired red evidence has no per_case_summary", failures[0])

    def test_green_rejects_nonzero_exit_status(self) -> None:
        failures = validate(green_record(command_exit_status=1), {"EVID_RED": red_record()})

        self.assertIn("green proof command_exit_status must be 0", failures[0])

    def test_green_rejects_blocked_or_unknown_case_summary(self) -> None:
        for summary, expected in [
            (["CASE_FAIL:passed", "CASE_BLOCKED:blocked"], "non-passing case CASE_BLOCKED:blocked"),
            (["CASE_FAIL:passed", "CASE_OTHER:errored"], "unknown case result 'errored'"),
        ]:
            with self.subTest(summary=summary):
                failures = validate(green_record(per_case_summary=summary), {"EVID_RED": red_record()})

                self.assertIn(expected, failures[0])

    def test_green_rejects_catalog_digest_drift(self) -> None:
        tests = {"TEST_RED": {"id": "TEST_RED", "case_file_digest": "sha256:current"}}

        failures = validate(green_record(), {"EVID_RED": red_record()}, tests=tests)

        self.assertIn("does not match catalog test TEST_RED", failures[0])

    def test_historical_green_pair_uses_source_revision_catalog_digest(self) -> None:
        tests = {"TEST_RED": {"id": "TEST_RED", "case_file_digest": "sha256:current"}}
        green = green_record(proof_contract_binding="source_revision")
        red = red_record(proof_contract_binding="source_revision")

        failures = validate(green, {"EVID_RED": red}, tests=tests)

        self.assertFalse(
            any("does not match catalog test TEST_RED" in failure for failure in failures),
            failures,
        )

    def test_valid_lock_compare_has_no_failures(self) -> None:
        failures = validate_lock(lock_compare_record(), {"EVID_LOCK": lock_baseline_record()})

        self.assertEqual(failures, [])

    def test_lock_pair_rejects_mismatched_proof_contract_digests(self) -> None:
        failures = validate_lock(
            lock_compare_record(proof_contract_digest="sha256:" + "f" * 64),
            {"EVID_LOCK": lock_baseline_record()},
        )

        self.assert_contains_failure(failures, "proof_contract_digest does not match lock baseline")

    def test_lock_compare_rejects_missing_or_wrong_kind_baseline(self) -> None:
        missing = validate_lock(lock_compare_record(), {})
        self.assertIn("pairs unknown lock baseline EVID_LOCK", missing[0])

        wrong_kind = validate_lock(lock_compare_record(), {"EVID_LOCK": red_record()})
        self.assertIn("pairs to proof_kind 'red'", wrong_kind[0])

    def test_lock_compare_rejects_wrong_test_baseline(self) -> None:
        failures = validate_lock(
            lock_compare_record(),
            {"EVID_LOCK": lock_baseline_record(linked_tests=["OTHER_TEST"])},
        )

        self.assertIn("linked_tests do not match lock baseline", failures[0])

    def test_lock_compare_rejects_catalog_digest_drift(self) -> None:
        tests = {"TEST_RED": {"id": "TEST_RED", "case_file_digest": "sha256:current"}}

        failures = validate_lock(
            lock_compare_record(),
            {"EVID_LOCK": lock_baseline_record()},
            tests=tests,
        )

        self.assertIn("does not match catalog test TEST_RED", failures[0])

    def test_historical_lock_pair_uses_source_revision_catalog_digest(self) -> None:
        tests = {
            "TEST_RED": {
                "id": "TEST_RED",
                "command": "python3 current-writer.py",
                "case_file_digest": "sha256:current",
            }
        }
        compare = lock_compare_record(proof_contract_binding="source_revision")
        baseline = lock_baseline_record(proof_contract_binding="source_revision")

        failures = validate_lock(compare, {"EVID_LOCK": baseline}, tests=tests)

        self.assertFalse(
            any("catalog test TEST_RED" in failure for failure in failures),
            failures,
        )

    def test_lock_compare_rejects_bad_baseline_producer(self) -> None:
        failures = validate_lock(
            lock_compare_record(),
            {"EVID_LOCK": lock_baseline_record(producer="codex")},
        )

        self.assertIn("lock baseline producer 'codex' is not allowlisted", failures[0])

    def test_lock_compare_rejects_command_drift(self) -> None:
        tests = {
            "TEST_RED": {
                "id": "TEST_RED",
                "command": "python3 writer.py",
                "case_file_digest": "sha256:cases",
            }
        }

        baseline_command = validate_lock(
            lock_compare_record(),
            {"EVID_LOCK": lock_baseline_record(command="python3 old_writer.py")},
            tests=tests,
        )
        self.assertIn("command does not match catalog test TEST_RED", baseline_command[0])

        compare_command = validate_lock(
            lock_compare_record(command="python3 other_writer.py"),
            {"EVID_LOCK": lock_baseline_record()},
            tests=tests,
        )
        self.assertIn("command does not match catalog test TEST_RED", compare_command[0])

    def test_lock_compare_rejects_result_or_summary_delta(self) -> None:
        for record, expected in [
            (
                lock_compare_record(case_result_digest="sha256:other"),
                "case_result_digest does not match lock baseline",
            ),
            (
                lock_compare_record(per_case_summary=["CASE_FAIL:failed"]),
                "per_case_summary does not match lock baseline",
            ),
        ]:
            with self.subTest(expected=expected):
                failures = validate_lock(record, {"EVID_LOCK": lock_baseline_record()})
                self.assert_contains_failure(failures, expected)

    def test_lock_compare_rejects_non_passing_summary(self) -> None:
        failures = validate_lock(
            lock_compare_record(per_case_summary=["CASE_FAIL:blocked"]),
            {"EVID_LOCK": lock_baseline_record(per_case_summary=["CASE_FAIL:blocked"])},
        )

        self.assertIn("non-passing case CASE_FAIL:blocked", failures[0])

    def test_lock_compare_rejects_nonzero_exit_or_forged_result_digest(self) -> None:
        nonzero = validate_lock(
            lock_compare_record(command_exit_status=3),
            {"EVID_LOCK": lock_baseline_record(command_exit_status=3)},
        )
        self.assert_contains_failure(nonzero, "lock_compare proof command_exit_status must be 0")

        forged = validate_lock(
            lock_compare_record(case_result_digest="sha256:forged"),
            {"EVID_LOCK": lock_baseline_record(case_result_digest="sha256:forged")},
        )
        self.assertIn("case_result_digest does not match command_exit_status", forged[0])

    def test_lock_compare_rejects_missing_required_lock_fields(self) -> None:
        for field in (
            "paired_lock_baseline",
            "case_file_digest",
            "case_result_digest",
            "command_exit_status",
            "per_case_summary",
        ):
            with self.subTest(field=field):
                record = lock_compare_record()
                del record[field]
                failures = validate_lock(record, {"EVID_LOCK": lock_baseline_record()})
                self.assertIn(f"lock_compare proof missing field {field}", failures[0])

    def assert_contains_failure(self, failures: list[str], expected: str) -> None:
        self.assertTrue(
            any(expected in failure for failure in failures),
            f"{expected!r} not found in {failures!r}",
        )


class EvidenceProofProvenanceTests(unittest.TestCase):
    RED_COMMIT = "1" * 40
    GREEN_COMMIT = "2" * 40

    def test_valid_red_before_green_provenance_has_no_failures(self) -> None:
        failures = validate_provenance(
            green_record(commit=self.GREEN_COMMIT),
            {"EVID_RED": red_record(commit=self.RED_COMMIT)},
            known={self.RED_COMMIT, self.GREEN_COMMIT},
            ancestors={(self.RED_COMMIT, self.GREEN_COMMIT)},
        )

        self.assertEqual(failures, [])

    def test_proof_records_require_clean_full_commit(self) -> None:
        for commit in ("dirty:" + "1" * 12, "1" * 12, "not-a-commit"):
            with self.subTest(commit=commit):
                failures = validate_provenance(red_record(commit=commit), {})

                self.assertIn("clean full 40-hex commit", failures[0])

    def test_prove_records_require_canonical_durable_path(self) -> None:
        failures = validate_provenance(
            red_record(commit=self.RED_COMMIT, path="target/gate-artifacts/prove/EVID_RED.toml"),
            {},
            known={self.RED_COMMIT},
        )

        self.assertIn("path must be verification/evidence-index.toml", failures[0])

    def test_green_rejects_equal_or_non_ancestral_revision(self) -> None:
        equal = validate_provenance(
            green_record(commit=self.RED_COMMIT),
            {"EVID_RED": red_record(commit=self.RED_COMMIT)},
            known={self.RED_COMMIT},
            ancestors={(self.RED_COMMIT, self.RED_COMMIT)},
        )
        self.assertIn("must use distinct commits", equal[0])

        non_ancestor = validate_provenance(
            green_record(commit=self.GREEN_COMMIT),
            {"EVID_RED": red_record(commit=self.RED_COMMIT)},
            known={self.RED_COMMIT, self.GREEN_COMMIT},
        )
        self.assertIn("is not an ancestor", non_ancestor[0])

    def test_proof_rejects_revision_absent_from_repository(self) -> None:
        failures = validate_provenance(
            red_record(commit=self.RED_COMMIT),
            {},
            known=set(),
        )

        self.assertIn("does not resolve to a commit", failures[0])


class EvidenceProofContractBindingTests(unittest.TestCase):
    def test_normal_producer_record_matches_current_contract(self) -> None:
        self.assertEqual(validate_binding(red_record()), [])

    def test_command_or_arbitrary_catalog_drift_is_rejected(self) -> None:
        for field, value in (
            ("command", "python3 changed.py"),
            ("title", "Changed title"),
        ):
            with self.subTest(field=field):
                tests = {"TEST_RED": dict(DEFAULT_TEST, **{field: value})}
                failures = validate_binding(red_record(), tests=tests)

                self.assert_contains_failure(failures, "proof_contract_digest does not match current")

    def test_lifecycle_progression_does_not_stale_proof_contract(self) -> None:
        tests = {
            "TEST_RED": dict(
                DEFAULT_TEST,
                status="validated",
                suite_tiers=["pr", "nightly"],
                spec_gap_ref=None,
                last_reviewed="2026-07-13",
            )
        }
        invariants = {
            "INV": dict(
                DEFAULT_INVARIANTS["INV"],
                status="validated",
                proof_level="G2",
                tests=["TEST_RED"],
                gates=["pr", "nightly"],
                evidence_refs=["EVID_RED", "EVID_GREEN"],
                spec_gap_refs=[],
                missing=[],
                coverage={"cells": [{"dimension": "happy_path", "state": "covered"}]},
                last_reviewed="2026-07-13",
            )
        }

        failures = validate_binding(red_record(), tests=tests, invariants=invariants)

        self.assertEqual(failures, [])

    def test_oracle_or_behavior_drift_stales_proof_contract(self) -> None:
        for invariants in (
            {"INV": dict(DEFAULT_INVARIANTS["INV"], oracle={"ref": "SPEC_OTHER"})},
            {"INV": dict(DEFAULT_INVARIANTS["INV"], behavior=[{"outcome": "reject"}])},
        ):
            with self.subTest(invariants=invariants):
                failures = validate_binding(red_record(), invariants=invariants)
                self.assert_contains_failure(
                    failures,
                    "proof_contract_digest does not match current",
                )

    def test_invariant_list_or_content_drift_is_rejected(self) -> None:
        tests = {"TEST_RED": dict(DEFAULT_TEST, invariants=[])}
        list_failures = validate_binding(red_record(), tests=tests)
        self.assert_contains_failure(list_failures, "linked_invariants do not match current")

        invariants = {"INV": dict(DEFAULT_INVARIANTS["INV"], title="Changed")}
        content_failures = validate_binding(red_record(), invariants=invariants)
        self.assert_contains_failure(content_failures, "proof_contract_digest does not match current")

    def test_missing_proof_contract_digest_is_rejected(self) -> None:
        record = red_record()
        del record["proof_contract_digest"]

        failures = validate_binding(record)

        self.assert_contains_failure(failures, "missing proof_contract_digest")

    def test_missing_or_unsupported_proof_contract_version_is_rejected(self) -> None:
        missing = red_record()
        del missing["proof_contract_version"]
        unsupported = red_record(proof_contract_version="full_metadata_record_v1")

        self.assert_contains_failure(
            validate_binding(missing),
            "missing proof_contract_version",
        )
        self.assert_contains_failure(
            validate_binding(unsupported),
            "unsupported proof_contract_version",
        )

    def test_source_revision_binding_uses_historical_contract(self) -> None:
        historical_test = dict(DEFAULT_TEST)
        historical_invariants = {"INV": dict(DEFAULT_INVARIANTS["INV"])}
        record = red_record(
            proof_contract_binding="source_revision",
            proof_contract_digest=proof_contract_digest(
                test=historical_test,
                invariants=historical_invariants,
            ),
        )
        live_invariants = {
            "INV": dict(
                DEFAULT_INVARIANTS["INV"],
                oracle={"ref": "SPEC_REVIEWED_AFTER_PROOF"},
            )
        }

        failures = validate_binding(
            record,
            invariants=live_invariants,
            historical_loader=lambda _commit, _test_id: (
                historical_test,
                historical_invariants,
            ),
        )

        self.assertEqual(failures, [])

    def test_source_revision_binding_rejects_historical_digest_tamper(self) -> None:
        record = red_record(
            proof_contract_binding="source_revision",
            proof_contract_digest="sha256:" + "f" * 64,
        )

        failures = validate_binding(
            record,
            historical_loader=lambda _commit, _test_id: (
                DEFAULT_TEST,
                DEFAULT_INVARIANTS,
            ),
        )

        self.assert_contains_failure(
            failures,
            "does not match source-revision catalog and invariants",
        )

    def test_unknown_proof_contract_binding_is_rejected(self) -> None:
        failures = validate_binding(red_record(proof_contract_binding="archive"))

        self.assert_contains_failure(failures, "unknown proof_contract_binding")

    def assert_contains_failure(self, failures: list[str], expected: str) -> None:
        self.assertTrue(
            any(expected in failure for failure in failures),
            f"{expected!r} not found in {failures!r}",
        )


def validate(
    record: dict[str, object],
    evidence: dict[str, dict[str, object]],
    *,
    tests: dict[str, dict[str, object]] | None = None,
) -> list[str]:
    failures: list[str] = []
    validate_green_pairing(
        fail=lambda _path, message: failures.append(message),
        path=Path("verification/evidence-index.toml"),
        record=record,
        evidence=evidence,
        tests=tests
        or {"TEST_RED": DEFAULT_TEST},
        invariants=DEFAULT_INVARIANTS,
        approved_producers=set(),
    )
    return failures


def validate_lock(
    record: dict[str, object],
    evidence: dict[str, dict[str, object]],
    *,
    tests: dict[str, dict[str, object]] | None = None,
) -> list[str]:
    failures: list[str] = []
    validate_lock_pairing(
        fail=lambda _path, message: failures.append(message),
        path=Path("verification/evidence-index.toml"),
        record=record,
        evidence=evidence,
        tests=tests
        or {"TEST_RED": DEFAULT_TEST},
        invariants=DEFAULT_INVARIANTS,
        approved_producers=set(),
    )
    return failures


def validate_binding(
    record: dict[str, object],
    *,
    tests: dict[str, dict[str, object]] | None = None,
    invariants: dict[str, dict[str, object]] | None = None,
    historical_loader=None,
) -> list[str]:
    failures: list[str] = []
    validate_proof_contract_binding(
        fail=lambda _path, message: failures.append(message),
        path=Path("verification/evidence-index.toml"),
        record=record,
        tests=tests or {"TEST_RED": DEFAULT_TEST},
        invariants=invariants or DEFAULT_INVARIANTS,
        historical_contract_loader=historical_loader,
    )
    return failures


DEFAULT_TEST: dict[str, object] = {
    "id": "TEST_RED",
    "title": "Red test",
    "command": "python3 writer.py",
    "case_file_digest": "sha256:cases",
    "invariants": ["INV"],
    "status": "mapped",
    "suite_tiers": ["pr"],
    "spec_gap_ref": "SPEC_GAP",
    "last_reviewed": "2026-07-12",
}
DEFAULT_INVARIANTS: dict[str, dict[str, object]] = {
    "INV": {
        "id": "INV",
        "title": "Invariant",
        "status": "gap_open",
        "proof_level": "S0",
        "tests": [],
        "gates": ["pr"],
        "evidence_refs": [],
        "spec_gap_refs": ["SPEC_GAP"],
        "missing": ["proof"],
        "coverage": {"cells": [{"dimension": "happy_path", "state": "gap_open"}]},
        "oracle": {"ref": "SPEC"},
        "behavior": [{"outcome": "accept_value"}],
        "last_reviewed": "2026-07-12",
    }
}


def contract_digest() -> str:
    return proof_contract_digest(
        test=DEFAULT_TEST,
        invariants=DEFAULT_INVARIANTS,
    )


def validate_provenance(
    record: dict[str, object],
    evidence: dict[str, dict[str, object]],
    *,
    known: set[str] | None = None,
    ancestors: set[tuple[str, str]] | None = None,
) -> list[str]:
    failures: list[str] = []
    known_revisions = known or set()
    ancestor_pairs = ancestors or set()
    validate_proof_provenance(
        fail=lambda _path, message: failures.append(message),
        path=Path("verification/evidence-index.toml"),
        record=record,
        evidence=evidence,
        revision_exists=lambda commit: commit in known_revisions,
        is_ancestor=lambda before, after: (before, after) in ancestor_pairs,
    )
    return failures


def green_record(**overrides: object) -> dict[str, object]:
    record: dict[str, object] = {
        "id": "EVID_GREEN",
        "kind": "committed_file",
        "path": "verification/evidence-index.toml",
        "commit": "2" * 40,
        "proof_kind": "green",
        "producer": "prove.py v1",
        "linked_tests": ["TEST_RED"],
        "linked_invariants": ["INV"],
        "case_file_digest": "sha256:cases",
        "proof_contract_digest": contract_digest(),
        "proof_contract_version": PROOF_CONTRACT_VERSION,
        "paired_red_evidence": "EVID_RED",
        "formerly_red_case_ids": ["CASE_FAIL"],
        "per_case_summary": ["CASE_FAIL:passed"],
        "command_exit_status": 0,
    }
    record.update(overrides)
    return record


def red_record(**overrides: object) -> dict[str, object]:
    record: dict[str, object] = {
        "id": "EVID_RED",
        "kind": "committed_file",
        "path": "verification/evidence-index.toml",
        "commit": "1" * 40,
        "proof_kind": "red",
        "producer": "prove.py v1",
        "failure_kind": "assertion_failure",
        "linked_tests": ["TEST_RED"],
        "linked_invariants": ["INV"],
        "case_file_digest": "sha256:cases",
        "proof_contract_digest": contract_digest(),
        "proof_contract_version": PROOF_CONTRACT_VERSION,
        "red_case_ids": ["CASE_FAIL"],
        "per_case_summary": ["CASE_FAIL:failed"],
    }
    record.update(overrides)
    return record


def lock_baseline_record(**overrides: object) -> dict[str, object]:
    summary = overrides.get("per_case_summary", ["CASE_FAIL:passed"])
    exit_status = overrides.get("command_exit_status", 0)
    record: dict[str, object] = {
        "id": "EVID_LOCK",
        "proof_kind": "lock_baseline",
        "producer": "prove.py v1",
        "linked_tests": ["TEST_RED"],
        "linked_invariants": ["INV"],
        "command": "python3 writer.py",
        "case_file_digest": "sha256:cases",
        "proof_contract_digest": contract_digest(),
        "proof_contract_version": PROOF_CONTRACT_VERSION,
        "case_result_digest": case_result_digest(
            command_exit_status=int(exit_status),
            per_case_summary=list(summary) if isinstance(summary, list) else [],
        ),
        "command_exit_status": 0,
        "per_case_summary": ["CASE_FAIL:passed"],
    }
    record.update(overrides)
    return record


def lock_compare_record(**overrides: object) -> dict[str, object]:
    summary = overrides.get("per_case_summary", ["CASE_FAIL:passed"])
    exit_status = overrides.get("command_exit_status", 0)
    record: dict[str, object] = {
        "id": "EVID_LOCK_COMPARE",
        "proof_kind": "lock_compare",
        "producer": "prove.py v1",
        "linked_tests": ["TEST_RED"],
        "linked_invariants": ["INV"],
        "command": "python3 writer.py",
        "case_file_digest": "sha256:cases",
        "proof_contract_digest": contract_digest(),
        "proof_contract_version": PROOF_CONTRACT_VERSION,
        "case_result_digest": case_result_digest(
            command_exit_status=int(exit_status),
            per_case_summary=list(summary) if isinstance(summary, list) else [],
        ),
        "command_exit_status": 0,
        "paired_lock_baseline": "EVID_LOCK",
        "per_case_summary": ["CASE_FAIL:passed"],
    }
    record.update(overrides)
    return record


if __name__ == "__main__":
    unittest.main()
