"""Focused tests for index-based hidden-ground-truth semantic review."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_pilot_review import build_pilot_review


def _candidate() -> dict[str, object]:
    return {
        "discovery_id": "DISC_00000000000000000001",
        "candidate_invariant_ids": ["INV_WRONG", "INV_RIGHT"],
    }


def _inventory() -> dict[str, object]:
    return {
        "source_digest": "sha256:" + "1" * 64,
        "inventory_digest": "sha256:" + "2" * 64,
        "assertions": [
            {"assertion_id": "ASSERT_" + "A" * 24},
            {"assertion_id": "ASSERT_" + "B" * 24},
        ],
        "unresolved_assertion_helpers": [],
    }


class MappingPilotReviewTests(unittest.TestCase):
    def test_display_indices_generate_exact_ids_and_complete_assertion_review(self) -> None:
        review = build_pilot_review(
            candidate=_candidate(),
            fact={},
            assertion_inventory=_inventory(),
            invariants={
                "INV_RIGHT": {
                    "risk": "maintenance",
                    "behavior": [{"rationale": "exact"}],
                }
            },
            invariant_index=2,
            oracle_assertion_bindings={2: [1]},
            setup_assertion_indices=[1],
            non_oracle_assertion_indices=[],
            reviewer_ids=["reviewer-a"],
        )

        self.assertEqual(["INV_RIGHT"], review["selected_invariant_ids"])
        self.assertEqual(
            ["INV_RIGHT#behavior:0"], review["selected_behavior_refs"]
        )
        self.assertEqual(
            ["ASSERT_" + "A" * 24, "ASSERT_" + "B" * 24],
            [
                assertion["assertion_id"]
                for assertion in review["assertion_reviews"]
            ],
        )
        self.assertEqual(
            ["setup_or_precondition", "oracle_bound"],
            [
                assertion["disposition"]
                for assertion in review["assertion_reviews"]
            ],
        )

    def test_each_oracle_assertion_binds_only_its_selected_behaviors(self) -> None:
        review = build_pilot_review(
            candidate=_candidate(),
            fact={},
            assertion_inventory=_inventory(),
            invariants={
                "INV_RIGHT": {
                    "risk": "maintenance",
                    "behavior": [
                        {"rationale": "accept"},
                        {"rationale": "reject"},
                    ],
                }
            },
            invariant_index=2,
            oracle_assertion_bindings={1: [1], 2: [2]},
            setup_assertion_indices=[],
            non_oracle_assertion_indices=[],
            reviewer_ids=["reviewer-a"],
        )
        by_id = {
            row["assertion_id"]: row["behavior_refs"]
            for row in review["assertion_reviews"]
        }

        self.assertEqual(
            ["INV_RIGHT#behavior:0"],
            by_id["ASSERT_" + "A" * 24],
        )
        self.assertEqual(
            ["INV_RIGHT#behavior:1"],
            by_id["ASSERT_" + "B" * 24],
        )

    def test_sampling_or_unresolved_helper_cannot_generate_acceptance(self) -> None:
        with self.assertRaisesRegex(ValueError, "classify every"):
            build_pilot_review(
                candidate=_candidate(),
                fact={},
                assertion_inventory=_inventory(),
                invariants={
                    "INV_RIGHT": {
                        "risk": "maintenance",
                        "behavior": [{}],
                    }
                },
                invariant_index=2,
                oracle_assertion_bindings={2: [1]},
                setup_assertion_indices=[],
                non_oracle_assertion_indices=[],
                reviewer_ids=["reviewer-a"],
            )

        inventory = _inventory()
        inventory["unresolved_assertion_helpers"] = ["expect_external:unresolved"]
        with self.assertRaisesRegex(ValueError, "unresolved assertion helpers"):
            build_pilot_review(
                candidate=_candidate(),
                fact={},
                assertion_inventory=inventory,
                invariants={
                    "INV_RIGHT": {
                        "risk": "maintenance",
                        "behavior": [{}],
                    }
                },
                invariant_index=2,
                oracle_assertion_bindings={2: [1]},
                setup_assertion_indices=[1],
                non_oracle_assertion_indices=[],
                reviewer_ids=["reviewer-a"],
            )


if __name__ == "__main__":
    unittest.main()
