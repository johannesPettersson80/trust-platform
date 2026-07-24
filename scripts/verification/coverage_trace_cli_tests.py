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
    instrumented_nextest_list_environment,
    legacy_requested_tests_document,
    nextest_filter_expression,
    requested_tests_document,
    reusable_completed_attempt_ids,
    rust_package_target_inventory,
    select_ground_truth_pilot_ids,
    targeted_nextest_list_shards,
    trace_environment,
    validate_resume_request,
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

        self.assertEqual(1, len(shards))
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
        self.assertEqual(2, len(shards[0]["targets"]))

    def test_nextest_listing_batches_at_most_sixteen_exact_targets_per_package(
        self,
    ) -> None:
        facts = [
            {
                "discovery_id": f"DISC_{index:020X}",
                "discovery_source_kind": "rust_integration_test",
                "package": "example",
                "path": f"crates/example/tests/target_{index:02}.rs",
            }
            for index in range(17)
        ]

        shards = targeted_nextest_list_shards(list(reversed(facts)))

        self.assertEqual(2, len(shards))
        self.assertEqual(
            [16, 1],
            [len(shard["targets"]) for shard in shards],
        )
        self.assertTrue(
            all(
                len(shard["targets"]) <= 16
                for shard in shards
            )
        )
        self.assertEqual(
            [16, 1],
            [
                sum(1 for argument in shard["command"] if argument == "--test")
                for shard in shards
            ],
        )
        self.assertEqual(
            [
                "target_00",
                "target_01",
                "target_02",
                "target_03",
                "target_04",
                "target_05",
                "target_06",
                "target_07",
                "target_08",
                "target_09",
                "target_10",
                "target_11",
                "target_12",
                "target_13",
                "target_14",
                "target_15",
            ],
            [
                target["cargo_target_name"]
                for target in shards[0]["targets"]
            ],
        )

    def test_nextest_listing_never_batches_targets_across_packages(self) -> None:
        shards = targeted_nextest_list_shards(
            [
                {
                    "discovery_id": "DISC_B",
                    "discovery_source_kind": "rust_integration_test",
                    "package": "package-b",
                    "path": "crates/package-b/tests/shared.rs",
                },
                {
                    "discovery_id": "DISC_A",
                    "discovery_source_kind": "rust_integration_test",
                    "package": "package-a",
                    "path": "crates/package-a/tests/shared.rs",
                },
            ]
        )

        self.assertEqual(["package-a", "package-b"], [
            shard["package"] for shard in shards
        ])
        for shard in shards:
            package = shard["package"]
            self.assertEqual(
                [package],
                sorted(
                    {
                        fact["package"]
                        for target in shard["targets"]
                        for fact in target["facts"]
                    }
                ),
            )

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

    def test_requested_document_contains_current_executable_identity(self) -> None:
        document = requested_tests_document(
            [
                {
                    "discovery_id": "DISC_ONE",
                    "nextest_binary_id": "pkg-a",
                    "nextest_binary_path": "/target/pkg-a",
                    "nextest_test_name": "module::test_a",
                    "path": "ignored",
                }
            ]
        )

        self.assertEqual(
            {
                "schema_version": 2,
                "tests": [
                    {
                        "discovery_id": "DISC_ONE",
                        "nextest_binary_id": "pkg-a",
                        "nextest_binary_path": "/target/pkg-a",
                        "nextest_test_name": "module::test_a",
                    }
                ],
            },
            document,
        )

    def test_resume_request_accepts_well_formed_denominator_reconciliation(
        self,
    ) -> None:
        resolved = [
            {
                "discovery_id": "DISC_ONE",
                "nextest_binary_id": "pkg-a",
                "nextest_binary_path": "/target/pkg-a",
                "nextest_test_name": "module::test_a",
            }
        ]
        current = requested_tests_document(resolved)
        legacy = legacy_requested_tests_document(resolved)

        validate_resume_request(
            existing=current,
            current=current,
            legacy=legacy,
        )
        validate_resume_request(
            existing=legacy,
            current=current,
            legacy=legacy,
        )
        changed_identity = {
            **legacy,
            "tests": [
                {
                    **legacy["tests"][0],
                    "nextest_test_name": "module::different",
                }
            ],
        }
        validate_resume_request(
            existing=changed_identity,
            current=current,
            legacy=legacy,
        )
        validate_resume_request(
            existing={"schema_version": 2, "tests": []},
            current=current,
            legacy=legacy,
        )
        validate_resume_request(
            existing={
                "schema_version": 2,
                "tests": [
                    *current["tests"],
                    {
                        "discovery_id": "DISC_TWO",
                        "nextest_binary_id": "pkg-b",
                        "nextest_binary_path": "/target/pkg-b",
                        "nextest_test_name": "module::test_b",
                    },
                ],
            },
            current=current,
            legacy=legacy,
        )

    def test_resume_request_rejects_malformed_or_ambiguous_history(self) -> None:
        resolved = [
            {
                "discovery_id": "DISC_ONE",
                "nextest_binary_id": "pkg-a",
                "nextest_binary_path": "/target/pkg-a",
                "nextest_test_name": "module::test_a",
            }
        ]
        current = requested_tests_document(resolved)
        legacy = legacy_requested_tests_document(resolved)
        malformed = {
            **current,
            "tests": [{**current["tests"][0], "unexpected": "field"}],
        }
        with self.assertRaisesRegex(ValueError, "fields drift"):
            validate_resume_request(
                existing=malformed,
                current=current,
                legacy=legacy,
            )
        duplicate = {
            **current,
            "tests": [current["tests"][0], current["tests"][0]],
        }
        with self.assertRaisesRegex(ValueError, "duplicates DISC_ONE"):
            validate_resume_request(
                existing=duplicate,
                current=current,
                legacy=legacy,
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
                base_environment={
                    "PATH": "/bin",
                    "CARGO_LLVM_COV_TARGET_DIR": "/stale/target",
                    "CARGO_LLVM_COV_BUILD_DIR": "/stale/build",
                },
                cargo_target_dir=target,
                temporary_dir=temporary,
            )

            self.assertEqual(str(target.resolve()), environment["CARGO_TARGET_DIR"])
            coverage_target = str((target / "llvm-cov-target").resolve())
            self.assertEqual(
                coverage_target,
                environment["CARGO_LLVM_COV_TARGET_DIR"],
            )
            self.assertEqual(
                coverage_target,
                environment["CARGO_LLVM_COV_BUILD_DIR"],
            )
            self.assertEqual(str(temporary.resolve()), environment.get("TMPDIR"))
            self.assertTrue(temporary.is_dir())
            self.assertEqual("/bin", environment["PATH"])

    def test_nextest_listing_uses_the_exact_instrumented_target_directory(
        self,
    ) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            run_environment = trace_environment(
                base_environment={
                    "PATH": "/bin",
                    "CARGO_LLVM_COV_TARGET_DIR": "/stale/target",
                    "CARGO_LLVM_COV_BUILD_DIR": "/stale/build",
                },
                cargo_target_dir=target,
                temporary_dir=root / "tmp",
            )
            environment = instrumented_nextest_list_environment(
                base_environment=run_environment,
                llvm_cov_environment={
                    "RUSTC_WRAPPER": "/bin/cargo-llvm-cov",
                    "CARGO_LLVM_COV": "1",
                    "LLVM_PROFILE_FILE": "/tmp/profile-%p.profraw",
                },
                cargo_target_dir=target,
            )

        expected = str((target / "llvm-cov-target").resolve())
        self.assertEqual(expected, environment["CARGO_TARGET_DIR"])
        self.assertEqual(expected, environment["CARGO_LLVM_COV_TARGET_DIR"])
        self.assertEqual(expected, environment["CARGO_LLVM_COV_BUILD_DIR"])
        self.assertEqual(
            run_environment["CARGO_LLVM_COV_TARGET_DIR"],
            environment["CARGO_TARGET_DIR"],
        )
        self.assertEqual(
            run_environment["CARGO_LLVM_COV_BUILD_DIR"],
            environment["CARGO_TARGET_DIR"],
        )
        self.assertEqual("/bin/cargo-llvm-cov", environment["RUSTC_WRAPPER"])
        self.assertEqual(str(root / "tmp"), environment["TMPDIR"])

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
                        "nextest_binary_path": str(executable),
                        "nextest_test_name": "module::complete",
                    },
                    {
                        "discovery_id": "DISC_PARTIAL",
                        "nextest_binary_id": "example",
                        "nextest_binary_path": str(executable),
                        "nextest_test_name": "module::partial",
                    },
                ],
                purge_incomplete=True,
            )
            partial_exists = partial.exists()

        self.assertEqual({"DISC_COMPLETE"}, reusable)
        self.assertFalse(partial_exists)

    def test_resume_reconciles_removed_test_without_losing_unchanged_attempt(
        self,
    ) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "test-binary"
            executable.write_bytes(b"unchanged binary")
            executable_digest = (
                "sha256:" + hashlib.sha256(executable.read_bytes()).hexdigest()
            )
            for discovery_id, test_name in (
                ("DISC_CURRENT", "module::current"),
                ("DISC_REMOVED", "module::removed"),
            ):
                attempt = root / f"attempts/{discovery_id.lower()}"
                (attempt / "profiles").mkdir(parents=True)
                (attempt / "profiles/profile.profraw").write_bytes(b"profile")
                (attempt / "attempt.json").write_text(
                    json.dumps(
                        {
                            "discovery_id": discovery_id,
                            "binary_id": "example",
                            "test_name": test_name,
                            "test_executable": str(executable),
                            "test_executable_digest": executable_digest,
                            "status": "completed",
                            "exit_code": 0,
                        }
                    )
                )

            reusable = reusable_completed_attempt_ids(
                trace_root=root,
                resolved=[
                    {
                        "discovery_id": "DISC_CURRENT",
                        "nextest_binary_id": "example",
                        "nextest_binary_path": str(executable),
                        "nextest_test_name": "module::current",
                    }
                ],
                purge_incomplete=True,
            )
            current_exists = (root / "attempts/disc_current").is_dir()
            removed_exists = (root / "attempts/disc_removed").exists()

        self.assertEqual({"DISC_CURRENT"}, reusable)
        self.assertTrue(current_exists)
        self.assertFalse(removed_exists)

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
                        "nextest_binary_path": str(executable),
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

    def test_resume_rejects_manifest_executable_that_is_not_current(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            old_executable = root / "old-test-binary"
            old_executable.write_bytes(b"old")
            current_executable = root / "current-test-binary"
            current_executable.write_bytes(b"current")
            attempt = root / "attempts/old"
            (attempt / "profiles").mkdir(parents=True)
            (attempt / "profiles/profile.profraw").write_bytes(b"profile")
            (attempt / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_ONE",
                        "binary_id": "example",
                        "test_name": "module::one",
                        "test_executable": str(old_executable),
                        "test_executable_digest": (
                            "sha256:"
                            + hashlib.sha256(old_executable.read_bytes()).hexdigest()
                        ),
                        "status": "completed",
                        "exit_code": 0,
                    }
                )
            )

            reusable = reusable_completed_attempt_ids(
                trace_root=root,
                resolved=[
                    {
                        "discovery_id": "DISC_ONE",
                        "nextest_binary_id": "example",
                        "nextest_binary_path": str(current_executable),
                        "nextest_test_name": "module::one",
                    }
                ],
                purge_incomplete=True,
            )

        self.assertEqual(set(), reusable)

    def test_resume_hashes_each_distinct_executable_once(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            resolved = []
            for suffix in ("ONE", "TWO"):
                executable = root / f"test-binary-{suffix.lower()}"
                executable.write_bytes(suffix.encode())
                attempt = root / f"attempts/{suffix.lower()}"
                (attempt / "profiles").mkdir(parents=True)
                (attempt / "profiles/profile.profraw").write_bytes(b"profile")
                (attempt / "attempt.json").write_text(
                    json.dumps(
                        {
                            "discovery_id": f"DISC_{suffix}",
                            "binary_id": f"example-{suffix.lower()}",
                            "test_name": f"module::{suffix.lower()}",
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
                resolved.append(
                    {
                        "discovery_id": f"DISC_{suffix}",
                        "nextest_binary_id": f"example-{suffix.lower()}",
                        "nextest_binary_path": str(executable),
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
        self.assertEqual(2, digest.call_count)

    def test_resume_purges_missing_non_file_and_changed_digest_executables(
        self,
    ) -> None:
        for case in ("missing", "directory", "changed_digest"):
            with self.subTest(case=case), TemporaryDirectory() as directory:
                root = Path(directory)
                executable = root / "test-binary"
                if case == "directory":
                    executable.mkdir()
                    manifest_digest = "sha256:not-a-file"
                elif case == "changed_digest":
                    executable.write_bytes(b"current")
                    manifest_digest = (
                        "sha256:" + hashlib.sha256(b"old").hexdigest()
                    )
                else:
                    manifest_digest = "sha256:missing"
                attempt = root / "attempts/one"
                (attempt / "profiles").mkdir(parents=True)
                (attempt / "profiles/profile.profraw").write_bytes(b"profile")
                (attempt / "attempt.json").write_text(
                    json.dumps(
                        {
                            "discovery_id": "DISC_ONE",
                            "binary_id": "example",
                            "test_name": "module::one",
                            "test_executable": str(executable),
                            "test_executable_digest": manifest_digest,
                            "status": "completed",
                            "exit_code": 0,
                        }
                    )
                )

                reusable = reusable_completed_attempt_ids(
                    trace_root=root,
                    resolved=[
                        {
                            "discovery_id": "DISC_ONE",
                            "nextest_binary_id": "example",
                            "nextest_binary_path": str(executable),
                            "nextest_test_name": "module::one",
                        }
                    ],
                    purge_incomplete=True,
                )

                self.assertEqual(set(), reusable)
                self.assertFalse(attempt.exists())

    def test_resume_rejects_duplicate_discovery_even_when_one_attempt_is_invalid(
        self,
    ) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "test-binary"
            executable.write_bytes(b"binary")
            valid = root / "attempts/valid"
            invalid = root / "attempts/invalid"
            (valid / "profiles").mkdir(parents=True)
            invalid.mkdir(parents=True)
            (valid / "profiles/profile.profraw").write_bytes(b"profile")
            (valid / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_DUPLICATE",
                        "binary_id": "example",
                        "test_name": "module::duplicate",
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
            (invalid / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_DUPLICATE",
                        "status": "started",
                    }
                )
            )

            with self.assertRaisesRegex(ValueError, "duplicates attempt"):
                reusable_completed_attempt_ids(
                    trace_root=root,
                    resolved=[
                        {
                            "discovery_id": "DISC_DUPLICATE",
                            "nextest_binary_id": "example",
                            "nextest_binary_path": str(executable),
                            "nextest_test_name": "module::duplicate",
                        }
                    ],
                    purge_incomplete=True,
                )

    def test_resume_rejects_executable_replaced_while_it_is_hashed(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "test-binary"
            executable.write_bytes(b"original")
            attempt = root / "attempts/one"
            (attempt / "profiles").mkdir(parents=True)
            (attempt / "profiles/profile.profraw").write_bytes(b"profile")
            (attempt / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_ONE",
                        "binary_id": "example",
                        "test_name": "module::one",
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
            original_digest = coverage_trace_resume._file_digest

            def replace_after_hash(path: Path) -> str:
                digest = original_digest(path)
                path.write_bytes(b"replacement")
                return digest

            with (
                mock.patch(
                    "scripts.verification.coverage_trace_resume._file_digest",
                    side_effect=replace_after_hash,
                ),
                self.assertRaisesRegex(ValueError, "changed during resume"),
            ):
                reusable_completed_attempt_ids(
                    trace_root=root,
                    resolved=[
                        {
                            "discovery_id": "DISC_ONE",
                            "nextest_binary_id": "example",
                            "nextest_binary_path": str(executable),
                            "nextest_test_name": "module::one",
                        }
                    ],
                    purge_incomplete=True,
                )


if __name__ == "__main__":
    unittest.main()
