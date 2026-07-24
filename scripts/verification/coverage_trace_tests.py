"""Focused tests for per-test profile isolation and trace semantics."""

from __future__ import annotations

import tempfile
import json
import unittest
from pathlib import Path

from scripts.verification.coverage_trace import validate_trace_document
from scripts.verification.coverage_trace_wrapper import (
    complete_attempt_manifest,
    prepare_attempt_environment,
)


DISCOVERY_ID = "DISC_00000000000000000001"


def _fact() -> dict[str, object]:
    return {
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/lib.rs",
        "name": "example",
    }


def _trace(mode: str = "focused_shard") -> dict[str, object]:
    return {
        "schema_version": 1,
        "generator": "test-coverage-trace",
        "generator_version": "1",
        "source_commit": "a" * 40,
        "platform": "linux-x86_64",
        "scope": {
            "mode": mode,
            "requested_discovery_ids": [DISCOVERY_ID],
            "source_kinds": ["rust_unit_test"],
        },
        "input_digest": "sha256:" + "1" * 64,
        "code_inventory_digests": {
            "rust_product": "sha256:" + "3" * 64,
        },
        "tests": [
            {
                **_fact(),
                "command": "cargo nextest run example",
                "command_authority": "exact",
                "trace_status": "observed",
                "attribution_kind": "llvm_source_coverage",
                "attempt_id": "attempt-1",
                "exit_code": 0,
                "duration_ms": 10,
                "profile_digest": "sha256:" + "2" * 64,
                "covered_code_ids": ["CODE_" + "A" * 64],
                "limitations": [],
            }
        ],
    }


class CoverageTraceWrapperTests(unittest.TestCase):
    def test_attempt_identity_is_hashed_and_profiles_use_module_and_pid(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            trace_root = Path(directory).resolve()
            (trace_root / "requested-tests.json").write_text(
                """{
  "schema_version": 2,
  "tests": [
    {
      "discovery_id": "DISC_00000000000000000001",
      "nextest_binary_id": "trust-runtime::lib",
      "nextest_binary_path": "/repo/target/test-binary",
      "nextest_test_name": "module::test with spaces"
    }
  ]
}
"""
            )
            child_env, attempt_dir, manifest = prepare_attempt_environment(
                {
                    "TRUST_CATALOG_TRACE_ROOT": str(trace_root),
                    "NEXTEST_TEST_NAME": "module::test with spaces",
                    "NEXTEST_BINARY_ID": "trust-runtime::lib",
                    "NEXTEST_ATTEMPT_ID": "trust-runtime::lib$module::test#1",
                    "PATH": "/usr/bin",
                },
                command=["/repo/target/test-binary", "--exact", "module::test"],
            )

            self.assertEqual(trace_root / "attempts", attempt_dir.parent)
            self.assertRegex(attempt_dir.name, r"^[0-9a-f]{64}$")
            self.assertEqual(
                str(attempt_dir / "profiles" / "%m-%p.profraw"),
                child_env["LLVM_PROFILE_FILE"],
            )
            self.assertEqual("module::test with spaces", manifest["test_name"])
            self.assertEqual("trust-runtime::lib", manifest["binary_id"])
            self.assertEqual(
                "trust-runtime::lib$module::test#1",
                manifest["attempt_id"],
            )
            self.assertEqual("/usr/bin", child_env["PATH"])
            self.assertEqual(
                "/repo/target/test-binary", manifest["test_executable"]
            )
            self.assertEqual(
                "DISC_00000000000000000001", manifest["discovery_id"]
            )
            self.assertEqual(
                ["--exact", "module::test"], manifest["test_arguments"]
            )
            self.assertEqual("started", manifest["status"])

            completed = complete_attempt_manifest(
                attempt_dir,
                manifest,
                exit_code=0,
                duration_ms=17,
            )

            self.assertEqual("completed", completed["status"])
            self.assertEqual(0, completed["exit_code"])
            self.assertEqual(17, completed["duration_ms"])
            self.assertEqual(
                completed,
                json.loads((attempt_dir / "attempt.json").read_text()),
            )

    def test_missing_nextest_identity_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "NEXTEST_TEST_NAME"):
            prepare_attempt_environment(
                {
                    "TRUST_CATALOG_TRACE_ROOT": "/tmp/trace",
                    "NEXTEST_BINARY_ID": "binary",
                    "NEXTEST_ATTEMPT_ID": "attempt",
                }
            )

    def test_unrequested_runtime_identity_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            trace_root = Path(directory).resolve()
            (trace_root / "requested-tests.json").write_text(
                '{"schema_version":2,"tests":[]}'
            )
            with self.assertRaisesRegex(ValueError, "not in requested-tests"):
                prepare_attempt_environment(
                    {
                        "TRUST_CATALOG_TRACE_ROOT": str(trace_root),
                        "NEXTEST_TEST_NAME": "unrequested",
                        "NEXTEST_BINARY_ID": "binary",
                        "NEXTEST_ATTEMPT_ID": "attempt",
                    },
                    command=["/tmp/test-binary"],
                )

    def test_runtime_executable_must_match_requested_binary_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            trace_root = Path(directory).resolve()
            (trace_root / "requested-tests.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "tests": [
                            {
                                "discovery_id": DISCOVERY_ID,
                                "nextest_binary_id": "binary",
                                "nextest_binary_path": "/current/test-binary",
                                "nextest_test_name": "module::test",
                            }
                        ],
                    }
                )
            )

            with self.assertRaisesRegex(ValueError, "not in requested-tests"):
                prepare_attempt_environment(
                    {
                        "TRUST_CATALOG_TRACE_ROOT": str(trace_root),
                        "NEXTEST_TEST_NAME": "module::test",
                        "NEXTEST_BINARY_ID": "binary",
                        "NEXTEST_ATTEMPT_ID": "attempt",
                    },
                    command=["/stale/test-binary"],
                )


