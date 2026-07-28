"""Focused tests for the Phase 18 stale case-authority refresh."""

from __future__ import annotations

import json
import tomllib
import unittest
from pathlib import Path

from scripts.verification.stale_case_authority_refresh import (
    ASSIGNMENT_DIGEST,
    apply_prepared_plan,
    build_refresh_plan,
    finalize_refresh_plan,
)


ROOT = Path(__file__).resolve().parents[2]
GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-case-backed-behavior-101/"
    "frozen-gaps-after-local.json"
)
EXECUTION_ROOT = ROOT / (
    "target/gate-artifacts/verification/"
    "p18-stale-case-authority-refresh-54"
)


class StaleCaseAuthorityRefreshTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = build_refresh_plan(root=ROOT, gap_report=GAP_REPORT)

    def test_complete_recoverable_stale_case_population_is_frozen(self) -> None:
        self.assertEqual(13, self.plan["invariant_count"])
        self.assertEqual(82, self.plan["case_count"])
        self.assertEqual(6, self.plan["generated_authority_count"])
        self.assertEqual(16, self.plan["affected_catalog_test_count"])
        self.assertEqual(13, self.plan["execution_test_count"])
        self.assertEqual(12, self.plan["runner_source_count"])
        self.assertEqual(54, self.plan["behavior_count"])
        self.assertEqual(59, self.plan["assigned_case_count"])
        self.assertEqual(ASSIGNMENT_DIGEST, self.plan["assignment_digest"])
        self.assertEqual(
            "7647b73d2f76c3347416b085255cf05fb4a9b21f98620406fe7bc4e4c85956d7",
            self.plan["assignment_digest"],
        )

    def test_every_refreshed_case_has_one_exact_current_behavior(self) -> None:
        for authority in self.plan["authorities"]:
            case_bindings = authority["case_bindings"]
            self.assertEqual(authority["case_count"], len(case_bindings))
            self.assertTrue(
                all(len(row["behavior_refs"]) == 1 for row in case_bindings)
            )

    def test_generated_authorities_cover_every_current_behavior(self) -> None:
        generated = [
            authority
            for authority in self.plan["authorities"]
            if authority["case_provenance_kind"]
            == "generated_decision_table_v1"
        ]
        self.assertEqual(6, len(generated))
        for authority in generated:
            bound = {
                behavior_ref
                for row in authority["case_bindings"]
                for behavior_ref in row["behavior_refs"]
            }
            self.assertEqual(
                set(authority["current_behavior_refs"]),
                bound,
            )

    def test_semantically_stale_tables_remain_rejected(self) -> None:
        rejected = {
            row["invariant_id"]: row for row in self.plan["rejected_authorities"]
        }
        self.assertEqual(
            {
                "RT_SAFE_STOP_001",
                "VM_SEAM_VALID_001",
            },
            set(rejected),
        )
        self.assertTrue(
            rejected["RT_SAFE_STOP_001"]["unmatched_case_ids"]
        )
        self.assertEqual(
            7,
            len(rejected["VM_SEAM_VALID_001"]["unmatched_case_ids"]),
        )

    def test_transaction_requires_all_affected_runners_and_retires_old_current_proof(
        self,
    ) -> None:
        self.assertEqual(13, len(self.plan["execution_test_ids"]))
        self.assertEqual(3, len(self.plan["structural_test_ids"]))
        self.assertEqual(26, len(self.plan["retired_current_evidence_ids"]))
        self.assertTrue(
            all(
                evidence_id.startswith("EVID_")
                for evidence_id in self.plan["retired_current_evidence_ids"]
            )
        )

    def test_every_runner_source_is_rebound_to_the_current_case_digest(
        self,
    ) -> None:
        replacements = self.plan["replacements"]
        for authority in self.plan["authorities"]:
            current_digest = authority["current_case_file_digest"]
            old_digest = authority["old_case_file_digest"]
            for source_path in authority["runner_source_paths"]:
                source = replacements[source_path]
                self.assertIn(current_digest, source)
                self.assertNotIn(old_digest, source)

    def test_every_changed_source_closure_is_rebound(self) -> None:
        self.assertEqual(46, self.plan["source_closure_refresh_test_count"])
        catalog = {
            row["id"]: row
            for row in tomllib.loads(
                self.plan["replacements"]["verification/test-catalog.toml"]
            )["tests"]
        }
        replacements = self.plan["replacements"]
        for test_id in self.plan["source_closure_refresh_test_ids"]:
            record = catalog[test_id]
            closure = record.get("mapping_assertion_source_digests", {})
            changed_paths = sorted(set(closure).intersection(replacements))
            self.assertTrue(changed_paths)
            for path in changed_paths:
                self.assertEqual(
                    "sha256:"
                    + __import__("hashlib")
                    .sha256(replacements[path].encode())
                    .hexdigest(),
                    closure[path],
                )

    def test_every_source_closure_refresh_rebinds_accepted_review(self) -> None:
        self.assertEqual(
            46,
            self.plan["source_closure_refresh_decision_count"],
        )
        replacements = self.plan["replacements"]
        catalog = {
            row["id"]: row
            for row in tomllib.loads(
                replacements["verification/test-catalog.toml"]
            )["tests"]
        }
        review_ledger = tomllib.loads(
            replacements[
                "verification/test-catalog-mapping-decisions.toml"
            ]
        )
        proposals = {
            row["id"]: row for row in review_ledger["proposal_snapshots"]
        }
        decisions = {
            row["id"]: row for row in review_ledger["decisions"]
        }
        inventory_artifact = json.loads(
            (
                EXECUTION_ROOT
                / "source-closure-assertion-inventories.json"
            ).read_text()
        )
        inventories = inventory_artifact["inventories"]
        for test_id in self.plan["source_closure_refresh_test_ids"]:
            test = catalog[test_id]
            proposal = proposals[test["mapping_proposal_id"]]
            decision = decisions[test["mapping_decision_id"]]
            inventory = inventories[test["discovery_id"]]
            self.assertEqual("accepted", decision["outcome"])
            self.assertEqual(proposal["id"], decision["proposal_id"])
            self.assertEqual(
                proposal["proposal_digest"],
                decision["proposal_digest"],
            )
            self.assertEqual(
                inventory["source_digest"],
                proposal["test_source_digest"],
            )
            self.assertEqual(
                inventory["inventory_digest"],
                proposal["assertion_inventory_digest"],
            )
            self.assertEqual(
                inventory["source_digests"],
                proposal["assertion_source_digests"],
            )
            self.assertEqual(
                inventory["assertions"],
                proposal["assertions"],
            )
            self.assertEqual(
                inventory["unresolved_assertion_helpers"],
                proposal["unresolved_assertion_helpers"],
            )
            self.assertEqual(
                inventory["source_digest"],
                decision["reviewed_source_digest"],
            )
            self.assertEqual(
                inventory["source_digest"],
                test["mapping_reviewed_source_digest"],
            )

    def test_replacements_close_exactly_fifty_four_demands(self) -> None:
        after = self.plan["post_refresh_gap_summary"]
        self.assertEqual(0, after["missing_spec_authority_tests"])
        self.assertEqual(267, after["confirmed_missing_tests"])
        self.assertEqual(
            321,
            len(json.loads(GAP_REPORT.read_text())["confirmed_missing_tests"]),
        )

    def test_prepared_apply_is_exact_and_recoverable(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "existing.txt").write_text("before")
            plan = {
                "replacements": {
                    "existing.txt": "after",
                    "nested/new.txt": "new",
                }
            }

            apply_prepared_plan(root=root, plan=plan)

            self.assertEqual("after", (root / "existing.txt").read_text())
            self.assertEqual("new", (root / "nested/new.txt").read_text())

    def test_complete_execution_finalizes_all_proof_records(self) -> None:
        campaign = json.loads((EXECUTION_ROOT / "campaign.json").read_text())
        equality = json.loads(
            (EXECUTION_ROOT / "generated-authority-equality.json").read_text()
        )
        final = finalize_refresh_plan(
            root=ROOT,
            plan=self.plan,
            campaign=campaign,
            proof_records_dir=EXECUTION_ROOT / "proof-records",
            case_artifact_dir=EXECUTION_ROOT / "cases",
            generated_authority_equality=equality,
        )

        self.assertEqual("executed", final["state"])
        self.assertEqual(campaign["source_commit"], final["source_commit"])
        self.assertEqual(13, final["proof_record_count"])
        self.assertEqual(82, final["passed_case_count"])
        self.assertEqual(6, final["generated_authority_equality_count"])
        evidence = __import__("tomllib").loads(
            final["replacements"]["verification/evidence-index.toml"]
        )["evidence"]
        appended = [
            row
            for row in evidence
            if row["id"] in final["proof_record_ids"]
        ]
        self.assertEqual(13, len(appended))
        self.assertTrue(
            all(
                row["path"] == "verification/evidence-index.toml"
                and row["proof_contract_binding"] == "current"
                for row in appended
            )
        )
        lock = json.loads(
            final["replacements"][
                "verification/p18/review-locks/"
                "stale-case-authority-refresh-54-semantic-lock.json"
            ]
        )
        self.assertEqual("executed", lock["state"])
        self.assertEqual(82, lock["execution"]["passed_case_count"])

    def test_finalization_rejects_an_incomplete_campaign(self) -> None:
        campaign = json.loads((EXECUTION_ROOT / "campaign.json").read_text())
        campaign["results"] = campaign["results"][:-1]
        equality = json.loads(
            (EXECUTION_ROOT / "generated-authority-equality.json").read_text()
        )
        with self.assertRaisesRegex(ValueError, "execution population"):
            finalize_refresh_plan(
                root=ROOT,
                plan=self.plan,
                campaign=campaign,
                proof_records_dir=EXECUTION_ROOT / "proof-records",
                case_artifact_dir=EXECUTION_ROOT / "cases",
                generated_authority_equality=equality,
            )


if __name__ == "__main__":
    unittest.main()
