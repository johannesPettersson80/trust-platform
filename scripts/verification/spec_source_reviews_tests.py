"""Tests for the identity-bound specification and public-prose review registries."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.spec_source_reviews import load_spec_source_reviews


DOCUMENT = {
    "document_id": "SPEC_DOC_A",
    "path": "docs/specs/a.md",
    "content_sha256": "a" * 64,
}
BLOCK = {
    "block_id": "PUBLIC_BLOCK_A",
    "document_id": "SPEC_DOC_A",
    "path": "docs/specs/a.md",
    "block_kind": "paragraph",
    "visible_text_sha256": "b" * 64,
}
SOURCE = {
    "id": "SPEC_A",
    "path": "docs/specs/a.md",
    "area": "compiler_iec",
    "owner": "trust-hir",
    "authority": "normative_product",
    "source_status": "active",
    "oracle_eligible": True,
}


class SpecSourceReviewTests(unittest.TestCase):
    def test_exact_live_partition_validates(self) -> None:
        with self._root() as root:
            documents, blocks, failures = load_spec_source_reviews(
                root,
                documents=[DOCUMENT],
                public_blocks=[BLOCK],
                spec_sources={"SPEC_A": SOURCE},
                invariants={"INV_A": {"id": "INV_A"}},
            )

        self.assertEqual([], failures)
        self.assertEqual(["SPEC_DOC_A"], list(documents))
        self.assertEqual(["PUBLIC_BLOCK_A"], list(blocks))

    def test_missing_live_review_fails_exhaustiveness(self) -> None:
        with self._root(public_rows="") as root:
            _, _, failures = load_spec_source_reviews(
                root,
                documents=[DOCUMENT],
                public_blocks=[BLOCK],
                spec_sources={"SPEC_A": SOURCE},
                invariants={},
            )

        self.assertIn("public-block review registry is missing live IDs", "\n".join(failures))

    def test_stale_document_digest_fails_identity_binding(self) -> None:
        with self._root(document_digest="c" * 64) as root:
            _, _, failures = load_spec_source_reviews(
                root,
                documents=[DOCUMENT],
                public_blocks=[BLOCK],
                spec_sources={"SPEC_A": SOURCE},
                invariants={},
            )

        self.assertIn("content_sha256 does not match live fact", "\n".join(failures))

    def test_structural_nonclaim_cannot_hide_paragraph(self) -> None:
        with self._root(
            block_disposition="structural_nonclaim",
            block_rationale="document_structure",
        ) as root:
            _, _, failures = load_spec_source_reviews(
                root,
                documents=[DOCUMENT],
                public_blocks=[BLOCK],
                spec_sources={"SPEC_A": SOURCE},
                invariants={},
            )

        self.assertIn("structural disposition is invalid", "\n".join(failures))

    def test_invented_oracle_binding_fails(self) -> None:
        with self._root(
            block_disposition="claim_with_mapping",
            block_rationale="explicit_invariant_or_oracle_binding",
            oracle_refs='["SPEC_MISSING"]',
        ) as root:
            _, _, failures = load_spec_source_reviews(
                root,
                documents=[DOCUMENT],
                public_blocks=[BLOCK],
                spec_sources={"SPEC_A": SOURCE},
                invariants={},
            )

        self.assertIn("names ineligible oracle SPEC_MISSING", "\n".join(failures))

    def _root(
        self,
        *,
        document_digest: str = "a" * 64,
        public_rows: str | None = None,
        block_disposition: str = "claim_without_invariant_or_oracle",
        block_rationale: str = "conservative_unbound_public_prose",
        oracle_refs: str = "[]",
    ):
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        verification = root / "verification"
        verification.mkdir()
        (verification / "spec-document-reviews.toml").write_text(
            "\n".join(
                (
                    "[[document_reviews]]",
                    "schema_version = 1",
                    'document_id = "SPEC_DOC_A"',
                    'path = "docs/specs/a.md"',
                    f'content_sha256 = "{document_digest}"',
                    'areas = ["compiler_iec"]',
                    'authority_levels = ["normative_product"]',
                    'owners = ["trust-hir"]',
                    'freshness = "current"',
                    'visibility = "public"',
                    "oracle_usable = true",
                    'classification_basis = "registered_metadata"',
                    'conflict_disposition = "registered_conflicts_reviewed"',
                    'checklist_staleness = "not_applicable"',
                    'removed_behavior_disposition = "reviewed_current"',
                    'last_reviewed = "2026-07-19"',
                    "",
                )
            )
        )
        if public_rows is None:
            public_rows = "\n".join(
                (
                    "[[public_block_reviews]]",
                    "schema_version = 1",
                    'block_id = "PUBLIC_BLOCK_A"',
                    'document_id = "SPEC_DOC_A"',
                    'path = "docs/specs/a.md"',
                    'block_kind = "paragraph"',
                    f'visible_text_sha256 = "{"b" * 64}"',
                    f'disposition = "{block_disposition}"',
                    "invariant_ids = []",
                    f"oracle_refs = {oracle_refs}",
                    f'rationale_code = "{block_rationale}"',
                    'last_reviewed = "2026-07-19"',
                    "",
                )
            )
        (verification / "public-prose-reviews.toml").write_text(public_rows)

        class _RootContext:
            def __enter__(self):
                return root

            def __exit__(self, *args):
                temporary.cleanup()

        return _RootContext()


if __name__ == "__main__":
    unittest.main()