class CoverageTraceContractTests(unittest.TestCase):
    def test_valid_focused_observation_is_accepted(self) -> None:
        self.assertEqual(
            [],
            validate_trace_document(
                _trace(),
                live_facts={DISCOVERY_ID: _fact()},
            ),
        )

    def test_full_campaign_requires_explicit_final_authority(self) -> None:
        failures = validate_trace_document(
            _trace("full_campaign"),
            live_facts={DISCOVERY_ID: _fact()},
        )

        self.assertTrue(any("full campaign is not authorized" in item for item in failures))

    def test_trace_identity_and_requested_denominator_are_exact(self) -> None:
        document = _trace()
        document["tests"][0]["name"] = "stale"
        document["scope"]["requested_discovery_ids"].append(
            "DISC_00000000000000000002"
        )

        failures = validate_trace_document(
            document,
            live_facts={DISCOVERY_ID: _fact()},
        )

        self.assertTrue(any("name does not match live fact" in item for item in failures))
        self.assertTrue(any("requested discovery IDs" in item for item in failures))

    def test_non_observed_record_cannot_claim_covered_code(self) -> None:
        document = _trace()
        record = document["tests"][0]
        record["trace_status"] = "unsupported"
        record["attribution_kind"] = "unavailable"
        record["command"] = "unavailable"
        record["command_authority"] = "unavailable"

        failures = validate_trace_document(
            document,
            live_facts={DISCOVERY_ID: _fact()},
        )

        self.assertTrue(any("non-observed record cannot claim" in item for item in failures))

    def test_observed_record_requires_successful_exact_attempt(self) -> None:
        document = _trace()
        record = document["tests"][0]
        record["exit_code"] = 1
        record["command_authority"] = "conservative"

        failures = validate_trace_document(
            document,
            live_facts={DISCOVERY_ID: _fact()},
        )

        self.assertTrue(any("observed record requires exit_code 0" in item for item in failures))
        self.assertTrue(any("exact or verified command" in item for item in failures))

    def test_istanbul_observation_requires_typescript_inventory_digest(self) -> None:
        document = _trace()
        document["tests"][0]["attribution_kind"] = "istanbul_source_coverage"

        failures = validate_trace_document(
            document,
            live_facts={DISCOVERY_ID: _fact()},
        )

        self.assertEqual(
            [
                f"{DISCOVERY_ID} Istanbul attribution requires "
                "typescript_product inventory digest"
            ],
            failures,
        )


if __name__ == "__main__":
    unittest.main()
