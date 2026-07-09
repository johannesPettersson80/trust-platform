"""Adversarial fixtures for the verification control-plane pilot."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.metadata_validator.evidence_proof import validate_green_pairing
from scripts.verification.metadata_validator.case_files import validate_case_record
from scripts.verification.planner import Planner, risk_changes_from_matrices
from scripts.verification.prover import ProofError
from scripts.verification.prover_tests import fixture
from scripts.verification.report_gate import find_uncataloged_tests


class AdversarialSelfTestFixtures(unittest.TestCase):
    def test_assert_nothing_red_proof_is_refused(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "none")
            self.assertFalse((fx.evidence_dir / "EVID_TEST_RED_RED.toml").exists())

    def test_skipped_case_cannot_be_red_proof(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("skipped_case", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("skipped", str(raised.exception))

    def test_stale_case_digest_is_rejected_by_metadata_validator(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.tests["TEST_CASE_TABLE_VM_SEAM_VALID_001"]["case_file_digest"] = "sha256:stale"

        validator.validate()

        self.assertTrue(
            any("case_file_digest mismatch" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_mutation_catalog_corruption_is_rejected_by_full_validator(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.tests["TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"]["mutation_shard_id"] = (
            "BYTECODE_VALIDATOR_CORRUPTED"
        )

        validator.validate()

        self.assertTrue(
            any("shard/test binding mismatch" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_catalog_subject_bypass_is_rejected_by_full_validator(self) -> None:
        validator = Validator()
        validator.load_records()
        mutation = validator.tests["TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"]
        mutation["subject_kind"] = "generated_test"

        validator.validate()

        self.assertTrue(
            any("generated_test requires discovery_id" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_missing_oracle_is_rejected_by_case_file_validator(self) -> None:
        failures: list[str] = []

        validate_case_record(
            fail=lambda _path, message: failures.append(message),
            path=ROOT / "verification/test-catalog.toml",
            test_id="TEST_MISSING_ORACLE",
            case={
                "id": "CASE_EXPECT",
                "family": "happy_path",
                "input": {"scenario": "RUN"},
                "expect": {"outcome": "accept_value", "oracle_ref": "SPEC_DOES_NOT_EXIST#case"},
            },
            invariant={"id": "INV", "behavior": [], "input": {}},
            spec_sources={},
            spec_gaps={},
            seen_case_ids=set(),
        )

        self.assertTrue(any("unknown spec source" in failure for failure in failures), failures)
        self.assertTrue(any("does not match an oracle-backed behavior row" in failure for failure in failures), failures)

    def test_closed_spec_gap_still_referenced_by_required_spec_is_rejected(self) -> None:
        validator = Validator()
        validator.load_records()
        gap_id = "SPEC_GAP_BYTECODE_VALIDATOR_001"
        closed_gap = dict(validator.spec_gaps[gap_id])
        closed_gap["resolution_status"] = "closed"
        validator.spec_gaps[gap_id] = closed_gap

        validator.validate_required_spec_gap(
            ROOT / "verification/spec-matrix.toml",
            {"id": "REQ_TEST", "status": "spec_gap"},
            gap_id,
        )

        self.assertTrue(
            any("not open/actionable" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_risk_downgrade_is_reported(self) -> None:
        changes = risk_changes_from_matrices(
            {"bytecode_vm"},
            current_areas={
                "bytecode_vm": {
                    "risk_default": "maintenance",
                    "high_risks": [],
                }
            },
            baseline_areas={
                "bytecode_vm": {
                    "risk_default": "wrong_result",
                    "high_risks": ["wrong_result", "silent_corruption"],
                }
            },
        )

        joined = "\n".join(changes)
        self.assertIn("risk_default wrong_result -> maintenance", joined)
        self.assertIn("high_risks ['silent_corruption', 'wrong_result'] -> []", joined)

    def test_manual_safety_evidence_cannot_feed_green_pairing(self) -> None:
        failures: list[str] = []
        green = {
            "id": "EVID_GREEN",
            "proof_kind": "green",
            "producer": "prove.py v1",
            "linked_tests": ["TEST_RED"],
            "case_file_digest": "sha256:cases",
            "paired_red_evidence": "EVID_RED",
            "formerly_red_case_ids": ["CASE_FAIL"],
            "per_case_summary": ["CASE_FAIL:passed"],
            "command_exit_status": 0,
        }
        red = {
            "id": "EVID_RED",
            "proof_kind": "red",
            "producer": "manual",
            "failure_kind": "assertion_failure",
            "linked_tests": ["TEST_RED"],
            "case_file_digest": "sha256:cases",
            "red_case_ids": ["CASE_FAIL"],
            "per_case_summary": ["CASE_FAIL:failed"],
        }

        validate_green_pairing(
            fail=lambda _path, message: failures.append(message),
            path=ROOT / "verification/evidence-index.toml",
            record=green,
            evidence={"EVID_RED": red},
            tests={"TEST_RED": {"id": "TEST_RED", "case_file_digest": "sha256:cases"}},
            approved_producers=set(),
        )

        self.assertTrue(any("producer 'manual' is not allowlisted" in failure for failure in failures), failures)

    def test_compile_error_cannot_be_recorded_as_red(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("none", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "compile_error")
            self.assertFalse((fx.evidence_dir / "EVID_TEST_RED_RED.toml").exists())

    def test_uncataloged_changed_test_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "verification").mkdir()
            (root / "verification/test-catalog.toml").write_text("")

            missing = find_uncataloged_tests(
                root=root,
                changed_files=["crates/trust-runtime/tests/new_case.rs"],
            )

        self.assertEqual(missing, ["crates/trust-runtime/tests/new_case.rs"])

    def test_unmapped_file_is_reported_by_planner(self) -> None:
        result = Planner().plan(
            "bugfix",
            ["README.md"],
            None,
            None,
        )

        self.assertEqual(result.exit_code, 4)
        self.assertEqual(result.unmapped_files, ["README.md"])


if __name__ == "__main__":
    unittest.main()
