"""Focused input-closure and clean-revision tests for Phase 17."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.code_spec_provenance import (
    CodeSpecProvenanceError,
    input_closure_digest,
    load_private_input_manifest,
    validate_clean_source_revision,
)
from scripts.verification.code_spec_common import sha256_digest


class CodeSpecProvenanceTests(unittest.TestCase):
    def test_private_input_manifest_loads_exact_path_digest_bindings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "verification/code-spec-private-inputs.toml"
            path.parent.mkdir(parents=True)
            path.write_text(
                'schema_version = 1\n[[inputs]]\npath = ".claude/hooks/review.py"\n'
                'digest = "sha256:' + "a" * 64 + '"\nowner = "verification"\n'
                'reason = "private bootstrap input"\nlast_reviewed = "2026-07-21"\n'
            )

            bindings = load_private_input_manifest(root)

        self.assertEqual(
            {".claude/hooks/review.py": "sha256:" + "a" * 64},
            bindings,
        )

    def test_digest_binds_sorted_path_identity_and_exact_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a.txt").write_bytes(b"same")
            (root / "b.txt").write_bytes(b"same")
            first = input_closure_digest(root, ["b.txt", "a.txt"])
            self.assertEqual(first, input_closure_digest(root, ["a.txt", "b.txt"]))
            (root / "b.txt").write_bytes(b"changed")
            self.assertNotEqual(first, input_closure_digest(root, ["a.txt", "b.txt"]))
            self.assertNotEqual(first, input_closure_digest(root, ["a.txt"]))

    def test_duplicate_escape_and_symlink_inputs_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a.txt").write_text("a")
            with self.assertRaisesRegex(CodeSpecProvenanceError, "duplicate"):
                input_closure_digest(root, ["a.txt", "a.txt"])
            with self.assertRaisesRegex(CodeSpecProvenanceError, "repository-relative"):
                input_closure_digest(root, ["../outside"])
            (root / "link.txt").symlink_to(root / "a.txt")
            with self.assertRaisesRegex(CodeSpecProvenanceError, "symlink"):
                input_closure_digest(root, ["link.txt"])

    def test_clean_revision_requires_full_head_and_tracked_unchanged_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            (root / "input.txt").write_text("one\n")
            subprocess.run(["git", "-C", str(root), "add", "input.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "fixture"], check=True)
            commit = subprocess.check_output(
                ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
            ).strip()
            self.assertEqual([], validate_clean_source_revision(root, commit, ["input.txt"]))
            self.assertTrue(validate_clean_source_revision(root, commit[:12], ["input.txt"]))
            (root / "input.txt").write_text("dirty\n")
            self.assertTrue(validate_clean_source_revision(root, commit, ["input.txt"]))

    def test_exact_digest_manifest_can_bind_one_intentionally_ignored_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            (root / ".gitignore").write_text("private.py\n")
            (root / "private.py").write_text("print('reviewed')\n")
            subprocess.run(["git", "-C", str(root), "add", ".gitignore"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "fixture"], check=True)
            commit = subprocess.check_output(
                ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
            ).strip()
            digest = sha256_digest((root / "private.py").read_bytes())

            self.assertEqual(
                [],
                validate_clean_source_revision(
                    root,
                    commit,
                    ["private.py"],
                    allowed_untracked={"private.py": digest},
                ),
            )
            self.assertTrue(
                validate_clean_source_revision(
                    root,
                    commit,
                    ["private.py"],
                    allowed_untracked={"private.py": "sha256:" + "0" * 64},
                )
            )


if __name__ == "__main__":
    unittest.main()
