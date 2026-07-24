"""Focused tests for compiling numeric semantic review recommendations."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.mapping_review_recommendations import (
    compile_review_recommendations,
)


ROOT = Path(__file__).resolve().parents[2]


def _proposals() -> dict:
    return {
        "schema_version": 1,
        "source_commit": "a" * 40,
        "proposals": [{"id": "one"}, {"id": "two"}],
    }


def _recommendation(
    reviewer: str,
    *,
    accepted: bool = True,
    indices: tuple[int, ...] = (1, 2),
) -> dict:
    reviews = []
    for index in indices:
        if accepted and index == 1:
            reviews.append(
                {
                    "proposal_index": index,
                    "outcome": "accepted",
                    "evidence": "Assertion one exactly checks behavior one.",
                    "selected_invariant_indices": [1],
                    "oracle_bindings": [
                        {"assertion_index": 1, "behavior_indices": [1]}
                    ],
                    "setup_assertion_indices": [],
                    "non_oracle_assertion_indices": [],
                    "requires_network": False,
                }
            )
        else:
            reviews.append(
                {
                    "proposal_index": index,
                    "outcome": "rejected",
                    "evidence": "The assertion checks a different behavior.",
                    "rejection_reason": "Assertions do not establish the candidate.",
                }
            )
    return {
        "schema_version": 1,
        "reviewer_id": reviewer,
        "range": [1, 2],
        "reviews": reviews,
    }


def _confirmation(reviewer: str, indices: tuple[int, ...] = (1,)) -> dict:
    return {
        "schema_version": 1,
        "reviewer_id": reviewer,
        "range": [1, 2],
        "reviews": [
            {
                "proposal_index": index,
                "outcome": "confirmed",
                "evidence": "Independent review confirmed every semantic binding.",
            }
            for index in indices
        ],
    }


class MappingReviewRecommendationsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.recommendation_schema = json.loads(
            (
                ROOT
                / "verification/schemas/test-catalog-mapping-review-recommendations.schema.json"
            ).read_text()
        )
        cls.selection_schema = json.loads(
            (
                ROOT
                / "verification/schemas/test-catalog-mapping-review-selection.schema.json"
            ).read_text()
        )

    def compile(self, documents: list[dict], *, complete: bool = True) -> dict:
        return compile_review_recommendations(
            proposal_document=_proposals(),
            recommendation_documents=documents,
            recommendation_schema=self.recommendation_schema,
            selection_schema=self.selection_schema,
            require_complete_queue=complete,
        )

    def test_injects_digest_and_merges_matching_independent_reviewers(self) -> None:
        selection = self.compile(
            [
                _recommendation("reviewer-a"),
                _confirmation("reviewer-b"),
            ]
        )

        self.assertRegex(selection["proposal_queue_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(
            ["reviewer-a", "reviewer-b"],
            selection["reviews"][0]["reviewer_ids"],
        )
        self.assertTrue(selection["reviews"][0]["confirm_generated_catalog"])
        self.assertNotIn("evidence", selection["reviews"][0])
        self.assertEqual(
            ["reviewer-a"],
            selection["reviews"][1]["reviewer_ids"],
        )

    def test_conflicts_and_incomplete_queue_fail_closed(self) -> None:
        first = _recommendation("reviewer-a")
        conflicting = _recommendation("reviewer-b", indices=(1,))
        conflicting["reviews"][0]["requires_network"] = True
        with self.assertRaisesRegex(ValueError, "reviewers conflict"):
            self.compile([first, conflicting])

        with self.assertRaisesRegex(ValueError, "complete queue"):
            self.compile([_recommendation("reviewer-a", indices=(1,))])

        with self.assertRaisesRegex(ValueError, "cannot confirm a rejection"):
            self.compile(
                [
                    _recommendation(
                        "reviewer-a",
                        accepted=False,
                        indices=(1, 2),
                    ),
                    _confirmation("reviewer-b"),
                ]
            )

        with self.assertRaisesRegex(ValueError, "lacks a semantic decision"):
            self.compile([_confirmation("reviewer-b")], complete=False)

    def test_closed_schema_rejects_generated_ids_and_unknown_fields(self) -> None:
        forged = copy.deepcopy(_recommendation("reviewer-a"))
        forged["reviews"][0]["proposal_id"] = "MAP_CAND_FORGED"
        with self.assertRaisesRegex(ValueError, "recommendation 1"):
            self.compile([forged])


if __name__ == "__main__":
    unittest.main()
