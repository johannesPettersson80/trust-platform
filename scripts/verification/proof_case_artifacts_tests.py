"""Adversarial tests for closed proof case-artifact consumption."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.proof_case_artifacts import (
    CaseArtifactContractError,
    validate_case_artifact,
)


class ProofCaseArtifactContractTests(unittest.TestCase):
    def artifact(self) -> dict:
        return {
            "schema_version": 1,
            "test_id": "TEST_CASE",
            "case_file": "verification/cases/compiler_iec/TEST_CASE.toml",
            "case_file_digest": "sha256:" + "a" * 64,
            "helper_version": "verification-cases v1",
            "case_provenance_kind": "generated_decision_table_v1",
            "trace_definition_digest": None,
            "trust_verify_test_id": "TEST_CASE",
            "trust_verify_run_id": "run-1",
            "trust_verify_case_file_digest": "sha256:" + "a" * 64,
            "trust_verify_artifact_dir": "/tmp/cases",
            "cases": [
                {
                    "id": "CASE_ONE",
                    "family": "happy_path",
                    "result": "passed",
                    "spec_gap_ref": None,
                    "observed_error": None,
                    "observed_status": "ok",
                    "state_delta": "unchanged",
                    "before": None,
                    "after": None,
                }
            ],
        }

    def validate(self, artifact: dict) -> tuple[list[str], list[str], list[str]]:
        return validate_case_artifact(
            artifact=artifact,
            expected_test_id="TEST_CASE",
            expected_case_file="verification/cases/compiler_iec/TEST_CASE.toml",
            expected_run_id="run-1",
            expected_artifact_dir="/tmp/cases",
            expected_case_file_digest="sha256:" + "a" * 64,
            expected_case_ids=["CASE_ONE"],
            expected_case_provenance_kind="generated_decision_table_v1",
            expected_trace_definition_digest=None,
        )

    def test_complete_closed_artifact_is_accepted(self) -> None:
        self.assertEqual(
            self.validate(self.artifact()),
            ([], [], ["CASE_ONE:passed"]),
        )

    def test_root_fields_are_exact_and_all_required(self) -> None:
        artifact = self.artifact()
        artifact["unreviewed_root"] = True
        with self.assertRaisesRegex(
            CaseArtifactContractError, "root fields must be exactly"
        ):
            self.validate(artifact)

        for field in self.artifact():
            with self.subTest(field=field):
                missing = self.artifact()
                missing.pop(field)
                with self.assertRaisesRegex(
                    CaseArtifactContractError, "root fields must be exactly"
                ):
                    self.validate(missing)

    def test_case_fields_are_exact_and_all_required(self) -> None:
        artifact = self.artifact()
        artifact["cases"][0]["unreviewed_case"] = True
        with self.assertRaisesRegex(
            CaseArtifactContractError, "case fields must be exactly"
        ):
            self.validate(artifact)

        for field in self.artifact()["cases"][0]:
            with self.subTest(field=field):
                missing = copy.deepcopy(self.artifact())
                missing["cases"][0].pop(field)
                with self.assertRaisesRegex(
                    CaseArtifactContractError, "case fields must be exactly"
                ):
                    self.validate(missing)

    def test_case_file_and_helper_version_are_exact(self) -> None:
        wrong_path = self.artifact()
        wrong_path["case_file"] = "verification/cases/compiler_iec/OTHER.toml"
        with self.assertRaisesRegex(CaseArtifactContractError, "case_file mismatch"):
            self.validate(wrong_path)

        wrong_helper = self.artifact()
        wrong_helper["helper_version"] = "test-helper"
        with self.assertRaisesRegex(
            CaseArtifactContractError, "helper_version mismatch"
        ):
            self.validate(wrong_helper)


if __name__ == "__main__":
    unittest.main()
