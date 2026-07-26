from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.spec_source_registration import (
    build_spec_source_record,
    register_spec_source,
)


class SpecSourceRegistrationTests(unittest.TestCase):
    def test_record_is_derived_without_caller_supplied_id_or_covers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spec = root / "docs/specs/25-vscode-product-contract.md"
            spec.parent.mkdir(parents=True)
            spec.write_text(
                "# VS Code Product Contract\n\n"
                "## Product shell and command exposure\n\n"
                "## Live Values\n"
            )

            record = build_spec_source_record(
                root=root,
                path=Path("docs/specs/25-vscode-product-contract.md"),
                area="editor_safety",
                owner="editors-vscode",
                reviewed_date="2026-07-26",
            )

            self.assertEqual("SPEC_VSCODE_PRODUCT_CONTRACT_001", record["id"])
            self.assertEqual("VS Code Product Contract", record["title"])
            self.assertEqual(
                ["product_shell_and_command_exposure", "live_values"],
                record["covers"],
            )
            self.assertEqual("normative_product", record["authority"])

    def test_registration_is_idempotent_and_rejects_id_collision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spec = root / "docs/specs/25-vscode-product-contract.md"
            spec.parent.mkdir(parents=True)
            spec.write_text("# VS Code Product Contract\n\n## Live Values\n")
            registry = root / "verification/spec-sources.toml"
            registry.parent.mkdir(parents=True)
            registry.write_text("# registry\n")
            reviews = root / "verification/spec-document-reviews.toml"
            reviews.write_text("# reviews\n")

            first = register_spec_source(
                root=root,
                registry_path=registry,
                review_registry_path=reviews,
                path=Path("docs/specs/25-vscode-product-contract.md"),
                area="editor_safety",
                owner="editors-vscode",
                reviewed_date="2026-07-26",
            )
            registry.write_text(first.rendered)
            reviews.write_text(first.document_reviews_rendered)
            second = register_spec_source(
                root=root,
                registry_path=registry,
                review_registry_path=reviews,
                path=Path("docs/specs/25-vscode-product-contract.md"),
                area="editor_safety",
                owner="editors-vscode",
                reviewed_date="2026-07-26",
            )

            self.assertTrue(first.changed)
            self.assertFalse(second.changed)
            self.assertEqual(first.rendered, second.rendered)
            self.assertEqual(
                first.document_reviews_rendered,
                second.document_reviews_rendered,
            )
            self.assertIn(
                'document_id = "SPEC_DOC_',
                first.document_reviews_rendered,
            )
            self.assertIn(
                'classification_basis = "registered_metadata"',
                first.document_reviews_rendered,
            )
            registry.write_text(
                first.rendered.replace(
                    'path = "docs/specs/25-vscode-product-contract.md"',
                    'path = "docs/specs/other.md"',
                )
            )
            with self.assertRaisesRegex(ValueError, "ID collision"):
                register_spec_source(
                    root=root,
                    registry_path=registry,
                    review_registry_path=reviews,
                    path=Path("docs/specs/25-vscode-product-contract.md"),
                    area="editor_safety",
                    owner="editors-vscode",
                    reviewed_date="2026-07-26",
                )

    def test_registration_refreshes_existing_document_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spec = root / "docs/specs/25-vscode-product-contract.md"
            spec.parent.mkdir(parents=True)
            spec.write_text("# VS Code Product Contract\n\n## Live Values\n")
            registry = root / "verification/spec-sources.toml"
            registry.parent.mkdir(parents=True)
            registry.write_text("# registry\n")
            reviews = root / "verification/spec-document-reviews.toml"
            reviews.write_text("# reviews\n")

            first = register_spec_source(
                root=root,
                registry_path=registry,
                review_registry_path=reviews,
                path=Path("docs/specs/25-vscode-product-contract.md"),
                area="editor_safety",
                owner="editors-vscode",
                reviewed_date="2026-07-25",
            )
            registry.write_text(first.rendered)
            reviews.write_text(first.document_reviews_rendered)
            registry.write_text(
                registry.read_text().replace(
                    "known_limitations = []",
                    'known_limitations = ["rendered proof remains external"]',
                )
            )
            reviews.write_text(
                reviews.read_text().replace(
                    'conflict_disposition = "registered_conflicts_reviewed"',
                    'conflict_disposition = "manual_conflict_review"',
                )
            )
            old_digest = first.document_reviews_rendered.split(
                'content_sha256 = "',
                1,
            )[1].split('"', 1)[0]

            spec.write_text(
                "# VS Code Product Contract\n\n"
                "## Live Values\n\n"
                "## Devices and Connections\n"
            )
            refreshed = register_spec_source(
                root=root,
                registry_path=registry,
                review_registry_path=reviews,
                path=Path("docs/specs/25-vscode-product-contract.md"),
                area="editor_safety",
                owner="editors-vscode",
                reviewed_date="2026-07-26",
            )

            self.assertTrue(refreshed.changed)
            self.assertEqual(
                refreshed.rendered.count("[[spec_sources]]"),
                1,
            )
            self.assertEqual(
                refreshed.document_reviews_rendered.count(
                    "[[document_reviews]]"
                ),
                1,
            )
            self.assertIn(
                'covers = ["live_values"]',
                refreshed.rendered,
            )
            self.assertIn(
                'last_reviewed = "2026-07-26"',
                refreshed.rendered,
            )
            self.assertIn(
                'known_limitations = ["rendered proof remains external"]',
                refreshed.rendered,
            )
            self.assertIn(
                'conflict_disposition = "manual_conflict_review"',
                refreshed.document_reviews_rendered,
            )
            self.assertNotIn(old_digest, refreshed.document_reviews_rendered)

            with self.assertRaisesRegex(ValueError, "reviewed metadata"):
                register_spec_source(
                    root=root,
                    registry_path=registry,
                    review_registry_path=reviews,
                    path=Path("docs/specs/25-vscode-product-contract.md"),
                    area="editor_safety",
                    owner="different-owner",
                    reviewed_date="2026-07-26",
                )

    def test_refresh_preserves_reviewed_covers_for_legacy_h3_document(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spec = root / "docs/specs/11-runtime-engine.md"
            spec.parent.mkdir(parents=True)
            spec.write_text("# Runtime Engine\n\n### Existing section\n")
            registry = root / "verification/spec-sources.toml"
            registry.parent.mkdir(parents=True)
            registry.write_text(
                "[[spec_sources]]\n"
                "schema_version = 2\n"
                'id = "SPEC_RUNTIME_ENGINE_001"\n'
                'title = "Reviewed runtime title"\n'
                'area = "runtime_safety"\n'
                'owner = "trust-runtime"\n'
                'status = "mapped"\n'
                'authority = "normative_product"\n'
                'source_status = "active"\n'
                "oracle_eligible = true\n"
                'visibility = "public"\n'
                'locator_kind = "tracked_file"\n'
                'path = "docs/specs/11-runtime-engine.md"\n'
                'version = "current"\n'
                'last_reviewed = "2026-07-25"\n'
                'covers = ["scan_cycle_architecture", "fault_channel"]\n'
                "known_limitations = []\n"
                "conflicts_with = []\n"
            )
            reviews = root / "verification/spec-document-reviews.toml"
            reviews.write_text("# reviews\n")

            refreshed = register_spec_source(
                root=root,
                registry_path=registry,
                review_registry_path=reviews,
                path=Path("docs/specs/11-runtime-engine.md"),
                area="runtime_safety",
                owner="trust-runtime",
                reviewed_date="2026-07-26",
            )

            self.assertIn(
                'covers = ["scan_cycle_architecture", "fault_channel"]',
                refreshed.rendered,
            )
            self.assertIn(
                'title = "Reviewed runtime title"',
                refreshed.rendered,
            )
            registry.write_text(refreshed.rendered)
            reviews.write_text(refreshed.document_reviews_rendered)
            spec.write_text(
                "# Runtime Engine\n\n"
                "## First modern section\n\n"
                "### Existing section\n"
            )
            mixed_refresh = register_spec_source(
                root=root,
                registry_path=registry,
                review_registry_path=reviews,
                path=Path("docs/specs/11-runtime-engine.md"),
                area="runtime_safety",
                owner="trust-runtime",
                reviewed_date="2026-07-27",
            )
            self.assertIn(
                'covers = ["scan_cycle_architecture", "fault_channel"]',
                mixed_refresh.rendered,
            )


if __name__ == "__main__":
    unittest.main()
