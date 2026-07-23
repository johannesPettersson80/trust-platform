"""Focused rendering tests for numbered mapping review cards."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_review_card_cli import (
    render_review_card_markdown,
)
from scripts.verification.mapping_review_selection_tests import (
    _invariants,
    _proposal_document,
)
from scripts.verification.mapping_review_selection import build_review_card


class MappingReviewCardCliTests(unittest.TestCase):
    def test_markdown_directs_reviewers_to_indices_not_generated_metadata(self) -> None:
        card = build_review_card(
            proposal_document=_proposal_document(),
            invariants=_invariants(),
        )

        markdown = render_review_card_markdown(card)

        self.assertIn("## Proposal 1", markdown)
        self.assertIn("### Invariants", markdown)
        self.assertIn("### Behaviors", markdown)
        self.assertIn("### Assertions", markdown)
        self.assertIn("Submit only displayed one-based indices", markdown)
        self.assertIn("Do not copy proposal, discovery, invariant", markdown)


if __name__ == "__main__":
    unittest.main()
