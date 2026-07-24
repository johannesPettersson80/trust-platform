"""Focused adversarial tests for raw execution-trace authentication."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest import mock

from scripts.verification.coverage_trace_evidence import (
    RawTraceEvidence,
    capture_raw_trace_evidence,
    validate_external_evidence,
)
from scripts.verification.coverage_trace_resume import profile_digest
from scripts.verification.focused_trace_authentication import (
    authenticate_focused_trace,
)


DISCOVERY_ID = "DISC_" + "A" * 20


class FocusedTraceAuthenticationTests(unittest.TestCase):
    def test_raw_evidence_rejects_profile_and_executable_forgery(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root, executable, llvm_tools = _raw_fixture(root)
            with mock.patch(
                "scripts.verification.coverage_trace_evidence."
                "_rustc_llvm_tools_directory",
                return_value=llvm_tools.resolve(),
            ):
                evidence = capture_raw_trace_evidence(
                    root=root, trace_root=trace_root, llvm_tools_dir=llvm_tools
                )
            (llvm_tools / "llvm-cov").write_bytes(b"swapped cov")
            self.assertRegex(
                "; ".join(validate_external_evidence(evidence)),
                "changed after snapshot",
            )
            (llvm_tools / "llvm-cov").write_bytes(b"cov")

            profile = next(trace_root.glob("attempts/*/profiles/*.profraw"))
            profile.write_bytes(b"forged profile")
            with self.assertRaisesRegex(ValueError, "profile digest drifted"):
                with mock.patch(
                    "scripts.verification.coverage_trace_evidence."
                    "_rustc_llvm_tools_directory",
                    return_value=llvm_tools.resolve(),
                ):
                    capture_raw_trace_evidence(
                        root=root,
                        trace_root=trace_root,
                        llvm_tools_dir=llvm_tools,
                    )

            trace_root, executable, llvm_tools = _raw_fixture(root / "second")
            executable.write_bytes(b"forged executable")
            with self.assertRaisesRegex(ValueError, "executable digest drifted"):
                with mock.patch(
                    "scripts.verification.coverage_trace_evidence."
                    "_rustc_llvm_tools_directory",
                    return_value=llvm_tools.resolve(),
                ):
                    capture_raw_trace_evidence(
                        root=root / "second",
                        trace_root=trace_root,
                        llvm_tools_dir=llvm_tools,
                    )

    def test_hand_authored_observed_trace_fails_reconstruction(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root = root / "target/gate-artifacts/trace"
            trace_root.mkdir(parents=True)
            evidence = RawTraceEvidence(
                trace_root=trace_root,
                repository_originals={
                    "target/gate-artifacts/trace/requested-tests.json": (
                        json.dumps(
                            {
                                "schema_version": 2,
                                "tests": [
                                    {
                                        "discovery_id": DISCOVERY_ID,
                                        "nextest_binary_id": "example::lib",
                                        "nextest_binary_path": "/tmp/example",
                                        "nextest_test_name": "tests::case",
                                    }
                                ],
                            },
                            indent=2,
                            sort_keys=True,
                        )
                        + "\n"
                    ).encode()
                },
                executable_originals={},
                tool_digests={
                    "llvm-profdata": "sha256:" + "4" * 64,
                    "llvm-cov": "sha256:" + "5" * 64,
                },
                tool_paths={
                    "llvm-profdata": "/toolchain/llvm-profdata",
                    "llvm-cov": "/toolchain/llvm-cov",
                },
                digest="sha256:" + "1" * 64,
            )
            resolved = {
                "discovery_id": DISCOVERY_ID,
                "discovery_source_kind": "rust_unit_test",
                "path": "crates/example/src/lib.rs",
                "name": "case",
                "ignore_state": "not_ignored",
                "package": "example",
                "package_has_library": True,
                "package_binary_names": [],
                "cargo_target_kind": "lib",
                "cargo_target_name": None,
                "nextest_binary_id": "example::lib",
                "nextest_binary_path": "/tmp/example",
                "nextest_test_name": "tests::case",
                "command": "cargo nextest run",
                "command_authority": "verified_synthesized",
            }
            trace = {
                "source_commit": "a" * 40,
                "platform": "linux-x86_64",
                "input_digest": "sha256:" + "2" * 64,
                "input_provenance": {
                    "kind": "focused_rust_v1",
                    "trace_root": "target/gate-artifacts/trace",
                    "raw_evidence_digest": evidence.digest,
                    "llvm_tool_digests": evidence.tool_digests,
                    "llvm_tool_paths": evidence.tool_paths,
                    "resolved": [resolved],
                },
                "tests": [
                    {
                        "discovery_id": DISCOVERY_ID,
                        "trace_status": "observed",
                        "covered_code_ids": ["CODE_" + "F" * 64],
                    }
                ],
            }
            context = SimpleNamespace(
                selected_live_facts=(
                    SimpleNamespace(
                        stable_id=DISCOVERY_ID,
                        source_kind="rust_unit_test",
                        path="crates/example/src/lib.rs",
                        name="case",
                        ignore_state="not_ignored",
                        package="example",
                    ),
                )
            )
            honest = {**trace, "tests": []}
            with mock.patch(
                "scripts.verification.focused_trace_authentication._run",
                side_effect=[
                    mock.Mock(stdout='{"packages": []}'),
                    mock.Mock(stdout="unused"),
                    mock.Mock(stdout="{}"),
                ],
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "parse_llvm_cov_environment",
                return_value={},
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "rust_package_target_inventory",
                return_value={
                    "example": {
                        "package_has_library": True,
                        "package_binary_names": [],
                    }
                },
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "targeted_nextest_list_shards",
                return_value=[{"command": ["cargo"], "targets": []}],
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "resolve_nextest_list_shard",
                return_value=([resolved], []),
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "reusable_completed_attempt_ids",
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "export_attempt_coverage",
                return_value={DISCOVERY_ID: {}},
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "executed_repository_paths",
                return_value=(),
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "focused_rust_observed_source_projection",
                return_value=({}, "sha256:" + "3" * 64),
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "focused_rust_path_inventory",
                return_value=SimpleNamespace(spans={}),
            ), mock.patch(
                "scripts.verification.focused_trace_authentication."
                "build_rust_trace_document",
                return_value=honest,
            ):
                with self.assertRaisesRegex(
                    ValueError, "differs from raw execution reconstruction"
                ):
                    authenticate_focused_trace(
                        root=root,
                        trace=trace,
                        context=context,
                        metadata=SimpleNamespace(mappings={}),
                        trace_root=trace_root,
                        cargo_target_dir=root / "target",
                        llvm_tools_dir=root / "llvm",
                        temporary_dir=root / "temporary",
                        evidence=evidence,
                    )


def _raw_fixture(root: Path) -> tuple[Path, Path, Path]:
    trace_root = root / "target/gate-artifacts/trace"
    attempt = trace_root / "attempts/key"
    profiles = attempt / "profiles"
    profiles.mkdir(parents=True)
    (trace_root / "requested-tests.json").write_text(
        '{"schema_version":2,"tests":[]}\n'
    )
    (trace_root / "nextest.toml").write_text("[profile.trace]\n")
    profile = profiles / "case.profraw"
    profile.write_bytes(b"profile")
    executable = root / "target/test-binary"
    executable.parent.mkdir(parents=True, exist_ok=True)
    executable.write_bytes(b"binary")
    llvm_tools = root / "llvm-tools"
    llvm_tools.mkdir()
    (llvm_tools / "llvm-profdata").write_bytes(b"profdata")
    (llvm_tools / "llvm-cov").write_bytes(b"cov")
    attempt_manifest = {
        "discovery_id": DISCOVERY_ID,
        "test_executable": executable.resolve().as_posix(),
        "test_executable_digest": _digest(executable.read_bytes()),
        "profile_digest": profile_digest(profiles),
    }
    (attempt / "attempt.json").write_text(json.dumps(attempt_manifest))
    return trace_root, executable, llvm_tools


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


if __name__ == "__main__":
    unittest.main()
