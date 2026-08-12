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
from scripts.verification.focused_trace_receipt import (
    _write_json_atomic,
    load_authenticated_trace_receipt,
)


DISCOVERY_ID = "DISC_" + "A" * 20


class FocusedTraceAuthenticationTests(unittest.TestCase):
    def test_receipt_output_is_committed_atomically_and_is_idempotent(self) -> None:
        with TemporaryDirectory() as temporary:
            receipt = Path(temporary) / "receipts" / "focused.json"
            document = {"schema_version": 1, "value": "reviewed"}

            _write_json_atomic(receipt, document)
            first = receipt.read_bytes()
            _write_json_atomic(receipt, document)

            self.assertEqual(first, receipt.read_bytes())
            self.assertEqual(
                [],
                list(receipt.parent.glob(f".{receipt.name}.*.staged")),
            )

    def test_receipt_output_refuses_to_overwrite_different_evidence(self) -> None:
        with TemporaryDirectory() as temporary:
            receipt = Path(temporary) / "receipt.json"
            receipt.write_text('{"existing":"evidence"}\n')
            original = receipt.read_bytes()

            with self.assertRaisesRegex(
                ValueError,
                "destination already differs",
            ):
                _write_json_atomic(
                    receipt,
                    {"schema_version": 1, "value": "replacement"},
                )

            self.assertEqual(original, receipt.read_bytes())
            self.assertEqual(
                [],
                list(receipt.parent.glob(f".{receipt.name}.*.staged")),
            )

    def test_cached_receipt_path_does_not_run_live_commands_or_export(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root = root / "target/gate-artifacts/trace"
            trace_root.mkdir(parents=True)
            receipt_path = root / "target/gate-artifacts/receipt.json"
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"pub fn covered() {}\n")
            requested = b'{"schema_version":2,"tests":[]}\n'
            repository_originals = {
                "target/gate-artifacts/trace/requested-tests.json": requested,
                "target/gate-artifacts/trace/nextest.toml": b"[profile.trace]\n",
                "target/gate-artifacts/trace/attempts/key/attempt.json": b"{}",
                "target/gate-artifacts/trace/attempts/key/profiles/case.profraw": b"profile",
            }
            toolchain = root / "toolchain"
            toolchain.mkdir()
            executable_originals = {
                toolchain / "test-binary": b"binary",
                toolchain / "llvm-profdata": b"profdata",
                toolchain / "llvm-cov": b"cov",
            }
            for path, content in executable_originals.items():
                path.write_bytes(content)
            evidence = RawTraceEvidence(
                trace_root=trace_root,
                repository_originals=repository_originals,
                executable_originals=executable_originals,
                tool_digests={
                    "llvm-profdata": "sha256:" + "4" * 64,
                    "llvm-cov": "sha256:" + "5" * 64,
                },
                tool_paths={
                    "llvm-profdata": (toolchain / "llvm-profdata").as_posix(),
                    "llvm-cov": (toolchain / "llvm-cov").as_posix(),
                },
                digest="sha256:" + "1" * 64,
            )
            discovery_id = DISCOVERY_ID
            resolved = [{"discovery_id": discovery_id, "name": "case"}]
            trace = {
                "source_commit": "a" * 40,
                "scope": {"requested_discovery_ids": [discovery_id]},
                "input_provenance": {"resolved": resolved},
            }
            receipt_path.write_text(
                json.dumps(
                    _receipt_fixture(
                        trace=trace,
                        requested=[discovery_id],
                        evidence=evidence,
                        source=source,
                        resolved=resolved,
                    ),
                    sort_keys=True,
                )
            )

            with mock.patch(
                "scripts.verification.focused_trace_authentication._run",
                side_effect=AssertionError("unexpected live command"),
            ), mock.patch(
                "scripts.verification.focused_trace_authentication.export_attempt_coverage",
                side_effect=AssertionError("unexpected LLVM export"),
            ), mock.patch(
                "scripts.verification.focused_trace_authentication.targeted_nextest_list_shards",
                side_effect=AssertionError("unexpected nextest listing"),
            ):
                loaded = load_authenticated_trace_receipt(
                    root=root,
                    receipt_path=receipt_path,
                    trace=trace,
                    requested=[discovery_id],
                    trace_root=trace_root,
                    llvm_tools_dir=root / "llvm-tools",
                    evidence=evidence,
                )

            self.assertEqual(
                {"crates/example/src/lib.rs": source.read_bytes()},
                dict(loaded.source_originals),
            )

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

    def test_raw_evidence_reads_each_shared_executable_once(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            trace_root, executable, llvm_tools = _raw_fixture(root)
            first = next(trace_root.glob("attempts/*/attempt.json"))
            second = trace_root / "attempts/second"
            second_profiles = second / "profiles"
            second_profiles.mkdir(parents=True)
            (second_profiles / "case.profraw").write_bytes(b"second profile")
            manifest = json.loads(first.read_text())
            manifest["discovery_id"] = "DISC_" + "B" * 20
            manifest["profile_digest"] = profile_digest(second_profiles)
            (second / "attempt.json").write_text(json.dumps(manifest))
            original_read_bytes = Path.read_bytes
            executable_reads = 0

            def counted_read_bytes(path: Path) -> bytes:
                nonlocal executable_reads
                if path.resolve() == executable.resolve():
                    executable_reads += 1
                return original_read_bytes(path)

            with mock.patch(
                "scripts.verification.coverage_trace_evidence."
                "_rustc_llvm_tools_directory",
                return_value=llvm_tools.resolve(),
            ), mock.patch.object(
                Path,
                "read_bytes",
                autospec=True,
                side_effect=counted_read_bytes,
            ):
                capture_raw_trace_evidence(
                    root=root,
                    trace_root=trace_root,
                    llvm_tools_dir=llvm_tools,
                )

            self.assertEqual(1, executable_reads)

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
                "command": (
                    "cargo nextest run -p example --lib --features hir "
                    "-E '(binary_id(=example::lib) & test(=tests::case))'"
                ),
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
            ) as targeted_list, mock.patch(
                "scripts.verification.focused_trace_authentication."
                "resolve_nextest_list_shard",
                return_value=([resolved], []),
            ) as resolve_list, mock.patch(
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
                self.assertEqual(
                    ("hir",),
                    targeted_list.call_args.kwargs.get("cargo_features"),
                )
                self.assertEqual(
                    ("hir",),
                    resolve_list.call_args.kwargs.get("cargo_features"),
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


def _receipt_fixture(
    *,
    trace: dict[str, object],
    requested: list[str],
    evidence: RawTraceEvidence,
    source: Path,
    resolved: list[dict[str, str]],
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "kind": "focused_trace_authentication_receipt_v1",
        "source_commit": trace["source_commit"],
        "trace_digest": _canonical_digest(trace),
        "rebuilt_trace_digest": _canonical_digest(trace),
        "trace_root": evidence.trace_root.relative_to(source.parents[3]).as_posix(),
        "requested_discovery_ids": requested,
        "requested_tests_digest": _digest(
            evidence.repository_originals[
                "target/gate-artifacts/trace/requested-tests.json"
            ]
        ),
        "resolved_digest": _canonical_digest(resolved),
        "raw_evidence": {
            "digest": evidence.digest,
            "repository_files": {
                path: _digest(content)
                for path, content in evidence.repository_originals.items()
            },
            "executable_files": {
                path.as_posix(): _digest(content)
                for path, content in evidence.executable_originals.items()
            },
            "tool_digests": evidence.tool_digests,
            "tool_paths": evidence.tool_paths,
        },
        "source_files": {
            source.relative_to(source.parents[3]).as_posix(): _digest(
                source.read_bytes()
            )
        },
    }


def _canonical_digest(document: object) -> str:
    return _digest(
        json.dumps(
            document,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode()
    )


if __name__ == "__main__":
    unittest.main()
