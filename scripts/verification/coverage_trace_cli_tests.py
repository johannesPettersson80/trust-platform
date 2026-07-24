"""Focused tests for Rust trace CLI command construction."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification import coverage_trace_resume
from scripts.verification.coverage_trace_cli import (
    MAX_TESTS_PER_COVERAGE_COMMAND,
    coverage_run_commands,
    nextest_filter_expression,
    requested_tests_document,
    reusable_completed_attempt_ids,
    rust_package_target_inventory,
    select_ground_truth_pilot_ids,
    targeted_nextest_list_shards,
    trace_environment,
)


class CoverageTraceCliTests(unittest.TestCase):
    def test_cargo_metadata_marks_binary_only_test_ownership(self) -> None:
        inventory = rust_package_target_inventory(
            {
                "packages": [
                    {
                        "name": "trust-dev",
                        "targets": [
                            {
                                "name": "trust-dev",
                                "kind": ["bin"],
                                "test": True,
                            },
                            {
                                "name": "cli_smoke",
                                "kind": ["test"],
                                "test": True,
                            },
                        ],
                    },
                    {
                        "name": "with-lib",
                        "targets": [
                            {
                                "name": "with_lib",
                                "kind": ["lib"],
                                "test": True,
                            },
                            {
                                "name": "tool",
                                "kind": ["bin"],
                                "test": True,
                            },
                        ],
                    },
                ]
            }
        )

        self.assertEqual(
            {
                "package_has_library": False,
                "package_binary_names": ["trust-dev"],
            },
            inventory["trust-dev"],
        )
        self.assertEqual(
            {
                "package_has_library": True,
                "package_binary_names": ["tool"],
            },
            inventory["with-lib"],
        )

    def test_nextest_listing_is_also_sharded_before_any_build_occurs(self) -> None:
        shards = targeted_nextest_list_shards(
            [
                {
                    "discovery_id": "DISC_LIB",
                    "discovery_source_kind": "rust_unit_test",
                    "package": "example",
                    "path": "crates/example/src/lib.rs",
                },
                {
                    "discovery_id": "DISC_INTEGRATION",
                    "discovery_source_kind": "rust_integration_test",
                    "package": "example",
                    "path": "crates/example/tests/api_smoke.rs",
                },
            ]
        )

        self.assertEqual(2, len(shards))
        commands = [shard["command"] for shard in shards]
        self.assertTrue(any("--lib" in command for command in commands))
        self.assertTrue(
            any(
                command[command.index("--test") : command.index("--test") + 2]
                == ["--test", "api_smoke"]
                for command in commands
                if "--test" in command
            )
        )
        self.assertTrue(all("--tests" not in command for command in commands))

    def test_coverage_commands_are_sharded_by_exact_cargo_target(self) -> None:
        commands = coverage_run_commands(
            resolved=[
                {
                    "discovery_id": "DISC_LIB",
                    "package": "example",
                    "cargo_target_kind": "lib",
                    "cargo_target_name": None,
                    "nextest_binary_id": "example",
                    "nextest_test_name": "module::unit_case",
                },
                {
                    "discovery_id": "DISC_INTEGRATION",
                    "package": "example",
                    "cargo_target_kind": "test",
                    "cargo_target_name": "api_smoke",
                    "nextest_binary_id": "api_smoke",
                    "nextest_test_name": "integration_case",
                },
            ],
            tool_config="/tmp/nextest.toml",
        )

        self.assertEqual(2, len(commands))
        self.assertIn("--lib", commands[0])
        self.assertNotIn("--tests", commands[0])
        self.assertEqual(
            ["--test", "api_smoke"],
            commands[1][commands[1].index("--test") : commands[1].index("--test") + 2],
        )
        self.assertNotIn("--all-targets", commands[1])

    def test_large_single_target_is_split_into_bounded_commands(self) -> None:
        records = [
            {
                "discovery_id": f"DISC_{index:020X}",
                "package": "example",
                "cargo_target_kind": "lib",
                "cargo_target_name": None,
                "nextest_binary_id": "example",
                "nextest_test_name": f"module::test_{index:04}",
            }
            for index in range(MAX_TESTS_PER_COVERAGE_COMMAND + 1)
        ]

        commands = coverage_run_commands(
            resolved=records,
            tool_config="/tmp/nextest.toml",
        )

        self.assertEqual(2, len(commands))
        self.assertTrue(all("--all-targets" not in command for command in commands))

    def test_filter_expression_is_exact_sorted_and_shell_free(self) -> None:
        resolved = [
            {
                "discovery_id": "DISC_TWO",
                "nextest_binary_id": "pkg-b",
                "nextest_test_name": "module::test_b",
            },
            {
                "discovery_id": "DISC_ONE",
                "nextest_binary_id": "pkg-a",
                "nextest_test_name": "module::test_a",
            },
        ]

        expression = nextest_filter_expression(resolved)

        self.assertEqual(
            "(binary_id(=pkg-a) & test(=module::test_a)) | "
            "(binary_id(=pkg-b) & test(=module::test_b))",
            expression,
        )
        self.assertNotIn("'", expression)
        self.assertNotIn(";", expression)

    def test_requested_document_contains_only_wrapper_identity(self) -> None:
        document = requested_tests_document(
            [
                {
                    "discovery_id": "DISC_ONE",
                    "nextest_binary_id": "pkg-a",
                    "nextest_test_name": "module::test_a",
                    "path": "ignored",
                }
            ]
        )

        self.assertEqual(
            {
                "schema_version": 1,
                "tests": [
                    {
                        "discovery_id": "DISC_ONE",
                        "nextest_binary_id": "pkg-a",
                        "nextest_test_name": "module::test_a",
                    }
                ],
            },
            document,
        )

    def test_filter_encodes_syntax_characters_and_rejects_duplicates(self) -> None:
        expression = nextest_filter_expression(
            [
                {
                    "discovery_id": "DISC_ONE",
                    "nextest_binary_id": "pkg",
                    "nextest_test_name": "test); all()",
                }
            ]
        )
        self.assertEqual(
            "(binary_id(=pkg) & "
            r"test(=test\u{29}\u{3B}\u{20}all\u{28}\u{29}))",
            expression,
        )
        self.assertNotIn("all()", expression)
        duplicate = {
            "discovery_id": "DISC_ONE",
            "nextest_binary_id": "pkg",
            "nextest_test_name": "module::test",
        }
        with self.assertRaisesRegex(ValueError, "duplicate"):
            nextest_filter_expression([duplicate, dict(duplicate)])

    def test_ground_truth_selection_is_deterministic_and_area_diverse(self) -> None:
        tests = {
            "TEST_Z": {
                "subject_kind": "generated_test",
                "discovery_id": "DISC_Z",
                "discovery_source_kind": "rust_unit_test",
                "area": "runtime_safety",
                "invariants": ["INV_Z"],
            },
            "TEST_A": {
                "subject_kind": "generated_test",
                "discovery_id": "DISC_A",
                "discovery_source_kind": "rust_unit_test",
                "area": "compiler_iec",
                "invariants": ["INV_A"],
            },
            "TEST_B": {
                "subject_kind": "generated_test",
                "discovery_id": "DISC_B",
                "discovery_source_kind": "rust_integration_test",
                "area": "runtime_safety",
                "invariants": ["INV_B"],
            },
        }

        selected = select_ground_truth_pilot_ids(
            tests=tests,
            live_discovery_ids={"DISC_A", "DISC_B", "DISC_Z"},
            count=2,
        )

        self.assertEqual(["DISC_A", "DISC_B"], selected)

    def test_trace_environment_routes_build_temporaries_off_system_tmp(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            temporary = root / "temporary"

            environment = trace_environment(
                base_environment={"PATH": "/bin"},
                cargo_target_dir=target,
                temporary_dir=temporary,
            )

            self.assertEqual(str(target.resolve()), environment["CARGO_TARGET_DIR"])
            self.assertEqual(str(temporary.resolve()), environment.get("TMPDIR"))
            self.assertTrue(temporary.is_dir())
            self.assertEqual("/bin", environment["PATH"])

    def test_resume_reuses_only_complete_exact_attempts_and_purges_partial(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "test-binary"
            executable.touch()
            complete = root / "attempts/complete"
            partial = root / "attempts/partial"
            (complete / "profiles").mkdir(parents=True)
            (partial / "profiles").mkdir(parents=True)
            (complete / "profiles/profile.profraw").write_bytes(b"profile")
            (complete / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_COMPLETE",
                        "binary_id": "example",
                        "test_name": "module::complete",
                        "test_executable": str(executable),
                        "test_executable_digest": (
                            "sha256:"
                            + hashlib.sha256(executable.read_bytes()).hexdigest()
                        ),
                        "status": "completed",
                        "exit_code": 0,
                    }
                )
            )
            (partial / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_PARTIAL",
                        "status": "started",
                    }
                )
            )

            reusable = reusable_completed_attempt_ids(
                trace_root=root,
                resolved=[
                    {
                        "discovery_id": "DISC_COMPLETE",
                        "nextest_binary_id": "example",
                        "nextest_test_name": "module::complete",
                    },
                    {
                        "discovery_id": "DISC_PARTIAL",
                        "nextest_binary_id": "example",
                        "nextest_test_name": "module::partial",
                    },
                ],
                purge_incomplete=True,
            )
            partial_exists = partial.exists()

        self.assertEqual({"DISC_COMPLETE"}, reusable)
        self.assertFalse(partial_exists)

    def test_resume_hashes_each_shared_test_executable_once(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "test-binary"
            executable.write_bytes(b"shared binary")
            executable_digest = (
                "sha256:" + hashlib.sha256(executable.read_bytes()).hexdigest()
            )
            resolved = []
            for suffix in ("ONE", "TWO"):
                attempt = root / f"attempts/{suffix.lower()}"
                (attempt / "profiles").mkdir(parents=True)
                (attempt / "profiles/profile.profraw").write_bytes(b"profile")
                (attempt / "attempt.json").write_text(
                    json.dumps(
                        {
                            "discovery_id": f"DISC_{suffix}",
                            "binary_id": "example",
                            "test_name": f"module::{suffix.lower()}",
                            "test_executable": str(executable),
                            "test_executable_digest": executable_digest,
                            "status": "completed",
                            "exit_code": 0,
                        }
                    )
                )
                resolved.append(
                    {
                        "discovery_id": f"DISC_{suffix}",
                        "nextest_binary_id": "example",
                        "nextest_test_name": f"module::{suffix.lower()}",
                    }
                )

            with mock.patch(
                "scripts.verification.coverage_trace_resume._file_digest",
                wraps=coverage_trace_resume._file_digest,
            ) as digest:
                reusable = reusable_completed_attempt_ids(
                    trace_root=root,
                    resolved=resolved,
                    purge_incomplete=True,
                )

        self.assertEqual({"DISC_ONE", "DISC_TWO"}, reusable)
        self.assertEqual(1, digest.call_count)


if __name__ == "__main__":
    unittest.main()
