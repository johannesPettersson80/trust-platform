"""Focused crash-safety tests for catalog mapping batch replacement."""

from __future__ import annotations

import errno
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification import mapping_transaction
from scripts.verification.mapping_transaction import (
    apply_mapping_transaction,
    recover_mapping_transaction,
)


class MappingTransactionTests(unittest.TestCase):
    def test_emfile_parent_open_propagates_errno(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "nested").mkdir()
            root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
            original_open = os.open

            def exhausted_open(path, flags, mode=0o777, *, dir_fd=None):
                if path == "nested" and dir_fd is not None:
                    raise OSError(errno.EMFILE, "too many open files")
                return original_open(path, flags, mode, dir_fd=dir_fd)

            try:
                with patch.object(os, "open", side_effect=exhausted_open):
                    with self.assertRaises(OSError) as raised:
                        mapping_transaction._open_target(
                            root_fd,
                            "nested/file",
                            create_parent=False,
                        )
            finally:
                os.close(root_fd)

            self.assertEqual(errno.EMFILE, raised.exception.errno)

    def test_precondition_validation_uses_bounded_descriptors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = root / "inputs"
            inputs.mkdir()
            for index in range(80):
                (inputs / f"source-{index}.txt").write_bytes(
                    f"source-{index}".encode()
                )
            (root / "catalog.toml").write_bytes(b"old catalog")
            script = textwrap.dedent(
                """
                import resource
                import sys
                from pathlib import Path

                from scripts.verification.mapping_transaction import (
                    apply_mapping_transaction,
                )

                root = Path(sys.argv[1])
                _, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
                resource.setrlimit(resource.RLIMIT_NOFILE, (48, hard))
                expected = {
                    f"inputs/source-{index}.txt":
                        f"source-{index}".encode()
                    for index in range(80)
                }
                apply_mapping_transaction(
                    root=root,
                    replacements={"catalog.toml": b"new catalog"},
                    expected_originals=expected,
                    validate=lambda: [],
                )
                """
            )

            completed = subprocess.run(
                [sys.executable, "-c", script, str(root)],
                cwd=Path(__file__).resolve().parents[2],
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(0, completed.returncode, completed.stderr)
            self.assertEqual(b"new catalog", (root / "catalog.toml").read_bytes())
            for index in range(80):
                self.assertEqual(
                    f"source-{index}".encode(),
                    (inputs / f"source-{index}.txt").read_bytes(),
                )

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

    def test_locked_original_precondition_prevents_lost_update(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "catalog.toml"
            target.write_bytes(b"reviewed original")
            expected = target.read_bytes()
            target.write_bytes(b"concurrent update")

            with self.assertRaisesRegex(ValueError, "precondition failed"):
                apply_mapping_transaction(
                    root=root,
                    replacements={"catalog.toml": b"replacement"},
                    expected_originals={"catalog.toml": expected},
                    validate=lambda: [],
                )

            self.assertEqual(b"concurrent update", target.read_bytes())
            self.assertFalse((root / ".mapping-transaction.json").exists())

    def test_replacement_and_precondition_reject_symlink_ancestors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            real = root / "real"
            real.mkdir()
            target = real / "catalog.toml"
            target.write_bytes(b"original")
            (root / "alias").symlink_to(real, target_is_directory=True)

            for kwargs in (
                {
                    "replacements": {"alias/catalog.toml": b"replacement"},
                    "expected_originals": {},
                },
                {
                    "replacements": {"other.toml": b"replacement"},
                    "expected_originals": {
                        "alias/catalog.toml": b"original",
                    },
                },
            ):
                with self.subTest(kwargs=kwargs):
                    with self.assertRaisesRegex(ValueError, "symlink"):
                        apply_mapping_transaction(
                            root=root,
                            validate=lambda: [],
                            **kwargs,
                        )

            self.assertEqual(b"original", target.read_bytes())
            self.assertFalse((root / "other.toml").exists())

    def test_normalized_replacement_aliases_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "real").mkdir()

            with self.assertRaisesRegex(ValueError, "alias"):
                apply_mapping_transaction(
                    root=root,
                    replacements={
                        "real/catalog.toml": b"first",
                        "real\\catalog.toml": b"second",
                    },
                    validate=lambda: [],
                )

            self.assertFalse((root / "real/catalog.toml").exists())

    def test_parent_swap_race_never_writes_external_and_rolls_back(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            external = Path(temporary) / "external"
            safe = root / "safe"
            parked = root / "parked"
            safe.mkdir(parents=True)
            external.mkdir()
            (safe / "catalog.toml").write_bytes(b"original")
            (external / "catalog.toml").write_bytes(b"external sentinel")
            original_write = mapping_transaction._atomic_write_target
            swapped = False

            def racing_write(target, content):
                nonlocal swapped
                if target.relative == "safe/catalog.toml" and not swapped:
                    swapped = True
                    os.rename(safe, parked)
                    safe.symlink_to(external, target_is_directory=True)
                original_write(target, content)

            with patch.object(
                mapping_transaction,
                "_atomic_write_target",
                side_effect=racing_write,
            ):
                with self.assertRaisesRegex(ValueError, "lexical path drift"):
                    apply_mapping_transaction(
                        root=root,
                        replacements={
                            "safe/catalog.toml": b"replacement"
                        },
                        expected_originals={
                            "safe/catalog.toml": b"original"
                        },
                        validate=lambda: (
                            []
                            if (safe / "catalog.toml").read_bytes()
                            == b"replacement"
                            else ["lexical path drift"]
                        ),
                    )

            self.assertEqual(
                b"external sentinel",
                (external / "catalog.toml").read_bytes(),
            )
            self.assertEqual(b"original", (parked / "catalog.toml").read_bytes())
            self.assertFalse((root / ".mapping-transaction.json").exists())

    def test_internal_parent_identity_recheck_rejects_validate_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            external = Path(temporary) / "external"
            safe = root / "safe"
            parked = root / "parked"
            safe.mkdir(parents=True)
            external.mkdir()
            (safe / "catalog.toml").write_bytes(b"original")
            (external / "catalog.toml").write_bytes(b"external sentinel")
            original_write = mapping_transaction._atomic_write_target
            swapped = False

            def racing_write(target, content):
                nonlocal swapped
                if target.relative == "safe/catalog.toml" and not swapped:
                    swapped = True
                    os.rename(safe, parked)
                    safe.symlink_to(external, target_is_directory=True)
                original_write(target, content)

            with patch.object(
                mapping_transaction,
                "_atomic_write_target",
                side_effect=racing_write,
            ):
                with self.assertRaisesRegex(
                    ValueError, "lexical parent drift"
                ):
                    apply_mapping_transaction(
                        root=root,
                        replacements={
                            "safe/catalog.toml": b"replacement"
                        },
                        expected_originals={
                            "safe/catalog.toml": b"original"
                        },
                        validate=lambda: [],
                    )

            self.assertEqual(
                b"external sentinel",
                (external / "catalog.toml").read_bytes(),
            )
            self.assertEqual(b"original", (parked / "catalog.toml").read_bytes())
            self.assertFalse((root / ".mapping-transaction.json").exists())

    def test_recovery_parent_swap_race_never_writes_external(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            external = Path(temporary) / "external"
            safe = root / "safe"
            parked = root / "parked"
            safe.mkdir(parents=True)
            external.mkdir()
            (safe / "catalog.toml").write_bytes(b"partial")
            (external / "catalog.toml").write_bytes(b"external sentinel")
            journal = {
                "schema_version": 1,
                "state": "replacing",
                "entries": [
                    {
                        "path": "safe/catalog.toml",
                        "original_exists": True,
                        "original_base64": "b3JpZ2luYWw=",
                    }
                ],
            }
            (root / ".mapping-transaction.json").write_text(
                json.dumps(journal)
            )
            original_write = mapping_transaction._atomic_write_target
            swapped = False

            def racing_write(target, content):
                nonlocal swapped
                if not swapped:
                    swapped = True
                    os.rename(safe, parked)
                    safe.symlink_to(external, target_is_directory=True)
                original_write(target, content)

            with patch.object(
                mapping_transaction,
                "_atomic_write_target",
                side_effect=racing_write,
            ):
                with self.assertRaisesRegex(
                    ValueError, "lexical parent drift"
                ):
                    recover_mapping_transaction(root)

            self.assertEqual(
                b"external sentinel",
                (external / "catalog.toml").read_bytes(),
            )
            self.assertEqual(b"original", (parked / "catalog.toml").read_bytes())
            self.assertTrue((root / ".mapping-transaction.json").exists())

            safe.unlink()
            parked.rename(safe)
            self.assertTrue(recover_mapping_transaction(root))
            self.assertFalse((root / ".mapping-transaction.json").exists())


if __name__ == "__main__":
    unittest.main()
