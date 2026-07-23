"""Focused tests for direct Rust-backed test trace orchestration."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.verification.coverage_trace_direct import (
    direct_instrumentation_environment,
    direct_test_command,
    parse_llvm_cov_environment,
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


if __name__ == "__main__":
    unittest.main()
