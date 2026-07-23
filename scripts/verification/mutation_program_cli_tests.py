"""Executable entrypoint tests for the Phase 10 mutation report."""

from __future__ import annotations

import hashlib
import os
import subprocess
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path
from unittest import mock

from .metadata_validator.constants import ROOT
from .metadata_validator.mutation_contracts import MutationContractError
from .mutation_program_refresh import main as refresh_main
from .mutation_program_refresh import refresh_mutation_shard


class MutationProgramCliTests(unittest.TestCase):
    def test_documented_entrypoints_load_from_repository_root(self) -> None:
        for script in (
            "scripts/refresh_focused_mutation_shard.py",
            "scripts/report_mutation_program.py",
            "scripts/validate_mutation_program_report.py",
        ):
            result = subprocess.run(
                ["python3", script, "--help"],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stderr)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/trust-runtime/src/runtime/restart.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn retain_on_warm() -> bool { true }\n")
            second_source = source.with_name("restart_second.rs")
            second_source.write_text("fn retain_on_warm() -> bool { true }\n")
            manifest = root / "verification/mutation-program.toml"
            manifest.parent.mkdir()
            manifest.write_text(_refresh_fixture())
            original = manifest.read_bytes()
            candidate = {
                "name": "restart.rs:1:31: replace retain_on_warm -> bool with false",
                "function": {"function_name": "retain_on_warm"},
                "genre": "FnValue",
                "replacement": "false",
            }

            with mock.patch(
                "scripts.verification.mutation_program_refresh.os.fsync"
            ) as fsync:
                result = refresh_mutation_shard(
                    root,
                    "MUTATION_SHARD_RETAIN_RESTART_001",
                    discover=lambda *_args: [candidate],
                )
            self.assertEqual(1 if os.name == "nt" else 2, fsync.call_count)
            self.assertTrue(result.changed)
            refreshed = tomllib.loads(manifest.read_text())
            mutation = refreshed["shards"][0]["mutations"][0]
            self.assertEqual(
                "sha256:" + hashlib.sha256(source.read_bytes()).hexdigest(),
                mutation["source_digest"],
            )
            self.assertEqual(candidate["name"], mutation["selector_name"])
            self.assertIn('title = "Retain and restart"', manifest.read_text())
            self.assertFalse(
                refresh_mutation_shard(
                    root,
                    "MUTATION_SHARD_RETAIN_RESTART_001",
                    discover=lambda *_args: [candidate],
                ).changed
            )

            manifest.write_bytes(original)
            ambiguous = [candidate, {**candidate, "name": candidate["name"] + " duplicate"}]
            with self.assertRaisesRegex(MutationContractError, "found 2"):
                refresh_mutation_shard(
                    root,
                    "MUTATION_SHARD_RETAIN_RESTART_001",
                    discover=lambda *_args: ambiguous,
                )
            self.assertEqual(original, manifest.read_bytes())
            self.assertEqual([], list(manifest.parent.glob("*.tmp")))

            manifest.write_text(_refresh_fixture(two_mutations=True))
            original = manifest.read_bytes()
            calls = 0

            def fail_second_discovery(*_args: object) -> list[dict[str, object]]:
                nonlocal calls
                calls += 1
                if calls == 1:
                    return [candidate]
                raise MutationContractError("second discovery failed")

            with self.assertRaisesRegex(MutationContractError, "second discovery failed"):
                refresh_mutation_shard(
                    root,
                    "MUTATION_SHARD_RETAIN_RESTART_001",
                    discover=fail_second_discovery,
                )
            self.assertEqual(original, manifest.read_bytes())

            program = {"shards": [{"id": "MUTATION_SHARD_RETAIN_RESTART_001"}]}
            shard = program["shards"][0]
            payload = {
                "summary": {
                    "caught": 1,
                    "survived": 0,
                    "unviable": 0,
                    "timeout": 0,
                }
            }
            artifact_path = root / "artifact.json"
            with (
                mock.patch(
                    "scripts.verification.mutation_program_refresh.load_mutation_program",
                    return_value=program,
                ),
                mock.patch(
                    "scripts.verification.mutation_program_refresh."
                    "validate_mutation_program_for_shard_execution",
                    return_value=[],
                ) as validate,
                mock.patch(
                    "scripts.verification.mutation_program_refresh.select_source_shard",
                    return_value=shard,
                ),
                mock.patch(
                    "scripts.verification.mutation_program_refresh.artifact_output_path",
                    return_value=artifact_path,
                ),
                mock.patch(
                    "scripts.verification.mutation_program_refresh.execute_source_shard",
                    return_value=payload,
                ) as execute,
                mock.patch(
                    "scripts.verification.mutation_program_refresh._write_execution_artifact"
                ) as write,
            ):
                exit_code = refresh_main(
                    [
                        "--root",
                        str(root),
                        "--shard-id",
                        "MUTATION_SHARD_RETAIN_RESTART_001",
                        "--execute-artifact",
                        "--json-out",
                        "artifact.json",
                        "--target-dir",
                        str(root / "target"),
                    ]
                )
            self.assertEqual(0, exit_code)
            validate.assert_called_once_with(
                root.resolve(),
                program,
                "MUTATION_SHARD_RETAIN_RESTART_001",
            )
            execute.assert_called_once()
            write.assert_called_once_with(artifact_path, payload)


def _refresh_fixture(*, two_mutations: bool = False) -> str:
    second = """
        [[shards.mutations]]
        id = "MUTANT_RETAIN_ON_WARM_FALSE_SECOND"
        source_file = "crates/trust-runtime/src/runtime/restart_second.rs"
        source_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        function = "retain_on_warm"
        genre = "FnValue"
        replacement = "false"
        selector_name = "stale second selector"
        build_command = ["cargo", "test", "-p", "trust-runtime", "--no-run"]
        test_command = ["cargo", "test", "-p", "trust-runtime", "retain"]
        association_ids = ["DISC_SECOND"]
    """ if two_mutations else ""
    return textwrap.dedent(
        f"""
        schema_version = 1

        [[shards]]
        id = "MUTATION_SHARD_RETAIN_RESTART_001"
        title = "Retain and restart"

        [[shards.mutations]]
        id = "MUTANT_RETAIN_ON_WARM_FALSE"
        source_file = "crates/trust-runtime/src/runtime/restart.rs"
        source_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        function = "retain_on_warm"
        genre = "FnValue"
        replacement = "false"
        selector_name = "stale selector"
        build_command = ["cargo", "test", "-p", "trust-runtime", "--no-run"]
        test_command = ["cargo", "test", "-p", "trust-runtime", "retain"]
        association_ids = ["DISC_FIRST"]
        {second}
        """
    )


if __name__ == "__main__":
    unittest.main()
