#!/usr/bin/env python3
"""Subprocess behavior contract for the private pre-tool-use gate."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
HOOK = ROOT / ".claude" / "hooks" / "pre_tool_use_gate.py"
MAX_INPUT_BYTES = 1024 * 1024
CASE_FILE = Path(
    "verification/cases/verification/VERIFICATION_PRIVATE_HOOK_001.toml"
)
CASE_PARTITIONS = {
    "VERIFICATION_PRIVATE_HOOK_INPUT_FAIL_CLOSED": (
        "wrong_type_or_shape",
        (
            "test_rejects_malformed_or_non_utf8_input",
            "test_rejects_wrong_json_shapes",
            "test_rejects_empty_blank_nul_and_oversized_commands",
        ),
    ),
    "VERIFICATION_PRIVATE_HOOK_LIGHT_ALLOWED": (
        "happy_path",
        (
            "test_extra_top_level_fields_do_not_change_light_command_decision",
            "test_literal_mentions_are_not_misclassified_as_invocations",
            "test_unlisted_light_commands_are_allowed",
        ),
    ),
    "VERIFICATION_PRIVATE_HOOK_UNTRUSTED_HEAVY_DENIED": (
        "auth_or_permission",
        (
            "test_all_local_just_gate_forms_are_blocked",
            "test_all_local_cargo_gate_forms_are_blocked",
            "test_all_local_npm_gate_forms_are_blocked",
            "test_multiple_shell_segments_are_classified_independently",
            "test_dynamic_executable_or_subcommand_is_denied",
            "test_remote_text_does_not_exempt_a_local_heavy_command",
            "test_wrong_or_dynamic_remote_destination_is_not_trusted",
        ),
    ),
    "VERIFICATION_PRIVATE_HOOK_EXACT_REMOTE_ALLOWED": (
        "happy_path",
        ("test_exact_remote_builder_payload_is_allowed",),
    ),
}


def payload(command: object) -> bytes:
    return json.dumps({"tool_input": {"command": command}}).encode("utf-8")


def invoke(raw: bytes) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, str(HOOK)],
        input=raw,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        cwd=ROOT,
    )


class PrivateHookInputContractTests(unittest.TestCase):
    def assert_denied(self, raw: bytes, label: str) -> None:
        result = invoke(raw)
        self.assertEqual(result.returncode, 2, f"{label}: {result!r}")
        self.assertEqual(result.stdout, b"", label)
        self.assertTrue(result.stderr.strip(), label)

    def test_rejects_malformed_or_non_utf8_input(self) -> None:
        cases = {
            "empty": b"",
            "truncated object": b'{"tool_input":',
            "invalid utf8": b"\xff\xfe",
            "trailing JSON": b"{}{}",
        }
        for label, raw in cases.items():
            with self.subTest(label=label):
                self.assert_denied(raw, label)

    def test_rejects_wrong_json_shapes(self) -> None:
        cases = {
            "null root": b"null",
            "array root": b"[]",
            "missing tool_input": b"{}",
            "null tool_input": b'{"tool_input":null}',
            "array tool_input": b'{"tool_input":[]}',
            "missing command": b'{"tool_input":{}}',
            "null command": payload(None),
            "numeric command": payload(1),
            "array command": payload([]),
        }
        for label, raw in cases.items():
            with self.subTest(label=label):
                self.assert_denied(raw, label)

    def test_rejects_empty_blank_nul_and_oversized_commands(self) -> None:
        cases = {
            "empty command": payload(""),
            "blank command": payload(" \t\r\n"),
            "nul command": payload("rg foo\u0000bar"),
            "oversized input": payload("x" * MAX_INPUT_BYTES),
        }
        for label, raw in cases.items():
            with self.subTest(label=label):
                self.assert_denied(raw, label)

    def test_extra_top_level_fields_do_not_change_light_command_decision(self) -> None:
        raw = json.dumps(
            {
                "session_id": "ignored",
                "tool_name": "Bash",
                "tool_input": {"command": "rg -n pattern crates"},
            }
        ).encode("utf-8")
        result = invoke(raw)
        self.assertEqual(result.returncode, 0, result)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"")


class PrivateHookLocalPolicyTests(unittest.TestCase):
    def assert_allowed(self, command: str) -> None:
        result = invoke(payload(command))
        self.assertEqual(result.returncode, 0, f"{command!r}: {result!r}")
        self.assertEqual(result.stdout, b"", command)
        self.assertEqual(result.stderr, b"", command)

    def assert_blocked(self, command: str) -> None:
        result = invoke(payload(command))
        self.assertEqual(result.returncode, 2, f"{command!r}: {result!r}")
        self.assertEqual(result.stdout, b"", command)
        self.assertIn(b"trust-builder", result.stderr, command)

    def test_all_local_just_gate_forms_are_blocked(self) -> None:
        for command in [
            "just test",
            "just test-all",
            "just clippy",
            "just fmt",
            "/usr/bin/just test",
            "command just test",
            "env RUST_LOG=debug just test",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)

    def test_all_local_cargo_gate_forms_are_blocked(self) -> None:
        for subcommand in [
            "build",
            "test",
            "clippy",
            "check",
            "bench",
            "mutants",
            "fmt",
        ]:
            with self.subTest(subcommand=subcommand):
                self.assert_blocked(f"cargo {subcommand}")
        for command in [
            "/usr/bin/cargo test -p trust-hir",
            "command cargo test",
            "env RUSTFLAGS=-Dwarnings cargo check",
            "bash -lc 'cargo test -p trust-runtime'",
            'sh -c "cargo clippy"',
            "ca''rgo test",
            "cargo\\\n test",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)

    def test_all_local_npm_gate_forms_are_blocked(self) -> None:
        for command in [
            "npm test",
            "npm run test",
            "npm run compile",
            "npm run lint",
            "/usr/bin/npm test",
            "command npm run compile",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)

    def test_multiple_shell_segments_are_classified_independently(self) -> None:
        for command in [
            "rg pattern crates; cargo test",
            "git status && just clippy",
            "printf ok | npm test",
            "(cargo check)",
            "true\njust test-all",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)

    def test_dynamic_executable_or_subcommand_is_denied(self) -> None:
        for command in [
            'TOOL=cargo; "$TOOL" test',
            "TOOL=just; $TOOL clippy",
            'SUB=test; cargo "$SUB"',
            "TOOL=$(printf cargo); $TOOL test",
            "TOOL=`printf cargo`; $TOOL test",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)

    def test_literal_mentions_are_not_misclassified_as_invocations(self) -> None:
        for command in [
            "rg -n 'cargo test' docs",
            "printf '%s' 'just test-all'",
            "printf ok # npm test",
            "git grep 'cargo check'",
        ]:
            with self.subTest(command=command):
                self.assert_allowed(command)

    def test_unlisted_light_commands_are_allowed(self) -> None:
        for command in [
            "rg --files",
            "git status --short",
            "python3 scripts/plan_tests.py --help",
            "cargo metadata --no-deps",
            "npm --version",
        ]:
            with self.subTest(command=command):
                self.assert_allowed(command)


class PrivateHookRemoteBuilderTests(unittest.TestCase):
    def assert_allowed(self, command: str) -> None:
        result = invoke(payload(command))
        self.assertEqual(result.returncode, 0, f"{command!r}: {result!r}")
        self.assertEqual(result.stdout, b"", command)
        self.assertEqual(result.stderr, b"", command)

    def assert_blocked(self, command: str) -> None:
        result = invoke(payload(command))
        self.assertEqual(result.returncode, 2, f"{command!r}: {result!r}")
        self.assertEqual(result.stdout, b"", command)
        self.assertTrue(result.stderr.strip(), command)

    def test_exact_remote_builder_payload_is_allowed(self) -> None:
        for command in [
            "ssh trust-builder 'cd \"$HOME/projects/trust-platform\" && just test'",
            "ssh trust-builder 'cargo test -p trust-hir'",
            "ssh -F /home/johannes/.ssh/config trust-builder 'npm run compile'",
        ]:
            with self.subTest(command=command):
                self.assert_allowed(command)

    def test_remote_text_does_not_exempt_a_local_heavy_command(self) -> None:
        for command in [
            "echo 'ssh trust-builder'; cargo test",
            "printf '%s' 'ssh trust-builder' && just clippy",
            "ssh trust-builder 'true'; cargo check",
            "ssh trust-builder 'just test' && npm test",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)

    def test_wrong_or_dynamic_remote_destination_is_not_trusted(self) -> None:
        for command in [
            "ssh other-builder 'cargo test'",
            "ssh trust-builder.example.com 'cargo test'",
            "HOST=trust-builder; ssh $HOST 'cargo test'",
            "ssh 'trust-builder ' 'cargo test'",
        ]:
            with self.subTest(command=command):
                self.assert_blocked(command)


def _write_case_artifact(result: unittest.TestResult) -> None:
    keys = (
        "TRUST_VERIFY_TEST_ID",
        "TRUST_VERIFY_RUN_ID",
        "TRUST_VERIFY_ARTIFACT_DIR",
        "TRUST_VERIFY_CASE_FILE_DIGEST",
    )
    present = [key for key in keys if os.environ.get(key)]
    if not present:
        return
    if len(present) != len(keys):
        raise RuntimeError("partial TRUST_VERIFY environment")

    case_data = tomllib.loads((ROOT / CASE_FILE).read_text())
    configured = {case["id"]: case for case in case_data["case"]}
    failed = {
        test.id(): detail
        for test, detail in [*result.failures, *result.errors]
    }
    skipped = {test.id() for test, _reason in result.skipped}
    cases = []
    for case_id, (family, method_names) in CASE_PARTITIONS.items():
        matching_failures = [
            detail
            for test_id, detail in failed.items()
            if any(method_name in test_id for method_name in method_names)
        ]
        matching_skips = [
            test_id
            for test_id in skipped
            if any(method_name in test_id for method_name in method_names)
        ]
        if matching_failures:
            status = "failed"
            observed_error = matching_failures[0].splitlines()[-1][:1000]
        elif matching_skips:
            status = "skipped"
            observed_error = f"skipped tests: {', '.join(sorted(matching_skips))}"
        else:
            status = "passed"
            observed_error = None
        configured_case = configured[case_id]
        cases.append(
            {
                "id": case_id,
                "family": family,
                "result": status,
                "spec_gap_ref": None,
                "observed_error": observed_error,
                "observed_status": status,
                "state_delta": "not_applicable",
                "before": None,
                "after": None,
            }
        )
        if configured_case["family"] != family:
            raise RuntimeError(f"case family drift for {case_id}")

    artifact_dir = Path(os.environ["TRUST_VERIFY_ARTIFACT_DIR"])
    artifact_dir.mkdir(parents=True, exist_ok=True)
    test_id = os.environ["TRUST_VERIFY_TEST_ID"]
    document = {
        "schema_version": 1,
        "test_id": test_id,
        "case_file": CASE_FILE.as_posix(),
        "case_file_digest": os.environ["TRUST_VERIFY_CASE_FILE_DIGEST"],
        "helper_version": "verification-cases v1",
        "case_provenance_kind": case_data["case_provenance_kind"],
        "trace_definition_digest": case_data["trace_definition_digest"],
        "trust_verify_test_id": test_id,
        "trust_verify_run_id": os.environ["TRUST_VERIFY_RUN_ID"],
        "trust_verify_case_file_digest": os.environ[
            "TRUST_VERIFY_CASE_FILE_DIGEST"
        ],
        "trust_verify_artifact_dir": os.environ["TRUST_VERIFY_ARTIFACT_DIR"],
        "cases": cases,
    }
    destination = artifact_dir / f"{test_id}.json"
    destination.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    run = unittest.TextTestRunner(verbosity=1).run(suite)
    _write_case_artifact(run)
    raise SystemExit(0 if run.wasSuccessful() else 1)
