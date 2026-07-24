"""Focused tests for direct Rust-backed test trace orchestration."""

from __future__ import annotations

import hashlib
import json
import subprocess
import threading
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import patch

from scripts.verification.coverage_trace_direct import (
    direct_instrumentation_environment,
    direct_test_command,
    direct_test_request,
    parse_llvm_cov_environment,
    run_direct_trace,
)
from scripts.verification.coverage_trace_resume import (
    reusable_completed_attempt_ids,
)


class CoverageTraceDirectTests(unittest.TestCase):
    def test_structured_text_command_is_derived_from_live_project_geometry(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            source = (
                root
                / "crates/example/tests/fixtures/group/project/src/main.st"
            )
            source.parent.mkdir(parents=True)
            source.write_text("TEST_PROGRAM verifies_value\nEND_TEST_PROGRAM\n")
            trust_dev = root / "bin/trust-dev"
            trust_runtime = root / "bin/trust-runtime"
            trust_dev.parent.mkdir()
            trust_dev.touch()
            trust_runtime.touch()
            fact = {
                "discovery_source_kind": "structured_text_test",
                "path": (
                    "crates/example/tests/fixtures/group/project/src/main.st"
                ),
                "name": "verifies_value",
                "command_hint": (
                    "cargo run -p trust-dev -- test "
                    "--project crates/example/tests/fixtures/group/project "
                    "--filter verifies_value"
                ),
            }

            command, authority = direct_test_command(
                fact,
                root=root,
                trust_dev_bin=trust_dev,
                trust_runtime_bin=trust_runtime,
            )

        self.assertEqual("verified_synthesized", authority)
        self.assertEqual(
            [
                str(trust_dev.resolve()),
                "test",
                "--project",
                "crates/example/tests/fixtures/group/project",
                "--filter",
                "verifies_value",
            ],
            command,
        )

    def test_conformance_command_preserves_exact_scanner_contract(self) -> None:
        root = Path("/repo")
        command, authority = direct_test_command(
            {
                "discovery_source_kind": "conformance_case",
                "path": "conformance/cases/timers/example/manifest.toml",
                "name": "cfm_timer_001",
                "command_hint": (
                    "cargo run -p trust-runtime --bin trust-runtime -- "
                    "conformance --suite-root conformance --filter cfm_timer_001"
                ),
            },
            root=root,
            trust_dev_bin=Path("/repo/target/debug/trust-dev"),
            trust_runtime_bin=Path("/repo/target/debug/trust-runtime"),
        )

        self.assertEqual("exact", authority)
        self.assertEqual(
            [
                "/repo/target/debug/trust-runtime",
                "conformance",
                "--suite-root",
                "conformance",
                "--filter",
                "cfm_timer_001",
            ],
            command,
        )

    def test_llvm_environment_is_parsed_without_shell_evaluation(self) -> None:
        environment = parse_llvm_cov_environment(
            "LLVM_PROFILE_FILE='/repo/target/%p-%m.profraw'\n"
            "CARGO_LLVM_COV=1\n"
            "RUSTC_WRAPPER='/tools/cargo-llvm-cov'\n"
            "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS='-C\x1finstrument-coverage'\n"
        )

        self.assertEqual(
            "/repo/target/%p-%m.profraw",
            environment["LLVM_PROFILE_FILE"],
        )
        self.assertEqual(
            "-C\x1finstrument-coverage",
            environment["__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS"],
        )

    def test_direct_build_flags_participate_in_cargo_fingerprint(self) -> None:
        environment = direct_instrumentation_environment(
            {"RUSTFLAGS": "-Dwarnings", "RUSTC_WRAPPER": "old"},
            {
                "LLVM_PROFILE_FILE": "/tmp/%p.profraw",
                "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS": (
                    "-C\x1finstrument-coverage"
                ),
                "__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING": "sccache",
            },
        )

        self.assertEqual(
            "-Dwarnings -C instrument-coverage",
            environment["RUSTFLAGS"],
        )
        self.assertEqual("sccache", environment["RUSTC_WRAPPER"])
        self.assertEqual("0", environment["CARGO_INCREMENTAL"])

    def test_command_hint_drift_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "drifted"):
            direct_test_command(
                {
                    "discovery_source_kind": "conformance_case",
                    "path": "manifest.toml",
                    "name": "case",
                    "command_hint": "echo case",
                },
                root=Path("/repo"),
                trust_dev_bin=Path("/dev/null"),
                trust_runtime_bin=Path("/dev/null"),
            )

    def test_direct_request_can_resume_exact_completed_attempt(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "target/debug/trust-dev"
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b"instrumented direct runner")
            discovery_id = "DISC_TEST_DIRECT"
            fact = {
                "discovery_id": discovery_id,
                "discovery_source_kind": "structured_text_test",
                "path": "fixtures/project/src/main.st",
                "name": "checks_value",
                "package": None,
            }
            command = [
                str(executable),
                "test",
                "--project",
                "fixtures/project",
                "--filter",
                "checks_value",
            ]
            request = direct_test_request(
                fact,
                command=command,
                authority="verified_synthesized",
            )
            attempt = (
                root
                / "trace/attempts"
                / hashlib.sha256(discovery_id.encode()).hexdigest()
            )
            profiles = attempt / "profiles"
            profiles.mkdir(parents=True)
            (profiles / "coverage.profraw").write_bytes(b"profile")
            (attempt / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": discovery_id,
                        "binary_id": executable.name,
                        "test_name": "checks_value",
                        "test_executable": str(executable.resolve()),
                        "test_executable_digest": (
                            "sha256:" + hashlib.sha256(
                                executable.read_bytes()
                            ).hexdigest()
                        ),
                        "status": "completed",
                        "exit_code": 0,
                    }
                )
            )

            reusable = reusable_completed_attempt_ids(
                trace_root=root / "trace",
                resolved=[request],
                purge_incomplete=True,
            )

        self.assertEqual({discovery_id}, reusable)
        self.assertEqual(str(executable.resolve()), request["nextest_binary_path"])

    def test_two_st_workers_are_isolated_and_conformance_is_serialized(
        self,
    ) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            trust_dev, trust_runtime = self._write_binaries(root)
            project = root / "tests/group/project"
            source = project / "src/main.st"
            source.parent.mkdir(parents=True)
            source.write_text("TEST_PROGRAM A\nEND_TEST_PROGRAM\n")
            facts = [
                self._st_fact("DISC_A", "case_a"),
                self._st_fact("DISC_B", "case_b"),
                {
                    "discovery_id": "DISC_C",
                    "discovery_source_kind": "conformance_case",
                    "path": "conformance/cases/case_c/manifest.toml",
                    "name": "case_c",
                    "package": None,
                    "command_hint": (
                        "cargo run -p trust-runtime --bin trust-runtime -- "
                        "conformance --suite-root conformance --filter case_c"
                    ),
                },
            ]
            lock = threading.Lock()
            barrier = threading.Barrier(2)
            active_st = 0
            maximum_st = 0
            conformance_overlapped = False
            temporary_roots: set[str] = set()

            def fake_run(command, *, env, **_kwargs):
                nonlocal active_st, maximum_st, conformance_overlapped
                is_st = Path(command[0]).name == "trust-dev"
                with lock:
                    if is_st:
                        active_st += 1
                        maximum_st = max(maximum_st, active_st)
                    elif active_st:
                        conformance_overlapped = True
                    temporary_roots.add(env["TMPDIR"])
                    self.assertEqual(env["TMPDIR"], env["TMP"])
                    self.assertEqual(env["TMPDIR"], env["TEMP"])
                if is_st:
                    barrier.wait(timeout=2)
                    time.sleep(0.01)
                profile = Path(
                    env["LLVM_PROFILE_FILE"].replace(
                        "%m-%p", f"profile-{command[-1]}"
                    )
                )
                profile.write_bytes(b"profile")
                with lock:
                    if is_st:
                        active_st -= 1
                return subprocess.CompletedProcess(command, 0, "", "")

            with (
                patch(
                    "scripts.verification.coverage_trace_direct.subprocess.run",
                    side_effect=fake_run,
                ),
                patch(
                    "scripts.verification.coverage_trace_direct."
                    "export_attempt_coverage",
                    return_value={},
                ),
                patch(
                    "scripts.verification.coverage_trace_direct."
                    "build_rust_trace_document",
                    return_value={"tests": []},
                ),
            ):
                run_direct_trace(
                    root=root,
                    facts=facts,
                    trace_root=root / "trace",
                    output_path=root / "trace.json",
                    binaries={
                        "trust-dev": trust_dev,
                        "trust-runtime": trust_runtime,
                    },
                    environment={},
                    llvm_tools_dir=root / "llvm",
                    rust_inventory=SimpleNamespace(
                        source_paths=[],
                        spans={},
                        facts={},
                    ),
                    source_commit="0" * 40,
                    platform="test",
                    input_digest="sha256:test",
                    mode="focused_shard",
                    jobs=2,
                )

        self.assertEqual(2, maximum_st)
        self.assertFalse(conformance_overlapped)
        self.assertEqual(3, len(temporary_roots))

    def test_worker_failure_blocks_export_and_writes_no_manifest(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            trust_dev, trust_runtime = self._write_binaries(root)
            source = root / "tests/group/project/src/main.st"
            source.parent.mkdir(parents=True)
            source.write_text("TEST_PROGRAM A\nEND_TEST_PROGRAM\n")
            facts = [
                self._st_fact("DISC_FAIL", "case_fail"),
                self._st_fact("DISC_PASS", "case_pass"),
            ]

            def fake_run(command, *, env, **_kwargs):
                if command[-1] == "case_fail":
                    return subprocess.CompletedProcess(
                        command, 1, "", "expected failure"
                    )
                profile = Path(
                    env["LLVM_PROFILE_FILE"].replace("%m-%p", "profile")
                )
                profile.write_bytes(b"profile")
                return subprocess.CompletedProcess(command, 0, "", "")

            with (
                patch(
                    "scripts.verification.coverage_trace_direct.subprocess.run",
                    side_effect=fake_run,
                ),
                patch(
                    "scripts.verification.coverage_trace_direct."
                    "export_attempt_coverage"
                ) as export,
            ):
                with self.assertRaisesRegex(
                    ValueError, "DISC_FAIL.*expected failure"
                ):
                    run_direct_trace(
                        root=root,
                        facts=facts,
                        trace_root=root / "trace",
                        output_path=root / "trace.json",
                        binaries={
                            "trust-dev": trust_dev,
                            "trust-runtime": trust_runtime,
                        },
                        environment={},
                        llvm_tools_dir=root / "llvm",
                        rust_inventory=SimpleNamespace(
                            source_paths=[],
                            spans={},
                            facts={},
                        ),
                        source_commit="0" * 40,
                        platform="test",
                        input_digest="sha256:test",
                        mode="focused_shard",
                        jobs=2,
                    )

            export.assert_not_called()
            failed = (
                root
                / "trace/attempts"
                / hashlib.sha256(b"DISC_FAIL").hexdigest()
                / "attempt.json"
            )
            passed = (
                root
                / "trace/attempts"
                / hashlib.sha256(b"DISC_PASS").hexdigest()
                / "attempt.json"
            )
            self.assertFalse(failed.exists())
            self.assertTrue(passed.exists())

    def test_direct_jobs_outside_reviewed_bound_fail_closed(self) -> None:
        for jobs in (0, 3):
            with self.subTest(jobs=jobs):
                with self.assertRaisesRegex(ValueError, "must be 1 or 2"):
                    run_direct_trace(
                        root=Path("/repo"),
                        facts=[],
                        trace_root=Path("/trace"),
                        output_path=Path("/trace.json"),
                        binaries={},
                        environment={},
                        llvm_tools_dir=Path("/llvm"),
                        rust_inventory=SimpleNamespace(),
                        source_commit="0" * 40,
                        platform="test",
                        input_digest="sha256:test",
                        mode="focused_shard",
                        jobs=jobs,
                    )

    def test_profile_or_executable_instability_blocks_export(self) -> None:
        for scenario in ("missing_profile", "mutated_executable"):
            with self.subTest(scenario=scenario), TemporaryDirectory() as directory:
                root = Path(directory)
                trust_dev, trust_runtime = self._write_binaries(root)
                source = root / "tests/group/project/src/main.st"
                source.parent.mkdir(parents=True)
                source.write_text("TEST_PROGRAM A\nEND_TEST_PROGRAM\n")

                def fake_run(command, *, env, **_kwargs):
                    if scenario == "mutated_executable":
                        trust_dev.write_bytes(b"changed while executing")
                    else:
                        self.assertEqual("missing_profile", scenario)
                    if scenario != "missing_profile":
                        profile = Path(
                            env["LLVM_PROFILE_FILE"].replace(
                                "%m-%p", "profile"
                            )
                        )
                        profile.write_bytes(b"profile")
                    return subprocess.CompletedProcess(command, 0, "", "")

                with (
                    patch(
                        "scripts.verification.coverage_trace_direct."
                        "subprocess.run",
                        side_effect=fake_run,
                    ),
                    patch(
                        "scripts.verification.coverage_trace_direct."
                        "export_attempt_coverage"
                    ) as export,
                ):
                    with self.assertRaises(ValueError):
                        run_direct_trace(
                            root=root,
                            facts=[self._st_fact("DISC_A", "case_a")],
                            trace_root=root / "trace",
                            output_path=root / "trace.json",
                            binaries={
                                "trust-dev": trust_dev,
                                "trust-runtime": trust_runtime,
                            },
                            environment={},
                            llvm_tools_dir=root / "llvm",
                            rust_inventory=SimpleNamespace(
                                source_paths=[],
                                spans={},
                                facts={},
                            ),
                            source_commit="0" * 40,
                            platform="test",
                            input_digest="sha256:test",
                            mode="focused_shard",
                            jobs=1,
                        )

                export.assert_not_called()

    @staticmethod
    def _write_binaries(root: Path) -> tuple[Path, Path]:
        binary_root = root / "target/debug"
        binary_root.mkdir(parents=True)
        trust_dev = binary_root / "trust-dev"
        trust_runtime = binary_root / "trust-runtime"
        trust_dev.write_bytes(b"trust-dev")
        trust_runtime.write_bytes(b"trust-runtime")
        return trust_dev, trust_runtime

    @staticmethod
    def _st_fact(discovery_id: str, name: str) -> dict[str, object]:
        return {
            "discovery_id": discovery_id,
            "discovery_source_kind": "structured_text_test",
            "path": "tests/group/project/src/main.st",
            "name": name,
            "package": None,
            "command_hint": (
                "cargo run -p trust-dev -- test "
                f"--project tests/group/project --filter {name}"
            ),
        }


if __name__ == "__main__":
    unittest.main()
