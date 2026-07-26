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


if __name__ == "__main__":
    unittest.main()
