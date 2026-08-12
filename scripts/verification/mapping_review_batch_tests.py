"""Focused tests for generated semantic-review batch replacements."""

from __future__ import annotations

import tomllib
import unittest

from scripts.verification.mapping_review_batch import build_review_batch


def _proposal(number: int) -> dict[str, object]:
    suffix = f"{number:020d}"
    invariant = f"INV_{number}"
    return {
        "schema_version": 2,
        "id": f"MAP_CAND_{suffix}_A1B2C3D4",
        "proposal_digest": "sha256:" + str(number) * 64,
        "discovery_id": f"DISC_{suffix}",
        "discovery_source_kind": "rust_unit_test",
        "path": f"crates/example/src/case_{number}.rs",
        "name": f"case_{number}",
        "ignore_state": "not_ignored",
        "command": f"cargo nextest run -p example -E 'test(=case_{number})'",
        "command_authority": "verified_synthesized",
        "duration_ms": 75,
        "duration_class": "very_fast",
        "status": "pending",
        "candidate_tier": "C2",
        "candidate_invariant_ids": [invariant],
        "covered_code_ids": ["CODE_" + "A" * 64],
        "specified_code_ids": ["CODE_" + "A" * 64],
        "spec_gap_code_ids": [],
        "spec_gap_refs": [],
        "mutation_kill_ids": [],
        "test_source_digest": "sha256:" + "a" * 64,
        "assertion_inventory_digest": "sha256:" + "d" * 64,
        "assertion_source_digests": {
            f"crates/example/src/case_{number}.rs": "sha256:" + "a" * 64,
        },
        "assertions": [
            {
                "assertion_id": "ASSERT_" + f"{number:X}" * 24,
                "kind": "macro:assert",
                "source_path": f"crates/example/src/case_{number}.rs",
                "function": f"case_{number}",
                "line": 10,
                "expression_digest": "sha256:" + "e" * 64,
                "expression": "assert ! (observed)",
            }
        ],
        "unresolved_assertion_helpers": [],
        "trace_record_digest": "sha256:" + "b" * 64,
        "candidate_signal_digest": "sha256:" + "c" * 64,
        "oracle_refs": [f"SPEC_{number}#exact"],
        "behavior_refs": [f"{invariant}#behavior:0"],
        "suggestions": {
            "authoritative": False,
            "test_class": "unit",
            "expected_result": "Candidate only.",
            "expected_failure_mode": "Candidate only.",
            "source_behavior_refs": [f"{invariant}#behavior:0"],
        },
    }


def _accepted_input(proposal: dict[str, object]) -> dict[str, object]:
    invariant = proposal["candidate_invariant_ids"][0]
    return {
        "proposal_id": proposal["id"],
        "outcome": "accepted",
        "reviewer_ids": ["reviewer-a"],
        "assertion_reviews": [
            {
                "assertion_id": proposal["assertions"][0]["assertion_id"],
                "disposition": "oracle_bound",
                "behavior_refs": [f"{invariant}#behavior:0"],
                "rationale": "The assertion checks the selected behavior.",
            }
        ],
        "selected_invariant_ids": [invariant],
        "selected_behavior_refs": [f"{invariant}#behavior:0"],
        "command": proposal["command"],
        "command_authority": "verified_exact",
        "confirmed_catalog": {
            "test_class": "unit",
            "area": "verification",
            "owner": "verification",
            "expected_result": "The exact assertion passes.",
            "expected_failure_mode": "The assertion accepts a wrong value.",
            "suite_tiers": ["pr"],
            "requires_hardware": False,
            "requires_network": False,
            "duration_class": proposal["duration_class"],
        },
    }


