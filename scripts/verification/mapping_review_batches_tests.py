"""Focused tests for deterministic semantic mapping-review batches."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.mapping_review_batches import build_review_batches
from scripts.verification.test_catalog_json_schema import (
    validate_json_schema_instance,
)


ROOT = Path(__file__).resolve().parents[2]


def _proposal(index: int, *, tier: str, invariant: str, behavior: str) -> dict:
    return {
        "proposal_index": index,
        "candidate_tier": tier,
        "invariants": [{"invariant_id": invariant}],
        "behaviors": [{"behavior_ref": behavior}],
    }


class MappingReviewBatchesTests(unittest.TestCase):
    def test_groups_exact_semantics_caps_batches_and_preserves_indices(self) -> None:
        card = {
            "schema_version": 1,
            "generator": "test-catalog-mapping-review-card",
            "generator_version": "2",
            "source_commit": "a" * 40,
            "proposal_queue_digest": "sha256:" + "b" * 64,
            "proposals": [
                _proposal(3, tier="C2", invariant="INV_B", behavior="INV_B#behavior:0"),
                _proposal(1, tier="C1", invariant="INV_A", behavior="INV_A#behavior:0"),
                _proposal(2, tier="C1", invariant="INV_A", behavior="INV_A#behavior:0"),
                _proposal(4, tier="C1", invariant="INV_A", behavior="INV_A#behavior:0"),
            ],
        }

        manifest, cards = build_review_batches(card, max_batch_size=2)

        self.assertEqual(2, manifest["semantic_group_count"])
        self.assertEqual(3, manifest["batch_count"])
        self.assertEqual(4, manifest["proposal_count"])
        self.assertEqual(
            [1, 2, 4, 3],
            [
                proposal["proposal_index"]
                for batch in cards
                for proposal in batch["proposals"]
            ],
        )
        self.assertTrue(all(len(batch["proposals"]) <= 2 for batch in cards))
        for batch, row in zip(cards, manifest["batches"], strict=True):
            self.assertEqual(
                row["proposal_indices"],
                [proposal["proposal_index"] for proposal in batch["proposals"]],
            )
            self.assertEqual(
                1,
                len(
                    {
                        (
                            proposal["candidate_tier"],
                            tuple(
                                invariant["invariant_id"]
                                for invariant in proposal["invariants"]
                            ),
                            tuple(
                                behavior["behavior_ref"]
                                for behavior in proposal["behaviors"]
                            ),
                        )
                        for proposal in batch["proposals"]
                    }
                ),
            )
        schema = json.loads(
            (
                ROOT
                / "verification/schemas/test-catalog-mapping-review-batches.schema.json"
            ).read_text()
        )
        self.assertEqual([], validate_json_schema_instance(manifest, schema))

    def test_rejects_duplicate_indices_and_invalid_batch_size(self) -> None:
        proposal = _proposal(
            1,
            tier="C1",
            invariant="INV_A",
            behavior="INV_A#behavior:0",
        )
        card = {"proposals": [proposal, dict(proposal)]}

        with self.assertRaisesRegex(ValueError, "positive and unique"):
            build_review_batches(card, max_batch_size=10)
        with self.assertRaisesRegex(ValueError, "positive integer"):
            build_review_batches({"proposals": []}, max_batch_size=0)


if __name__ == "__main__":
    unittest.main()
