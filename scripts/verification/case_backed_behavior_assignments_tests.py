"""Focused tests for the Phase 18 case-backed behavior mapping transaction."""

from __future__ import annotations

import copy
import json
import tempfile
import tomllib
import unittest
from pathlib import Path

from scripts.verification.case_backed_behavior_assignments import (
    ASSIGNMENT_DIGEST,
    applied_plan_from_documents,
    build_case_backed_plan,
    match_behavior_cases,
    replace_table_record_fields,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-remaining-authority-142/"
    "frozen-gaps-after-local.json"
)


class CaseBackedBehaviorAssignmentsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_case_backed_plan(root=ROOT, gap_report=GAP_REPORT)

    def test_complete_current_case_backed_population_is_one_frozen_plan(self) -> None:
        self.assertEqual(101, self.plan["behavior_count"])
        self.assertEqual(37, self.plan["catalog_test_count"])
        self.assertEqual(37, self.plan["invariant_count"])
        self.assertEqual(105, self.plan["case_count"])
        self.assertEqual(ASSIGNMENT_DIGEST, self.plan["assignment_digest"])
        self.assertEqual(
            "cb4a4f71406292b32a6d4c3aa0697d1a0d216fe21482c11272e465dc1c87fd18",
            self.plan["assignment_digest"],
        )

    def test_every_assignment_reuses_exact_passing_case_evidence(self) -> None:
        assignments = self.plan["assignments"]
        reused_evidence = {
            row["catalog_test_id"]: row
            for row in self.plan["reused_execution_evidence"]
        }
        self.assertEqual(37, len(reused_evidence))
        for assignment in assignments:
            evidence = reused_evidence[assignment["catalog_test_id"]]
            self.assertEqual(0, evidence["command_exit_status"])
            self.assertEqual(
                assignment["case_file_digest"],
                evidence["case_file_digest"],
            )
            self.assertTrue(
                set(assignment["case_ids"]).issubset(
                    set(evidence["passed_case_ids"])
                )
            )

    def test_plan_updates_every_current_bound_proof_digest(self) -> None:
        self.assertEqual(72, self.plan["proof_digest_update_count"])
        self.assertEqual(
            self.plan["proof_digest_update_count"],
            len(self.plan["proof_digest_updates"]),
        )
        updated_evidence_ids = {
            row["evidence_id"] for row in self.plan["proof_digest_updates"]
        }
        self.assertTrue(
            {
                "EVID_TEST_RUNTIME_RESTART_TRACE_001_RED",
                "EVID_TEST_RUNTIME_RESTART_TRACE_001_GREEN",
                "EVID_TEST_VM_RESOURCE_LIMIT_CASES_001_RED",
                "EVID_TEST_VM_RESOURCE_LIMIT_CASES_001_GREEN",
            }.issubset(updated_evidence_ids)
        )
        self.assertTrue(
            all(
                row["proof_contract_digest"].startswith("sha256:")
                for row in self.plan["proof_digest_updates"]
            )
        )

    def test_stale_or_ambiguous_case_tables_remain_unmapped(self) -> None:
        rejected = {
            row["invariant_id"]: set(row["reasons"])
            for row in self.plan["rejected_invariants"]
        }
        self.assertIn("stale_source_digest", rejected["IEC_PARSE_RECOVER_001"])
        self.assertIn(
            "no_exact_case:EDIT_RENAME_001#behavior:3",
            rejected["EDIT_RENAME_001"],
        )
        assigned = {row["behavior_ref"] for row in self.plan["assignments"]}
        self.assertNotIn("IEC_PARSE_RECOVER_001#behavior:0", assigned)
        self.assertNotIn("EDIT_RENAME_001#behavior:3", assigned)

    def test_matcher_accepts_multiple_exact_cases_for_one_behavior(self) -> None:
        behavior = {
            "partition": {"equals": "DENIED"},
            "case_family": "auth_or_permission",
            "oracle_ref": "SPEC#denial",
            "outcome": "reject",
            "delta": {"target": "unchanged"},
        }
        cases = [
            {
                "id": "VIEWER_DENIED",
                "family": "auth_or_permission",
                "input": {
                    "source_partition": {"equals": "DENIED"},
                    "scenario": "viewer",
                },
                "expect": {
                    "oracle_ref": "SPEC#denial",
                    "outcome": "reject",
                    "delta": {"target": "unchanged"},
                },
            },
            {
                "id": "OPERATOR_DENIED",
                "family": "auth_or_permission",
                "input": {
                    "source_partition": {"equals": "DENIED"},
                    "scenario": "operator",
                },
                "expect": {
                    "oracle_ref": "SPEC#denial",
                    "outcome": "reject",
                    "delta": {"target": "unchanged"},
                },
            },
        ]

        self.assertEqual(
            ["OPERATOR_DENIED", "VIEWER_DENIED"],
            match_behavior_cases(behavior=behavior, cases=cases),
        )

    def test_matcher_rejects_neighboring_oracle_or_outcome(self) -> None:
        behavior = {
            "partition": {"equals": "DENIED"},
            "case_family": "auth_or_permission",
            "oracle_ref": "SPEC#denial",
            "outcome": "reject",
        }
        wrong_oracle = {
            "id": "WRONG_ORACLE",
            "family": "auth_or_permission",
            "input": {"source_partition": {"equals": "DENIED"}},
            "expect": {"oracle_ref": "SPEC#other", "outcome": "reject"},
        }
        wrong_outcome = copy.deepcopy(wrong_oracle)
        wrong_outcome["id"] = "WRONG_OUTCOME"
        wrong_outcome["expect"] = {
            "oracle_ref": "SPEC#denial",
            "outcome": "accept_value",
        }

        self.assertEqual(
            [],
            match_behavior_cases(
                behavior=behavior,
                cases=[wrong_oracle, wrong_outcome],
            ),
        )

    def test_catalog_record_replacement_preserves_unrelated_fields_and_rows(self) -> None:
        source = """\
schema_version = 1

[[tests]]
schema_version = 2
id = "TEST_ONE"
command = "cargo test one"
invariants = ["INV"]
case_file = "verification/cases/INV.toml"
case_file_digest = "sha256:old"
last_reviewed = "2026-07-01"

[[tests]]
schema_version = 2
id = "TEST_TWO"
command = "cargo test two"
invariants = ["OTHER"]
last_reviewed = "2026-07-01"
"""
        result = replace_table_record_fields(
            source,
            table="tests",
            updates={
                "TEST_ONE": {
                    "mapping_behavior_refs": [
                        "INV#behavior:0",
                        "INV#behavior:1",
                    ]
                }
            },
        )
        document = tomllib.loads(result)

        self.assertEqual(
            ["INV#behavior:0", "INV#behavior:1"],
            document["tests"][0]["mapping_behavior_refs"],
        )
        self.assertEqual("cargo test one", document["tests"][0]["command"])
        self.assertEqual("sha256:old", document["tests"][0]["case_file_digest"])
        self.assertEqual("TEST_TWO", document["tests"][1]["id"])
        self.assertEqual(
            result,
            replace_table_record_fields(
                result,
                table="tests",
                updates={
                    "TEST_ONE": {
                        "mapping_behavior_refs": [
                            "INV#behavior:0",
                            "INV#behavior:1",
                        ]
                    }
                },
            ),
        )

    def test_generated_replacements_round_trip_and_close_only_101_demands(self) -> None:
        replacements = self.plan["replacements"]
        self.assertEqual(
            {
                "verification/test-catalog.toml",
                "verification/evidence-index.toml",
                "verification/p18/review-locks/"
                "case-backed-behavior-101-semantic-lock.json",
            },
            set(replacements),
        )
        catalog = tomllib.loads(replacements["verification/test-catalog.toml"])
        mapped_refs = {
            ref
            for test in catalog["tests"]
            for ref in test.get("mapping_behavior_refs", [])
        }
        self.assertTrue(
            {
                row["behavior_ref"] for row in self.plan["assignments"]
            }.issubset(mapped_refs)
        )
        lock = json.loads(
            replacements[
                "verification/p18/review-locks/"
                "case-backed-behavior-101-semantic-lock.json"
            ]
        )
        self.assertEqual(101, lock["behavior_count"])
        self.assertEqual(self.plan["assignment_digest"], lock["assignment_digest"])

    def test_applied_lock_reconstructs_the_same_idempotent_plan(self) -> None:
        replacements = self.plan["replacements"]

        applied = applied_plan_from_documents(
            root=ROOT,
            report=json.loads(GAP_REPORT.read_text()),
            catalog_source=replacements["verification/test-catalog.toml"],
            evidence_source=replacements["verification/evidence-index.toml"],
            lock_source=replacements[
                "verification/p18/review-locks/"
                "case-backed-behavior-101-semantic-lock.json"
            ],
        )

        self.assertEqual(self.plan["assignment_digest"], applied["assignment_digest"])
        self.assertEqual(101, applied["behavior_count"])
        self.assertEqual(72, applied["proof_digest_update_count"])
        self.assertEqual(replacements, applied["replacements"])


if __name__ == "__main__":
    unittest.main()
