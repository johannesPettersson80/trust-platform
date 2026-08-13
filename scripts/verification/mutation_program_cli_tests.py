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
    def test_stale_generated_fields_are_refreshed_atomically_and_idempotently(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/trust-runtime/src/runtime/restart.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn retain_on_warm() -> bool { true }\n")
            manifest = root / "verification/mutation-program.toml"
            manifest.parent.mkdir()
            manifest.write_text(_refresh_fixture())
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
            mutation = tomllib.loads(manifest.read_text())["shards"][0]["mutations"][0]
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

    def test_unsafe_source_path_is_rejected_without_mutating_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "verification/mutation-program.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(_refresh_fixture(source_file="../escape.rs"))
            original = manifest.read_bytes()

            with self.assertRaisesRegex(MutationContractError, "unsafe"):
                refresh_mutation_shard(
                    root,
                    "MUTATION_SHARD_RETAIN_RESTART_001",
                    discover=lambda *_args: self.fail(
                        "unsafe source paths must fail before discovery"
                    ),
                )

            self.assertEqual(original, manifest.read_bytes())
            self.assertEqual([], list(manifest.parent.glob("*.tmp")))

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
            instruction_source = (
                root / "crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs"
            )
            instruction_source.parent.mkdir(parents=True)
            instruction_source.write_text("fn validate_instruction_stream() {}\n")
            stack_source = instruction_source.with_name("stack_shape.rs")
            stack_source.write_text("fn validate_stack_shape() {}\n")
            manifest = root / "verification/mutation-program.toml"
            manifest.parent.mkdir()
            manifest.write_text(_legacy_refresh_fixture())
            before = tomllib.loads(manifest.read_text())
            candidates = {
                instruction_source.name: {
                    "name": "current instruction-stream selector",
                    "function": {"function_name": "validate_instruction_stream"},
                    "genre": "FnValue",
                    "replacement": "Ok(())",
                },
                stack_source.name: {
                    "name": "current stack-shape selector",
                    "function": {"function_name": "validate_stack_shape"},
                    "genre": "FnValue",
                    "replacement": "Ok(())",
                },
            }

            with mock.patch(
                "scripts.verification.mutation_program_refresh.discover_mutants",
                side_effect=lambda source, *_args: [candidates[source.name]],
            ):
                exit_code = refresh_main(
                    [
                        "--root",
                        str(root),
                        "--shard-id",
                        "MUTATION_SHARD_BYTECODE_VALIDATOR_001",
                    ]
                )

            self.assertEqual(0, exit_code)
            after = tomllib.loads(manifest.read_text())
            before_mutations = before["shards"][0]["mutations"]
            after_mutations = after["shards"][0]["mutations"]
            for source, before_mutation, after_mutation in zip(
                (instruction_source, stack_source),
                before_mutations,
                after_mutations,
                strict=True,
            ):
                self.assertEqual(
                    {
                        key: value
                        for key, value in before_mutation.items()
                        if key not in {"source_digest", "selector_name"}
                    },
                    {
                        key: value
                        for key, value in after_mutation.items()
                        if key not in {"source_digest", "selector_name"}
                    },
                )
                self.assertEqual(
                    "sha256:" + hashlib.sha256(source.read_bytes()).hexdigest(),
                    after_mutation["source_digest"],
                )
                self.assertEqual(
                    candidates[source.name]["name"],
                    after_mutation["selector_name"],
                )

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


def _refresh_fixture(
    *,
    two_mutations: bool = False,
    source_file: str = "crates/trust-runtime/src/runtime/restart.rs",
) -> str:
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
        source_file = "{source_file}"
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


def _legacy_refresh_fixture() -> str:
    return textwrap.dedent(
        """
        schema_version = 1

        [[shards]]
        id = "MUTATION_SHARD_BYTECODE_VALIDATOR_001"
        title = "Bytecode validator"

        [[shards.mutations]]
        id = "MUTANT_VALIDATE_INSTRUCTION_STREAM_BYPASS"
        source_file = "crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs"
        source_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        function = "validate_instruction_stream"
        genre = "FnValue"
        replacement = "Ok(())"
        selector_name = "stale instruction-stream selector"
        build_command = ["cargo", "test", "--no-run"]
        test_command = ["cargo", "test"]
        association_ids = ["CASE_INSTRUCTION"]

        [[shards.mutations]]
        id = "MUTANT_VALIDATE_STACK_SHAPE_BYPASS"
        source_file = "crates/trust-runtime/src/bytecode/validate/stack_shape.rs"
        source_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        function = "validate_stack_shape"
        genre = "FnValue"
        replacement = "Ok(())"
        selector_name = "stale stack-shape selector"
        build_command = ["cargo", "test", "--no-run"]
        test_command = ["cargo", "test"]
        association_ids = ["CASE_STACK"]
        """
    )


if __name__ == "__main__":
    unittest.main()
