from __future__ import annotations

import json
import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.verification.frozen_hir_family_batch import (
    build_hir_family_command,
    derive_hir_family_selection,
    hir_family_selection_digest,
    hir_remaining_paths,
    project_raw_results,
    run_hir_family_batch,
    validate_projection_execution,
)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content.strip() + "\n")


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


class FrozenHirFamilyBatchTests(unittest.TestCase):
    def test_direct_runner_cli_is_importable(self) -> None:
        root = Path(__file__).resolve().parents[2]
        completed = subprocess.run(
            [
                sys.executable,
                "scripts/run_frozen_hir_family_batch.py",
                "--help",
            ],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertIn("--raw-results", completed.stdout)

    def test_closed_manifest_has_all_remaining_hir_source_paths(self) -> None:
        paths = hir_remaining_paths()

        self.assertEqual(38, len(paths))
        self.assertEqual(
            21,
            sum(path.startswith("crates/trust-hir/tests/") for path in paths),
        )
        self.assertEqual(
            17,
            sum(path.startswith("crates/trust-hir/src/") for path in paths),
        )

    def test_selection_and_command_include_mapped_controls_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            integration_path = "crates/trust-hir/tests/generic_types.rs"
            unit_path = "crates/trust-hir/src/diagnostics.rs"
            _write(root / integration_path, "#[test]\nfn generic_case() {}")
            _write(
                root / unit_path,
                """
                #[cfg(test)]
                mod tests {
                    #[test]
                    fn unit_new() {}
                    #[test]
                    fn unit_control() {}
                }
                """,
            )
            _write(
                root / "verification/test-catalog-denominator.toml",
                f"""
                [[reviews]]
                discovery_id = "DISC_GENERIC"
                discovery_source_kind = "rust_integration_test"
                path = "{integration_path}"
                name = "generic_case"
                ignore_state = "not_ignored"
                disposition = "reviewed_nonmapping"

                [[reviews]]
                discovery_id = "DISC_UNIT_NEW"
                discovery_source_kind = "rust_unit_test"
                path = "{unit_path}"
                name = "unit_new"
                ignore_state = "not_ignored"
                disposition = "reviewed_nonmapping"

                [[reviews]]
                discovery_id = "DISC_UNIT_CONTROL"
                discovery_source_kind = "rust_unit_test"
                path = "{unit_path}"
                name = "unit_control"
                ignore_state = "not_ignored"
                disposition = "catalog_mapped"
                """,
            )
            _write(
                root / "verification/test-catalog.toml",
                f"""
                [[tests]]
                id = "TEST_CONTROL"
                discovery_id = "DISC_UNIT_CONTROL"
                discovery_source_kind = "rust_unit_test"
                path = "{unit_path}"
                name = "unit_control"
                status = "mapped"
                """,
            )

            selection = derive_hir_family_selection(root)
            command = build_hir_family_command(selection)

            self.assertEqual(2, selection.reviewed_nonmapping_count)
            self.assertEqual(1, selection.catalog_mapped_count)
            self.assertEqual(3, len(selection.tests))
            unit_names = {
                test.name: test.encoded_name
                for test in selection.tests
                if test.path == unit_path
            }
            self.assertEqual(
                "trust-hir::trust_hir$diagnostics::tests::unit_control",
                unit_names["unit_control"],
            )
            self.assertIn("--lib", command)
            self.assertIn("--test", command)
            self.assertIn("generic_types", command)
            expression = command[-1]
            self.assertIn(
                "binary_id(=trust-hir::generic_types) & test(=generic_case)",
                expression,
            )
            self.assertIn(
                "binary_id(=trust-hir) & "
                "test(=diagnostics::tests::unit_control)",
                expression,
            )

    def test_projection_is_exact_and_rejects_false_success(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            integration_path = "crates/trust-hir/tests/generic_types.rs"
            _write(root / integration_path, "#[test]\nfn generic_case() {}")
            _write(
                root / "verification/test-catalog-denominator.toml",
                f"""
                [[reviews]]
                discovery_id = "DISC_GENERIC"
                discovery_source_kind = "rust_integration_test"
                path = "{integration_path}"
                name = "generic_case"
                ignore_state = "not_ignored"
                disposition = "reviewed_nonmapping"
                """,
            )
            _write(root / "verification/test-catalog.toml", "tests = []")
            selection = derive_hir_family_selection(root)
            encoded = selection.tests[0].encoded_name
            started = json.dumps(
                {"type": "test", "event": "started", "name": encoded}
            )
            passed = json.dumps(
                {
                    "type": "test",
                    "event": "ok",
                    "name": encoded,
                    "exec_time": 0.012,
                }
            )

            projected = project_raw_results(
                f"{started}\n{passed}\n",
                selection,
            )

            self.assertEqual({integration_path}, set(projected))
            self.assertIn('"event": "ok"', projected[integration_path])
            with self.assertRaisesRegex(ValueError, "terminal identities"):
                project_raw_results(f"{started}\n", selection)
            failed = json.dumps(
                {
                    "type": "test",
                    "event": "failed",
                    "name": encoded,
                    "exec_time": 0.012,
                }
            )
            with self.assertRaisesRegex(ValueError, "did not pass"):
                project_raw_results(
                    f"{started}\n{failed}\n",
                    selection,
                )
            with self.assertRaisesRegex(ValueError, "unexpected HIR test"):
                project_raw_results(
                    f'{started}\n'
                    '{"type":"test","event":"ok","name":"unexpected"}\n',
                    selection,
                )

    def test_execution_refuses_a_dirty_checkout_before_running_cargo(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _write(root / "README.md", "fixture")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(
                ["git", "config", "user.email", "test@example.com"],
                cwd=root,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Test"],
                cwd=root,
                check=True,
            )
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(
                ["git", "commit", "-qm", "fixture"],
                cwd=root,
                check=True,
            )
            _write(
                root / "crates/trust-hir/src/lib.rs",
                "pub fn dirty_before_execution() {}",
            )

            with self.assertRaisesRegex(ValueError, "clean frozen commit"):
                run_hir_family_batch(
                    root=root,
                    raw_path=root / "target/raw.jsonl",
                    metadata_path=root / "target/execution.json",
                    results_dir=root / "target/results",
                )

    def test_projection_provenance_binds_combined_raw_and_commit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = "crates/trust-hir/tests/generic_types.rs"
            _write(root / source, "#[test]\nfn generic_case() {}")
            _write(
                root / "verification/test-catalog-denominator.toml",
                f"""
                [[reviews]]
                discovery_id = "DISC_GENERIC"
                discovery_source_kind = "rust_integration_test"
                path = "{source}"
                name = "generic_case"
                ignore_state = "not_ignored"
                disposition = "reviewed_nonmapping"
                """,
            )
            _write(root / "verification/test-catalog.toml", "tests = []")
            runner = root / "runner.py"
            _write(runner, "pass")
            selection = derive_hir_family_selection(root)
            encoded = selection.tests[0].encoded_name
            raw = (
                json.dumps(
                    {"type": "test", "event": "started", "name": encoded}
                )
                + "\n"
                + json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": encoded,
                        "exec_time": 0.012,
                    }
                )
                + "\n"
            )
            combined = root / "combined.jsonl"
            projected = root / "projected.jsonl"
            _write(combined, raw)
            _write(projected, raw)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(
                ["git", "config", "user.email", "test@example.com"],
                cwd=root,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Test"],
                cwd=root,
                check=True,
            )
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(
                ["git", "commit", "-qm", "fixture"],
                cwd=root,
                check=True,
            )
            head = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=root,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            command = "cargo nextest run -p trust-hir"
            metadata = root / "execution.json"
            _write(
                metadata,
                json.dumps(
                    {
                        "source_commit": head,
                        "command": command,
                        "reviewed_nonmapping_count": 1,
                        "catalog_mapped_count": 0,
                        "selected_test_count": 1,
                        "selection_sha256": hir_family_selection_digest(
                            selection
                        ),
                        "denominator_sha256": _digest(
                            root
                            / "verification/test-catalog-denominator.toml"
                        ),
                        "catalog_sha256": _digest(
                            root / "verification/test-catalog.toml"
                        ),
                        "module_path": "runner.py",
                        "module_sha256": _digest(runner),
                        "shim_path": "runner.py",
                        "shim_sha256": _digest(runner),
                        "raw_results_path": "combined.jsonl",
                        "raw_results_sha256": _digest(combined),
                        "paths": [
                            {
                                "path": source,
                                "source_sha256": _digest(root / source),
                                "projected_raw_results_path": "projected.jsonl",
                                "projected_raw_results_sha256": _digest(projected),
                                "binary_id": "trust-hir::generic_types",
                                "test_prefix": "",
                                "target_kind": "test",
                            }
                        ],
                    }
                ),
            )
            execution = {
                "kind": "rust_nextest_jsonl_projection",
                "batch_command": command,
                "runner_path": "runner.py",
                "runner_sha256": _digest(runner),
                "source_path": source,
                "source_sha256": _digest(root / source),
                "raw_results_path": "projected.jsonl",
                "raw_results_sha256": _digest(projected),
                "combined_raw_results_path": "combined.jsonl",
                "combined_raw_results_sha256": _digest(combined),
                "execution_metadata_path": "execution.json",
                "execution_metadata_sha256": _digest(metadata),
                "nextest_version": "cargo-nextest fixture",
                "package": "trust-hir",
                "target_kind": "test",
                "binary_id": "trust-hir::generic_types",
                "test_prefix": "",
            }
            results = {
                "execution": execution,
                "tests": [
                    {
                        "title": "generic_case",
                        "duration": 12,
                        "state": "passed",
                    }
                ],
            }
            plan = {
                "path": source,
                "command": command,
                "runner_path": "runner.py",
            }

            validate_projection_execution(
                root=root,
                plan=plan,
                execution=execution,
                results_document=results,
            )
            dirty_source = root / "crates/trust-hir/src/lib.rs"
            _write(dirty_source, "pub fn changed_after_execution() {}")
            with self.assertRaisesRegex(ValueError, "dirty build"):
                validate_projection_execution(
                    root=root,
                    plan=plan,
                    execution=execution,
                    results_document=results,
                )
            dirty_source.unlink()
            combined.write_text(raw.replace('"ok"', '"failed"'))
            with self.assertRaisesRegex(ValueError, "combined_raw_results_sha256"):
                validate_projection_execution(
                    root=root,
                    plan=plan,
                    execution=execution,
                    results_document=results,
                )


if __name__ == "__main__":
    unittest.main()
