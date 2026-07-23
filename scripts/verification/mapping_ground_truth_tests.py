"""Focused tests for zero-false mapping-pilot qualification."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_ground_truth import (
    evaluate_ground_truth,
    partition_ground_truth_candidates,
)


class MappingGroundTruthTests(unittest.TestCase):
    def test_exact_candidate_is_eligible_but_not_auto_accepted(self) -> None:
        report = evaluate_ground_truth(
            candidates=[
                {
                    "discovery_id": "DISC_ONE",
                    "candidate_invariant_ids": ["INV_ONE"],
                }
            ],
            expected_invariants={"DISC_ONE": ["INV_ONE"]},
            accepted_reviews=[
                {
                    "discovery_id": "DISC_ONE",
                    "selected_invariant_ids": ["INV_ONE"],
                }
            ],
        )

        self.assertTrue(report["zero_false_accepted_mappings"])
        self.assertEqual(["DISC_ONE"], report["exact_candidates"])
        self.assertEqual(["DISC_ONE"], report["exact_accepted_mappings"])
        self.assertFalse(report["creates_catalog_mappings"])

    def test_extra_invariant_is_a_false_candidate_and_missing_is_visible(self) -> None:
        report = evaluate_ground_truth(
            candidates=[
                {
                    "discovery_id": "DISC_ONE",
                    "candidate_invariant_ids": ["INV_ONE", "INV_WRONG"],
                }
            ],
            expected_invariants={"DISC_ONE": ["INV_ONE", "INV_MISSING"]},
            accepted_reviews=[
                {
                    "discovery_id": "DISC_ONE",
                    "selected_invariant_ids": ["INV_ONE"],
                }
            ],
        )

        self.assertFalse(report["zero_false_candidates"])
        self.assertTrue(report["zero_false_accepted_mappings"])
        self.assertEqual(
            ["INV_WRONG"],
            report["candidate_results"][0]["false_invariant_ids"],
        )
        self.assertEqual(
            ["INV_MISSING"],
            report["candidate_results"][0]["missed_invariant_ids"],
        )

    def test_pilot_requires_expected_rows_for_every_candidate(self) -> None:
        with self.assertRaisesRegex(ValueError, "outside ground truth"):
            evaluate_ground_truth(
                candidates=[
                    {
                        "discovery_id": "DISC_UNKNOWN",
                        "candidate_invariant_ids": ["INV_ONE"],
                    }
                ],
                expected_invariants={},
                accepted_reviews=[],
            )

    def test_unknown_candidate_is_quarantined_outside_hidden_truth(self) -> None:
        known, unknown = partition_ground_truth_candidates(
            candidates=[
                {
                    "discovery_id": "DISC_UNKNOWN",
                    "candidate_invariant_ids": ["INV_WRONG"],
                },
                {
                    "discovery_id": "DISC_ONE",
                    "candidate_invariant_ids": ["INV_ONE"],
                },
            ],
            expected_invariants={"DISC_ONE": ["INV_ONE"]},
        )

        self.assertEqual(["DISC_ONE"], [row["discovery_id"] for row in known])
        self.assertEqual(["DISC_UNKNOWN"], unknown)

    def test_false_reviewed_acceptance_fails_even_when_candidate_is_broad(self) -> None:
        report = evaluate_ground_truth(
            candidates=[
                {
                    "discovery_id": "DISC_ONE",
                    "candidate_invariant_ids": ["INV_ONE", "INV_WRONG"],
                }
            ],
            expected_invariants={"DISC_ONE": ["INV_ONE"]},
            accepted_reviews=[
                {
                    "discovery_id": "DISC_ONE",
                    "selected_invariant_ids": ["INV_WRONG"],
                }
            ],
        )

        self.assertFalse(report["zero_false_accepted_mappings"])
        self.assertEqual(
            ["INV_WRONG"],
            report["accepted_results"][0]["false_invariant_ids"],
        )

    def test_vacuous_no_acceptance_cannot_pass_zero_false_gate(self) -> None:
        report = evaluate_ground_truth(
            candidates=[],
            expected_invariants={},
            accepted_reviews=[],
        )

        self.assertFalse(report["zero_false_accepted_mappings"])


if __name__ == "__main__":
    unittest.main()
