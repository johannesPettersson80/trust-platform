"""Focused tests for candidate report and quarantine selection."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_candidates_cli import (
    build_candidate_document,
    initial_decision_document,
    select_quarantine_candidates,
)


class MappingCandidatesCliTests(unittest.TestCase):
    def test_decision_ledger_initialization_is_generated_not_hand_authored(self) -> None:
        document = initial_decision_document("a" * 40)

        self.assertEqual("test-catalog-mapping-review", document.get("generator"))
        self.assertEqual([], document.get("proposal_snapshots"))
        self.assertEqual([], document.get("decisions"))
        self.assertEqual("a" * 40, document.get("source_commit"))

    def test_candidate_document_adds_generated_provenance(self) -> None:
        document = build_candidate_document(
            source_commit="a" * 40,
            trace_digest="sha256:" + "1" * 64,
            analysis={"candidates": [], "demands": []},
        )

        self.assertEqual("test-mapping-candidates", document["generator"])
        self.assertEqual([], document["candidates"])
        self.assertEqual([], document["demands"])

    def test_only_nonmapped_reviewed_candidates_enter_quarantine(self) -> None:
        candidates = [
            {"discovery_id": "DISC_MAPPED"},
            {"discovery_id": "DISC_PENDING"},
            {"discovery_id": "DISC_WRONG_REASON"},
            {
                "discovery_id": "DISC_CODE_MAP_STALE",
                "unmapped_code_ids": ["CODE_LIVE_UNMAPPED"],
            },
        ]
        selected = select_quarantine_candidates(
            candidates,
            catalog_discovery_ids={"DISC_MAPPED"},
            denominator_reviews={
                "DISC_MAPPED": {"disposition": "catalog_mapped"},
                "DISC_PENDING": {"disposition": "reviewed_nonmapping"},
                "DISC_WRONG_REASON": {"disposition": "catalog_mapped"},
                "DISC_CODE_MAP_STALE": {"disposition": "reviewed_nonmapping"},
            },
        )

        self.assertEqual(["DISC_PENDING"], [row["discovery_id"] for row in selected])


if __name__ == "__main__":
    unittest.main()