class MappingReviewBatchTests(unittest.TestCase):
    def test_accept_and_reject_batch_generates_every_ledger_replacement(self) -> None:
        accepted = _proposal(1)
        rejected = _proposal(2)
        proposal_document = {
            "schema_version": 1,
            "generator": "test-catalog-proposals",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "trace_digest": "sha256:" + "d" * 64,
            "candidate_report_digest": "sha256:" + "e" * 64,
            "last_generated": "2026-07-23T20:30:00Z",
            "demands": [],
            "demand_ranking": [],
            "structural_demand_ranking": [],
            "unmapped_code_demand_ranking": [],
            "proposals": [accepted, rejected],
        }
        decision_document = {
            "schema_version": 1,
            "generator": "test-catalog-mapping-review",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "proposal_snapshots": [],
            "decisions": [],
        }
        reviews = [_accepted_input(accepted)]
        reviews.append(
            {
                "proposal_id": rejected["id"],
                "outcome": "rejected",
                "reviewer_ids": ["reviewer-a"],
                "assertion_reviews": [
                    {
                        "assertion_id": rejected["assertions"][0]["assertion_id"],
                        "disposition": "non_oracle",
                        "behavior_refs": [],
                        "rationale": "The assertion does not establish this candidate.",
                    }
                ],
                "selected_invariant_ids": [],
                "selected_behavior_refs": [],
                "command": "",
                "command_authority": "not_applicable",
                "rejection_reason": "The assertion does not establish the candidate behavior.",
            }
        )
        denominator = {
            "schema_version": 1,
            "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
            "title": "Exact denominator",
            "denominator_basis": "all_live_existing_test_scanner_facts",
            "review_basis": "explicit_per_discovery_id_no_semantic_inference",
            "reason_definitions": {
                "no_reviewed_spec_or_invariant_binding": "reason",
            },
            "last_reviewed": "2026-07-22",
            "reviews": [
                {
                    "discovery_id": proposal["discovery_id"],
                    "discovery_source_kind": proposal["discovery_source_kind"],
                    "path": proposal["path"],
                    "name": proposal["name"],
                    "ignore_state": proposal["ignore_state"],
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "no_reviewed_spec_or_invariant_binding",
                    "last_reviewed": "2026-07-22",
                }
                for proposal in (accepted, rejected)
            ],
        }
        invariants = {
            "INV_1": {"risk": "maintenance", "oracle": {"ref": "SPEC_1#exact"}},
            "INV_2": {"risk": "maintenance", "oracle": {"ref": "SPEC_2#exact"}},
        }

        plan = build_review_batch(
            proposal_document=proposal_document,
            decision_document=decision_document,
            review_inputs=reviews,
            catalog_source="tests = []\n",
            denominator_document=denominator,
            invariant_sources={
                "INV_1": (
                    "verification/invariants/verification/INV_1.toml",
                    """
schema_version = 1
id = "INV_1"
tests = ["TEST_AUTO_00000000000000000001"]
missing = ["cataloged_behavior_tests"]
[[behavior]]
case_family = "happy_path"
[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Awaiting catalog acceptance."
""",
                ),
                "INV_2": (
                    "verification/invariants/verification/INV_2.toml",
                    'schema_version = 1\nid = "INV_2"\ntests = []\n',
                ),
            },
            invariants=invariants,
            reviewed_date="2026-07-23",
            source_commit="b" * 40,
        )

        self.assertIn("verification/test-catalog.toml", plan.replacements)
        catalog = tomllib.loads(
            plan.replacements["verification/test-catalog.toml"].decode()
        )
        denominator_after = tomllib.loads(
            plan.replacements[
                "verification/test-catalog-denominator.toml"
            ].decode()
        )
        proposal_after = tomllib.loads(
            plan.replacements[
                "verification/test-catalog-proposals.toml"
            ].decode()
        )
        decisions_after = tomllib.loads(
            plan.replacements[
                "verification/test-catalog-mapping-decisions.toml"
            ].decode()
        )

        self.assertEqual(1, len(catalog["tests"]))
        self.assertEqual(
            accepted["assertion_source_digests"],
            catalog["tests"][0]["mapping_assertion_source_digests"],
        )
        self.assertEqual([], proposal_after["proposals"])
        self.assertEqual(2, len(decisions_after["proposal_snapshots"]))
        accepted_snapshot = next(
            snapshot
            for snapshot in decisions_after["proposal_snapshots"]
            if snapshot["id"] == accepted["id"]
        )
        self.assertEqual(
            accepted["assertion_source_digests"],
            accepted_snapshot["assertion_source_digests"],
        )
        self.assertEqual(2, len(decisions_after["decisions"]))
        self.assertEqual(
            ["catalog_mapped", "reviewed_nonmapping"],
            [row["disposition"] for row in denominator_after["reviews"]],
        )
        self.assertIn(
            "TEST_AUTO_00000000000000000001",
            tomllib.loads(
                plan.replacements[
                    "verification/invariants/verification/INV_1.toml"
                ].decode()
            )["tests"],
        )
        self.assertEqual(
            1,
            tomllib.loads(
                plan.replacements[
                    "verification/invariants/verification/INV_1.toml"
                ].decode()
            )["tests"].count("TEST_AUTO_00000000000000000001"),
        )
        self.assertEqual(
            ["MAP_CAND_00000000000000000002_A1B2C3D4"],
            list(plan.rejected_proposal_ids),
        )


if __name__ == "__main__":
    unittest.main()
