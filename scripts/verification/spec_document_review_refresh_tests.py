from __future__ import annotations

import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.spec_document_review_refresh import (
    DocumentReviewRefreshError,
    refresh_document_reviews,
)


class SpecDocumentReviewRefreshTests(unittest.TestCase):
    def test_dry_run_refreshes_explicit_rows_without_writing_or_reordering(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            first = self._tracked_document(root, "docs/specs/first.md", "first\n")
            second = self._tracked_document(root, "docs/specs/second.md", "second\n")
            untouched = self._tracked_document(
                root, "docs/specs/untouched.md", "untouched\n"
            )
            registry = self._registry(
                root,
                self._row("docs/specs/first.md", "1" * 64, freshness="review_due")
                + self._row("docs/specs/untouched.md", "2" * 64)
                + self._row("docs/specs/second.md", "3" * 64, freshness=None),
            )
            before = registry.read_text()
            self._add(root, first, second, untouched, registry)

            result = refresh_document_reviews(
                root=root,
                paths=["docs/specs/second.md", "docs/specs/first.md"],
                reviewed_date="2026-07-26",
                apply=False,
            )

            self.assertTrue(result.changed)
            self.assertFalse(result.applied)
            self.assertEqual(before, registry.read_text())
            self.assertEqual(
                [
                    "docs/specs/second.md",
                    "docs/specs/first.md",
                ],
                [item.path for item in result.documents],
            )
            self.assertLess(
                result.rendered.index('path = "docs/specs/first.md"'),
                result.rendered.index('path = "docs/specs/untouched.md"'),
            )
            self.assertLess(
                result.rendered.index('path = "docs/specs/untouched.md"'),
                result.rendered.index('path = "docs/specs/second.md"'),
            )
            self.assertIn(
                f'content_sha256 = "{self._digest(first)}"',
                result.rendered,
            )
            self.assertIn(
                f'content_sha256 = "{self._digest(second)}"',
                result.rendered,
            )
            self.assertIn('content_sha256 = "' + "2" * 64 + '"', result.rendered)
            first_row = self._rendered_row(result.rendered, "docs/specs/first.md")
            second_row = self._rendered_row(result.rendered, "docs/specs/second.md")
            self.assertIn('freshness = "current"', first_row)
            self.assertNotIn("freshness =", second_row)
            self.assertIn('owners = ["review-team"]', first_row)
            self.assertIn('last_reviewed = "2026-07-26"', first_row)

    def test_apply_writes_only_the_registry_refresh_fields(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            document = self._tracked_document(root, "docs/specs/first.md", "changed\n")
            row = self._row(
                "docs/specs/first.md",
                "1" * 64,
                freshness="historical",
                last_reviewed="2026-07-20",
            )
            registry = self._registry(root, "# retained header\n" + row)
            before_document = document.read_bytes()
            self._add(root, document, registry)

            result = refresh_document_reviews(
                root=root,
                paths=["docs/specs/first.md"],
                reviewed_date="2026-07-26",
                apply=True,
            )

            self.assertTrue(result.applied)
            self.assertEqual(result.rendered, registry.read_text())
            self.assertEqual(before_document, document.read_bytes())
            rendered = registry.read_text()
            self.assertTrue(rendered.startswith("# retained header\n"))
            self.assertIn(f'content_sha256 = "{self._digest(document)}"', rendered)
            self.assertIn('freshness = "current"', rendered)
            self.assertIn('last_reviewed = "2026-07-26"', rendered)
            self.assertIn('authority_levels = ["normative_product"]', rendered)
            self.assertIn('owners = ["review-team"]', rendered)

    def test_rejects_unknown_and_duplicate_registry_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            document = self._tracked_document(root, "docs/specs/first.md", "first\n")
            registry = self._registry(root, self._row("docs/specs/other.md", "1" * 64))
            self._add(root, document, registry)

            with self.assertRaisesRegex(
                DocumentReviewRefreshError, "no document review row"
            ):
                refresh_document_reviews(
                    root=root,
                    paths=["docs/specs/first.md"],
                    reviewed_date="2026-07-26",
                )

            registry.write_text(
                self._row("docs/specs/first.md", "1" * 64)
                + self._row("docs/specs/first.md", "2" * 64)
            )
            with self.assertRaisesRegex(
                DocumentReviewRefreshError, "multiple document review rows"
            ):
                refresh_document_reviews(
                    root=root,
                    paths=["docs/specs/first.md"],
                    reviewed_date="2026-07-26",
                )

    def test_rejects_duplicate_requested_paths_and_invalid_date(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            document = self._tracked_document(root, "docs/specs/first.md", "first\n")
            registry = self._registry(
                root, self._row("docs/specs/first.md", "1" * 64)
            )
            self._add(root, document, registry)

            with self.assertRaisesRegex(
                DocumentReviewRefreshError, "duplicate requested path"
            ):
                refresh_document_reviews(
                    root=root,
                    paths=["docs/specs/first.md", "docs/specs/first.md"],
                    reviewed_date="2026-07-26",
                )
            with self.assertRaisesRegex(DocumentReviewRefreshError, "reviewed date"):
                refresh_document_reviews(
                    root=root,
                    paths=["docs/specs/first.md"],
                    reviewed_date="2026-02-30",
                )

    def test_rejects_absolute_escaping_and_symlink_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            document = self._tracked_document(root, "docs/specs/first.md", "first\n")
            registry = self._registry(
                root, self._row("docs/specs/first.md", "1" * 64)
            )
            self._add(root, document, registry)
            link = root / "docs/specs/link.md"
            link.symlink_to("first.md")
            self._add(root, link)

            for unsafe in (str(document), "../outside.md", "docs/../docs/specs/first.md"):
                with self.subTest(path=unsafe):
                    with self.assertRaisesRegex(
                        DocumentReviewRefreshError, "safe repo-relative path"
                    ):
                        refresh_document_reviews(
                            root=root,
                            paths=[unsafe],
                            reviewed_date="2026-07-26",
                        )
            with self.assertRaisesRegex(DocumentReviewRefreshError, "symlink"):
                refresh_document_reviews(
                    root=root,
                    paths=["docs/specs/link.md"],
                    reviewed_date="2026-07-26",
                )

    def test_rejects_untracked_document(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            document = self._tracked_document(root, "docs/specs/first.md", "first\n")
            registry = self._registry(
                root, self._row("docs/specs/first.md", "1" * 64)
            )
            self._add(root, registry)

            with self.assertRaisesRegex(DocumentReviewRefreshError, "not tracked"):
                refresh_document_reviews(
                    root=root,
                    paths=[document.relative_to(root).as_posix()],
                    reviewed_date="2026-07-26",
                )

    def test_rejects_invalid_existing_freshness_without_writing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._repository(Path(directory))
            document = self._tracked_document(root, "docs/specs/first.md", "first\n")
            registry = self._registry(
                root,
                self._row("docs/specs/first.md", "1" * 64, freshness="unknown"),
            )
            before = registry.read_text()
            self._add(root, document, registry)

            with self.assertRaisesRegex(DocumentReviewRefreshError, "invalid freshness"):
                refresh_document_reviews(
                    root=root,
                    paths=["docs/specs/first.md"],
                    reviewed_date="2026-07-26",
                    apply=True,
                )
            self.assertEqual(before, registry.read_text())

    @staticmethod
    def _repository(root: Path) -> Path:
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        return root

    @staticmethod
    def _tracked_document(root: Path, relative: str, content: str) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        return path

    @staticmethod
    def _registry(root: Path, content: str) -> Path:
        path = root / "verification/spec-document-reviews.toml"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        return path

    @staticmethod
    def _add(root: Path, *paths: Path) -> None:
        relative = [path.relative_to(root).as_posix() for path in paths]
        subprocess.run(["git", "-C", str(root), "add", "--", *relative], check=True)

    @staticmethod
    def _digest(path: Path) -> str:
        return hashlib.sha256(path.read_bytes()).hexdigest()

    @staticmethod
    def _row(
        path: str,
        digest: str,
        *,
        freshness: str | None = "current",
        last_reviewed: str = "2026-07-20",
    ) -> str:
        freshness_line = (
            f'freshness = "{freshness}"\n' if freshness is not None else ""
        )
        return (
            "[[document_reviews]]\n"
            "schema_version = 1\n"
            f'document_id = "DOC_{path}"\n'
            f'path = "{path}"\n'
            f'content_sha256 = "{digest}"\n'
            'areas = ["compiler_iec"]\n'
            'authority_levels = ["normative_product"]\n'
            'owners = ["review-team"]\n'
            f"{freshness_line}"
            'visibility = "public"\n'
            "oracle_usable = true\n"
            'classification_basis = "explicit_review"\n'
            'conflict_disposition = "registered_conflicts_reviewed"\n'
            'checklist_staleness = "not_applicable"\n'
            'removed_behavior_disposition = "reviewed_current"\n'
            f'last_reviewed = "{last_reviewed}"\n\n'
        )

    @staticmethod
    def _rendered_row(rendered: str, path: str) -> str:
        for row in rendered.split("[[document_reviews]]")[1:]:
            if f'path = "{path}"' in row:
                return row
        raise AssertionError(f"missing rendered row for {path}")


if __name__ == "__main__":
    unittest.main()
