"""Execution-source binding tests for the Phase 9 fuzz program."""

from __future__ import annotations

import tomllib
import unittest
from unittest import mock

from .fuzz_program_contract import TARGET_ID_ORDER, load_fuzz_program
from .fuzz_program_source_contract import (
    EXECUTION_SOURCE_PATHS,
    REVIEWED_EXECUTABLE_MODE,
    _tracked_git_mode,
    validate_execution_source_bindings,
    validate_reviewed_execution_source_digests,
)
from .metadata_validator.constants import ROOT


class FuzzProgramSourceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.program = load_fuzz_program(ROOT)
        cls.sources = {
            relative: (ROOT / relative).read_bytes()
            for relative in EXECUTION_SOURCE_PATHS
        }

    def test_reviewed_source_digests_and_executable_modes_match_live_tree(self) -> None:
        failures: list[str] = []
        validate_reviewed_execution_source_digests(self.sources, failures)
        self.assertEqual([], failures)
        for relative in REVIEWED_EXECUTABLE_MODE:
            with self.subTest(relative=relative):
                self.assertEqual("100755", _tracked_git_mode(ROOT, relative))

    def test_dead_or_uncalled_script_wrappers_fail_even_when_commands_remain(self) -> None:
        for relative in REVIEWED_EXECUTABLE_MODE:
            for prefix, suffix in (
                (b"if false; then\n", b"\nfi\n"),
                (b"never_called() {\n", b"\n}\n"),
            ):
                with self.subTest(relative=relative, prefix=prefix):
                    sources = dict(self.sources)
                    sources[relative] = prefix + sources[relative] + suffix
                    failures: list[str] = []
                    validate_reviewed_execution_source_digests(sources, failures)
                    self.assertTrue(
                        any(relative in item and "digest drifted" in item for item in failures),
                        failures,
                    )

    def test_trigger_reviewed_job_and_unreviewed_workflow_changes_all_fail(self) -> None:
        for relative in (".github/workflows/ci.yml", ".github/workflows/salsa-hardening.yml"):
            with self.subTest(relative=relative, mutation="trigger"):
                sources = dict(self.sources)
                sources[relative] = sources[relative].replace(
                    b"  pull_request:\n", b"  # pull_request removed\n", 1
                )
                failures: list[str] = []
                validate_reviewed_execution_source_digests(sources, failures)
                self.assertTrue(any(f"{relative}#on" in item for item in failures), failures)

        sources = dict(self.sources)
        sources[".github/workflows/ci.yml"] = sources[".github/workflows/ci.yml"].replace(
            b"  fmt:\n", b"  fmt:\n    # whole-workflow review boundary changed\n", 1
        )
        failures = []
        validate_reviewed_execution_source_digests(sources, failures)
        self.assertTrue(any("ci.yml digest drifted" in item for item in failures), failures)

    def test_top_level_defaults_duplicates_and_crlf_fail_raw_binding(self) -> None:
        for relative in (".github/workflows/ci.yml", ".github/workflows/salsa-hardening.yml"):
            for insertion in (
                b"defaults:\n  run:\n    shell: bash -n {0}\n\n",
                b"on:\n  workflow_dispatch:\n\n",
                b"jobs:\n\n",
            ):
                with self.subTest(relative=relative, insertion=insertion):
                    sources = dict(self.sources)
                    sources[relative] = sources[relative].replace(b"jobs:\n", insertion + b"jobs:\n", 1)
                    failures: list[str] = []
                    validate_reviewed_execution_source_digests(sources, failures)
                    self.assertTrue(failures)

        sources = dict(self.sources)
        relative = "scripts/salsa_fuzz_gate.sh"
        sources[relative] = sources[relative].replace(b"\n", b"\r\n")
        failures = []
        validate_reviewed_execution_source_digests(sources, failures)
        self.assertTrue(any(relative in item and "digest drifted" in item for item in failures), failures)

    def test_mode_and_effective_default_workspace_membership_fail_closed(self) -> None:
        with mock.patch(
            "scripts.verification.fuzz_program_source_contract._tracked_git_mode",
            return_value="100644",
        ):
            failures: list[str] = []
            validate_execution_source_bindings(ROOT, self.program["targets"], TARGET_ID_ORDER, failures)
        self.assertTrue(any("must retain tracked mode 100755" in item for item in failures), failures)

        workspace = tomllib.loads(self.sources["Cargo.toml"].decode())
        self.assertNotIn("default-members", workspace["workspace"])
        sources = dict(self.sources)
        sources["Cargo.toml"] = sources["Cargo.toml"].replace(
            b"resolver = \"2\"\n",
            b"resolver = \"2\"\ndefault-members = [\"crates/trust-runtime\"]\n",
            1,
        )
        with mock.patch(
            "scripts.verification.fuzz_program_source_contract._load_execution_sources",
            return_value=sources,
        ):
            failures = []
            validate_execution_source_bindings(ROOT, self.program["targets"], TARGET_ID_ORDER, failures)
        self.assertTrue(any("effective default workspace" in item for item in failures), failures)


if __name__ == "__main__":
    unittest.main()
