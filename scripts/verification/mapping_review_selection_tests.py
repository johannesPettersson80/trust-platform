"""Focused tests for ID-free mapping review selections."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.mapping_review_selection import (
    build_review_card,
    build_review_input_from_selection,
    canonical_digest,
)
from scripts.verification.mapping_review import (
    build_mapping_decision,
    validate_mapping_decisions,
)
from scripts.verification.test_catalog_json_schema import (
    validate_json_schema_instance,
)


ROOT = Path(__file__).resolve().parents[2]


def _proposal_document() -> dict:
    return {
        "schema_version": 1,
        "generator": "test-catalog-proposals",
        "generator_version": "1",
        "source_commit": "a" * 40,
        "trace_digest": "sha256:" + "1" * 64,
        "candidate_report_digest": "sha256:" + "2" * 64,
        "last_generated": "2026-07-23T20:30:00Z",
        "demands": [],
        "demand_ranking": [],
        "unmapped_code_demand_ranking": [],
        "proposals": [
            {
                "schema_version": 1,
                "id": "MAP_CAND_00000000000000000001_A1B2C3D4",
                "proposal_digest": "sha256:" + "3" * 64,
                "discovery_id": "DISC_00000000000000000001",
                "discovery_source_kind": "rust_unit_test",
                "path": "crates/example/src/lib.rs",
                "name": "exact_case",
                "ignore_state": "not_ignored",
                "command": "cargo nextest run -p example -E 'test(=exact_case)'",
                "command_authority": "verified_synthesized",
                "duration_ms": 250,
                "duration_class": "fast",
                "status": "pending",
                "candidate_tier": "C2",
                "candidate_invariant_ids": ["INV_ONE"],
                "covered_code_ids": ["CODE_" + "A" * 64],
                "specified_code_ids": ["CODE_" + "A" * 64],
                "spec_gap_code_ids": [],
                "spec_gap_refs": [],
                "mutation_kill_ids": [],
                "test_source_digest": "sha256:" + "4" * 64,
                "assertion_inventory_digest": "sha256:" + "5" * 64,
                "assertions": [
                    {
                        "assertion_id": "ASSERT_" + "A" * 24,
                        "kind": "macro:assert",
                        "function": "exact_case",
                        "line": 10,
                        "expression_digest": "sha256:" + "6" * 64,
                        "expression": "assert_eq!(actual, expected)",
                    },
                    {
                        "assertion_id": "ASSERT_" + "B" * 24,
                        "kind": "method:unwrap",
                        "function": "exact_case",
                        "line": 9,
                        "expression_digest": "sha256:" + "7" * 64,
                        "expression": "setup.unwrap()",
                    },
                ],
                "unresolved_assertion_helpers": [],
                "trace_record_digest": "sha256:" + "8" * 64,
                "candidate_signal_digest": "sha256:" + "9" * 64,
                "oracle_refs": ["SPEC_EXAMPLE_001#exact"],
                "behavior_refs": ["INV_ONE#behavior:0"],
                "suggestions": {
                    "authoritative": False,
                    "test_class": "behavior_lock",
                    "expected_result": "Candidate only.",
                    "expected_failure_mode": "Candidate only.",
                    "source_behavior_refs": ["INV_ONE#behavior:0"],
                },
            }
        ],
    }


def _invariants() -> dict:
    return {
        "INV_ONE": {
            "id": "INV_ONE",
            "title": "Exact behavior",
            "area": "runtime_safety",
            "owner": "trust-runtime",
            "risk": "maintenance",
            "gates": ["pr"],
            "oracle": {"ref": "SPEC_EXAMPLE_001#exact"},
            "behavior": [
                {
                    "case_family": "happy_path",
                    "outcome": "accept_value",
                    "rationale": "The exact expected value is preserved.",
                }
            ],
        }
    }


def _accepted_selection(document: dict) -> dict:
    return {
        "schema_version": 1,
        "proposal_queue_digest": canonical_digest(document),
        "reviews": [
            {
                "proposal_index": 1,
                "outcome": "accepted",
                "reviewer_ids": ["reviewer-a"],
                "selected_invariant_indices": [1],
                "oracle_bindings": [
                    {"assertion_index": 1, "behavior_indices": [1]}
                ],
                "setup_assertion_indices": [2],
                "non_oracle_assertion_indices": [],
                "confirm_generated_catalog": True,
                "requires_network": False,
            }
        ],
    }


class MappingReviewSelectionTests(unittest.TestCase):
    def test_numbered_selection_generates_all_ids_command_and_catalog_fields(self) -> None:
        proposals = _proposal_document()
        card = build_review_card(
            proposal_document=proposals,
            invariants=_invariants(),
        )
        review = build_review_input_from_selection(
            proposal_document=proposals,
            selection_document=_accepted_selection(proposals),
            invariants=_invariants(),
        )["reviews"][0]

        self.assertEqual(1, card["proposals"][0]["proposal_index"])
        self.assertEqual(1, card["proposals"][0]["invariants"][0]["invariant_index"])
        self.assertEqual(1, card["proposals"][0]["behaviors"][0]["behavior_index"])
        self.assertEqual(2, len(card["proposals"][0]["assertions"]))
        self.assertEqual(
            proposals["proposals"][0]["id"],
            review["proposal_id"],
        )
        self.assertEqual(
            proposals["proposals"][0]["command"],
            review["command"],
        )
        self.assertEqual("verified_exact", review["command_authority"])
        self.assertEqual(["INV_ONE"], review["selected_invariant_ids"])
        self.assertEqual(
            ["INV_ONE#behavior:0"], review["selected_behavior_refs"]
        )
        self.assertEqual("runtime_safety", review["confirmed_catalog"]["area"])
        self.assertEqual("trust-runtime", review["confirmed_catalog"]["owner"])
        self.assertEqual(["pr"], review["confirmed_catalog"]["suite_tiers"])
        self.assertEqual("fast", review["confirmed_catalog"]["duration_class"])
        self.assertEqual(
            ["oracle_bound", "setup_or_precondition"],
            [
                row["disposition"]
                for row in review["assertion_reviews"]
            ],
        )
        proposal = proposals["proposals"][0]
        decision = build_mapping_decision(
            proposal=proposal,
            review_input=review,
        )
        self.assertEqual(
            [],
            validate_mapping_decisions(
                decisions=[decision],
                proposals={proposal["id"]: proposal},
                invariants=_invariants(),
                candidate_generator_id="test-catalog-proposals",
            ),
        )

    def test_selection_cannot_type_generated_ids_or_survive_queue_drift(self) -> None:
        proposals = _proposal_document()
        selection = _accepted_selection(proposals)
        selection["reviews"][0]["proposal_id"] = proposals["proposals"][0]["id"]

        with self.assertRaisesRegex(ValueError, "selection fields drift"):
            build_review_input_from_selection(
                proposal_document=proposals,
                selection_document=selection,
                invariants=_invariants(),
            )

        selection = _accepted_selection(proposals)
        changed = copy.deepcopy(proposals)
        changed["proposals"][0]["duration_ms"] = 251
        with self.assertRaisesRegex(ValueError, "queue digest is stale"):
            build_review_input_from_selection(
                proposal_document=changed,
                selection_document=selection,
                invariants=_invariants(),
            )

    def test_acceptance_requires_exact_exhaustive_assertion_partition(self) -> None:
        proposals = _proposal_document()
        selection = _accepted_selection(proposals)
        selection["reviews"][0]["setup_assertion_indices"] = []

        with self.assertRaisesRegex(ValueError, "classify every assertion"):
            build_review_input_from_selection(
                proposal_document=proposals,
                selection_document=selection,
                invariants=_invariants(),
            )

    def test_each_oracle_assertion_has_its_own_behavior_binding(self) -> None:
        proposals = _proposal_document()
        proposals["proposals"][0]["behavior_refs"].append(
            "INV_ONE#behavior:1"
        )
        proposals["proposals"][0]["suggestions"]["source_behavior_refs"].append(
            "INV_ONE#behavior:1"
        )
        invariants = _invariants()
        invariants["INV_ONE"]["behavior"].append(
            {
                "case_family": "wrong_type_or_shape",
                "outcome": "reject",
                "rationale": "The exact malformed value is rejected.",
            }
        )
        selection = _accepted_selection(proposals)
        accepted = selection["reviews"][0]
        accepted["oracle_bindings"] = [
            {"assertion_index": 1, "behavior_indices": [1]},
            {"assertion_index": 2, "behavior_indices": [2]},
        ]
        accepted["setup_assertion_indices"] = []

        review = build_review_input_from_selection(
            proposal_document=proposals,
            selection_document=selection,
            invariants=invariants,
        )["reviews"][0]
        by_id = {
            row["assertion_id"]: row["behavior_refs"]
            for row in review["assertion_reviews"]
        }

        self.assertEqual(
            ["INV_ONE#behavior:0"],
            by_id["ASSERT_" + "A" * 24],
        )
        self.assertEqual(
            ["INV_ONE#behavior:1"],
            by_id["ASSERT_" + "B" * 24],
        )

    def test_selection_and_card_schemas_are_closed(self) -> None:
        proposals = _proposal_document()
        selection = _accepted_selection(proposals)
        card = build_review_card(
            proposal_document=proposals,
            invariants=_invariants(),
        )
        selection_schema = json.loads(
            (
                ROOT
                / "verification/schemas/test-catalog-mapping-review-selection.schema.json"
            ).read_text()
        )
        card_schema = json.loads(
            (
                ROOT
                / "verification/schemas/test-catalog-mapping-review-card.schema.json"
            ).read_text()
        )

        self.assertEqual(
            [], validate_json_schema_instance(selection, selection_schema)
        )
        self.assertEqual([], validate_json_schema_instance(card, card_schema))
        forged = copy.deepcopy(selection)
        forged["reviews"][0]["proposal_id"] = "MAP_CAND_FORGED"
        self.assertTrue(
            validate_json_schema_instance(forged, selection_schema)
        )


if __name__ == "__main__":
    unittest.main()
