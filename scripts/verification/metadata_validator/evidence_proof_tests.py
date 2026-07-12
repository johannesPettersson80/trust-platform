"""Unit tests for evidence proof pairing validation."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.metadata_validator.evidence_proof import (
    validate_green_pairing,
    validate_lock_pairing,
    validate_proof_provenance,
)
from scripts.verification.prover import case_result_digest


class EvidenceProofTests(unittest.TestCase):
    def test_valid_green_pairing_has_no_failures(self) -> None:
        failures = validate(green_record(), {"EVID_RED": red_record()})

        self.assertEqual(failures, [])

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

    def test_valid_lock_compare_has_no_failures(self) -> None:
        failures = validate_lock(lock_compare_record(), {"EVID_LOCK": lock_baseline_record()})

        self.assertEqual(failures, [])

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
        or {
            "TEST_RED": {
                "id": "TEST_RED",
                "command": "python3 writer.py",
                "case_file_digest": "sha256:cases",
            }
        },
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
        or {
            "TEST_RED": {
                "id": "TEST_RED",
                "command": "python3 writer.py",
                "case_file_digest": "sha256:cases",
            }
        },
        approved_producers=set(),
    )
    return failures


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
        "case_file_digest": "sha256:cases",
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
        "case_file_digest": "sha256:cases",
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
        "command": "python3 writer.py",
        "case_file_digest": "sha256:cases",
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
        "command": "python3 writer.py",
        "case_file_digest": "sha256:cases",
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
