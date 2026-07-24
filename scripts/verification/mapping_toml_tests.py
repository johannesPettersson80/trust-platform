"""Focused round-trip tests for generated mapping TOML documents."""

from __future__ import annotations

import tomllib
import unittest

from scripts.verification.mapping_toml import (
    render_decisions_toml,
    render_proposals_toml,
)


class MappingTomlTests(unittest.TestCase):
    def test_accept_and_reject_decisions_round_trip_without_null_sentinels(self) -> None:
        accepted = {
            "schema_version": 1,
            "id": "MAP_DEC_" + "A" * 24,
            "proposal_id": "MAP_CAND_ONE",
            "proposal_digest": "sha256:" + "1" * 64,
            "discovery_id": "DISC_00000000000000000001",
            "outcome": "accepted",
            "reviewer_ids": ["reviewer-a"],
            "reviewed_source_digest": "sha256:" + "2" * 64,
            "assertion_reviews": [
                {
                    "assertion_id": "ASSERT_" + "A" * 24,
                    "disposition": "oracle_bound",
                    "behavior_refs": ["INV_ONE#behavior:0"],
                    "rationale": "The assertion checks the selected behavior.",
                }
            ],
            "selected_invariant_ids": ["INV_ONE"],
            "selected_behavior_refs": ["INV_ONE#behavior:0"],
            "command": "cargo test -p example exact -- --exact",
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
                "duration_class": "fast",
            },
        }
        rejected = {
            "schema_version": 1,
            "id": "MAP_DEC_" + "B" * 24,
            "proposal_id": "MAP_CAND_TWO",
            "proposal_digest": "sha256:" + "3" * 64,
            "discovery_id": "DISC_00000000000000000002",
            "outcome": "rejected",
            "reviewer_ids": ["reviewer-a"],
            "reviewed_source_digest": "sha256:" + "4" * 64,
            "assertion_reviews": [
                {
                    "assertion_id": "ASSERT_" + "B" * 24,
                    "disposition": "non_oracle",
                    "behavior_refs": [],
                    "rationale": "The assertion does not establish the candidate.",
                }
            ],
            "selected_invariant_ids": [],
            "selected_behavior_refs": [],
            "command": "",
            "command_authority": "not_applicable",
            "rejection_reason": "The assertion does not prove the behavior.",
        }
        document = {
            "schema_version": 1,
            "generator": "test-catalog-mapping-review",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "proposal_snapshots": [],
            "decisions": [accepted, rejected],
        }

        rendered = render_decisions_toml(document)

        self.assertEqual(document, tomllib.loads(rendered))
        self.assertNotIn("null", rendered)
        self.assertEqual(rendered, render_decisions_toml(document))

    def test_empty_proposal_queue_round_trips_as_explicit_array(self) -> None:
        document = {
            "schema_version": 1,
            "generator": "test-catalog-proposals",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "trace_digest": "sha256:" + "1" * 64,
            "candidate_report_digest": "sha256:" + "2" * 64,
            "last_generated": "2026-07-23T20:30:00Z",
            "demands": [],
            "demand_ranking": [],
            "structural_demand_ranking": [],
            "unmapped_code_demand_ranking": [],
            "proposals": [],
        }

        self.assertEqual(document, tomllib.loads(render_proposals_toml(document)))

    def test_proposals_round_trip_without_hand_authored_fields(self) -> None:
        document = {
            "schema_version": 1,
            "generator": "test-catalog-proposals",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "trace_digest": "sha256:" + "1" * 64,
            "candidate_report_digest": "sha256:" + "2" * 64,
            "last_generated": "2026-07-23T20:30:00Z",
            "demands": [
                {
                    "discovery_id": "DISC_00000000000000000002",
                    "candidate_tier": "C4",
                    "covered_code_ids": ["CODE_" + "B" * 64],
                    "spec_gap_refs": [],
                    "unmapped_code_ids": ["CODE_" + "B" * 64],
                }
            ],
            "demand_ranking": [],
            "structural_demand_ranking": [
                {
                    "cell_id": "DEMAND_CELL_" + "A" * 64,
                    "parent_spec_gap_ref": "SPEC_GAP_EXAMPLE",
                    "area": "verification",
                    "owner": "verification",
                    "source_kind": "python",
                    "path": "scripts/verification/example.py",
                    "qualified_symbol_parent": "scripts::verification",
                    "item_kind": "python_function",
                    "affected_discovery_ids": [
                        "DISC_00000000000000000002"
                    ],
                    "covered_code_ids": ["CODE_" + "B" * 64],
                    "tests_touching": 1,
                    "tests_exclusive": 1,
                    "tests_dominant": 0,
                    "code_facts_touched": 1,
                }
            ],
            "unmapped_code_demand_ranking": [],
            "proposals": [
                {
                    "schema_version": 1,
                    "id": "MAP_CAND_ONE",
                    "proposal_digest": "sha256:" + "3" * 64,
                    "discovery_id": "DISC_00000000000000000001",
                    "discovery_source_kind": "rust_unit_test",
                    "path": "crates/example/src/lib.rs",
                    "name": "example",
                    "ignore_state": "not_ignored",
                    "command": "cargo nextest run -p example -E 'test(=example)'",
                    "command_authority": "verified_synthesized",
                    "duration_ms": 75,
                    "duration_class": "very_fast",
                    "status": "pending",
                    "candidate_tier": "C2",
                    "candidate_invariant_ids": ["INV_ONE"],
                    "covered_code_ids": ["CODE_" + "A" * 64],
                    "specified_code_ids": ["CODE_" + "A" * 64],
                    "spec_gap_code_ids": [],
                    "spec_gap_refs": [],
                    "mutation_kill_ids": [],
                    "test_source_digest": "sha256:" + "4" * 64,
                    "assertion_inventory_digest": "sha256:" + "7" * 64,
                    "assertions": [
                        {
                            "assertion_id": "ASSERT_" + "A" * 24,
                            "kind": "macro:assert",
                            "function": "example",
                            "line": 10,
                            "expression_digest": "sha256:" + "8" * 64,
                            "expression": "assert ! (observed)",
                        }
                    ],
                    "unresolved_assertion_helpers": [],
                    "trace_record_digest": "sha256:" + "5" * 64,
                    "candidate_signal_digest": "sha256:" + "6" * 64,
                    "oracle_refs": ["SPEC_ONE#behavior"],
                    "behavior_refs": ["INV_ONE#behavior:0"],
                    "suggestions": {
                        "authoritative": False,
                        "test_class": "behavior_lock",
                        "expected_result": "Exact rationale.",
                        "expected_failure_mode": "Exact failure.",
                        "source_behavior_refs": ["INV_ONE#behavior:0"],
                    },
                }
            ],
        }

        rendered = render_proposals_toml(document)

        self.assertEqual(document, tomllib.loads(rendered))
        self.assertEqual(rendered, render_proposals_toml(document))


if __name__ == "__main__":
    unittest.main()
