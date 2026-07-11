"""Tests for verification-report input provenance contracts."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.report_input_contract import (
    validate_bound_input_paths,
    validator_code_input_paths,
)


class ReportInputContractTests(unittest.TestCase):
    def test_regular_workspace_input_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "verification").mkdir()
            (root / "verification/input.toml").write_text("value = 1\n")

            self.assertEqual(
                validate_bound_input_paths(root, ["verification/input.toml"]),
                [],
            )

    def test_symlink_input_is_rejected_even_when_target_stays_in_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "verification").mkdir()
            (root / "unbound.toml").write_text("value = 1\n")
            (root / "verification/input.toml").symlink_to(root / "unbound.toml")

            failures = validate_bound_input_paths(
                root,
                ["verification/input.toml"],
            )

        self.assertTrue(any("symlink component" in item for item in failures), failures)

    def test_symlinked_parent_and_workspace_escape_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            outside = root.parent / f"{root.name}-outside"
            outside.mkdir()
            self.addCleanup(lambda: outside.rmdir())
            (outside / "input.toml").write_text("value = 1\n")
            self.addCleanup(lambda: (outside / "input.toml").unlink())
            (root / "verification").symlink_to(outside, target_is_directory=True)

            failures = validate_bound_input_paths(
                root,
                ["verification/input.toml"],
            )

        self.assertTrue(any("symlink component" in item for item in failures), failures)
        self.assertTrue(any("escapes the workspace" in item for item in failures), failures)

    def test_validator_code_closure_excludes_mutable_evidence_plane(self) -> None:
        paths = validator_code_input_paths(ROOT)

        for expected in (
            "scripts/validate_verification_metadata.py",
            "scripts/gen_cases.py",
            "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
            "scripts/verification/malformed_input_contract.py",
            "scripts/verification/metadata_validator/case_files.py",
            "verification/malformed-input-taxonomy.md",
            "verification/malformed-input-taxonomy.toml",
            "verification/schemas/malformed-input-taxonomy.schema.json",
            "verification/runtime-anomaly-taxonomy.toml",
            "verification/schemas/runtime-anomaly-taxonomy.schema.json",
        ):
            self.assertIn(expected, paths)
        self.assertNotIn("verification/evidence-index.toml", paths)
        self.assertFalse(any("__pycache__" in path or path.endswith(".pyc") for path in paths))


if __name__ == "__main__":
    unittest.main()
