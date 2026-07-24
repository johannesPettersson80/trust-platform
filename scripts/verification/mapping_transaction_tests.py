"""Focused crash-safety tests for catalog mapping batch replacement."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.mapping_transaction import (
    apply_mapping_transaction,
    recover_mapping_transaction,
)


class MappingTransactionTests(unittest.TestCase):
    def test_success_replaces_complete_batch_and_removes_journal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "catalog.toml").write_bytes(b"old catalog")
            (root / "denominator.toml").write_bytes(b"old denominator")

            apply_mapping_transaction(
                root=root,
                replacements={
                    "catalog.toml": b"new catalog",
                    "denominator.toml": b"new denominator",
                },
                validate=lambda: [],
            )

            self.assertEqual(b"new catalog", (root / "catalog.toml").read_bytes())
            self.assertEqual(
                b"new denominator", (root / "denominator.toml").read_bytes()
            )
            self.assertFalse((root / ".mapping-transaction.json").exists())
            self.assertTrue((root / ".mapping-transaction.lock").is_file())

            apply_mapping_transaction(
                root=root,
                replacements={"catalog.toml": b"newer catalog"},
                validate=lambda: [],
            )

            self.assertEqual(b"newer catalog", (root / "catalog.toml").read_bytes())
            self.assertFalse((root / ".mapping-transaction.json").exists())

    def test_failed_post_replace_validation_rolls_back_every_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "catalog.toml").write_bytes(b"old catalog")
            (root / "denominator.toml").write_bytes(b"old denominator")

            with self.assertRaisesRegex(ValueError, "exact join failed"):
                apply_mapping_transaction(
                    root=root,
                    replacements={
                        "catalog.toml": b"new catalog",
                        "denominator.toml": b"new denominator",
                    },
                    validate=lambda: ["exact join failed"],
                )

            self.assertEqual(b"old catalog", (root / "catalog.toml").read_bytes())
            self.assertEqual(
                b"old denominator", (root / "denominator.toml").read_bytes()
            )
            self.assertFalse((root / ".mapping-transaction.json").exists())
            self.assertTrue((root / ".mapping-transaction.lock").is_file())

            self.assertFalse(recover_mapping_transaction(root))
            self.assertFalse((root / ".mapping-transaction.json").exists())

    def test_recovery_restores_originals_and_removes_new_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "catalog.toml").write_bytes(b"partially replaced")
            journal = {
                "schema_version": 1,
                "state": "replacing",
                "entries": [
                    {
                        "path": "catalog.toml",
                        "original_exists": True,
                        "original_base64": "b2xkIGNhdGFsb2c=",
                    },
                    {
                        "path": "new-decisions.toml",
                        "original_exists": False,
                        "original_base64": "",
                    },
                ],
            }
            (root / "new-decisions.toml").write_bytes(b"partial")
            (root / ".mapping-transaction.json").write_text(json.dumps(journal))

            self.assertTrue(recover_mapping_transaction(root))

            self.assertEqual(b"old catalog", (root / "catalog.toml").read_bytes())
            self.assertFalse((root / "new-decisions.toml").exists())
            self.assertFalse((root / ".mapping-transaction.json").exists())

    def test_paths_cannot_escape_transaction_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "repository-relative"):
                apply_mapping_transaction(
                    root=root,
                    replacements={"../outside": b"bad"},
                    validate=lambda: [],
                )


if __name__ == "__main__":
    unittest.main()
