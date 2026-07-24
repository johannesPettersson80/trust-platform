"""Focused source-provenance checks for semantic mapping review."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_review_cli import (
    _validate_proposal_source_commit,
)


class MappingReviewCliTests(unittest.TestCase):
    def test_review_requires_the_exact_proposal_source_commit(self) -> None:
        source_commit = "a" * 40

        _validate_proposal_source_commit(
            {"source_commit": source_commit},
            source_commit,
        )

        with self.assertRaisesRegex(ValueError, "source commit is stale"):
            _validate_proposal_source_commit(
                {"source_commit": "b" * 40},
                source_commit,
            )

        with self.assertRaisesRegex(ValueError, "source commit is stale"):
            _validate_proposal_source_commit(
                {"source_commit": "not-a-commit"},
                source_commit,
            )


if __name__ == "__main__":
    unittest.main()
