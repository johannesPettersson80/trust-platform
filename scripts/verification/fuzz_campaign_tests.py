"""Tests for bounded fuzz-campaign results and crash handoff."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.fuzz_campaign_contract import validate_campaign_payload
from scripts.verification.fuzz_crash_regressions import (
    campaign_regressions,
    validate_crash_registry,
)


def fixture_program() -> dict:
    return {
        "targets": [
            {
                "id": "FUZZ_TARGET_ONE",
                "target_kind": "cargo_fuzz",
                "name": "one",
                "command": "cd fuzz && cargo fuzz run one",
            },
            {
                "id": "FUZZ_SMOKE_TWO",
                "target_kind": "bounded_rust_smoke",
                "name": "two",
                "command": "cargo test -p crate two",
            },
        ]
    }


def fixture_payload() -> dict:
    results = [
        {
            "target_id": "FUZZ_TARGET_ONE",
            "target_kind": "cargo_fuzz",
            "command": "cargo +nightly fuzz run one -- -runs=10000 -max_total_time=120 -timeout=10 -max_len=65536",
            "exit_status": 0,
            "timed_out": False,
            "executions": 10000,
            "log_sha256": "sha256:" + "a" * 64,
            "artifact_files": [],
        },
        {
            "target_id": "FUZZ_SMOKE_TWO",
            "target_kind": "bounded_rust_smoke",
            "command": "cargo test -p crate two",
            "exit_status": 0,
            "timed_out": False,
            "executions": 1,
            "log_sha256": "sha256:" + "b" * 64,
            "artifact_files": [],
        },
    ]
    return {
        "schema_version": 1,
        "generator": "bounded-fuzz-campaign",
        "generator_version": 1,
        "source_commit": "c" * 40,
        "started_at": "2026-07-17T12:00:00+00:00",
        "finished_at": "2026-07-17T12:05:00+00:00",
        "platform": "linux-x86_64",
        "requested_runs": 10000,
        "max_total_time_seconds": 120,
        "timeout_seconds": 10,
        "results": results,
        "regressions": [],
        "summary": {
            "targets": 2,
            "passed": 2,
            "infrastructure_failures": 0,
            "crash_artifacts": 0,
            "regressions": 0,
        },
    }


def fixture_registry() -> dict:
    return {
        "schema_version": 1,
        "id": "FUZZ_CRASH_REGRESSIONS_V1",
        "status": "mapped",
        "required_disposition": "deterministic_regression",
        "regressions": [],
    }


class FuzzCampaignContractTests(unittest.TestCase):
    def test_complete_zero_crash_campaign_is_accepted(self) -> None:
        self.assertEqual(
            [],
            validate_campaign_payload(
                fixture_payload(),
                program=fixture_program(),
                tests={},
                regression_registry=fixture_registry(),
            ),
        )

    def test_missing_target_and_infrastructure_failure_are_rejected(self) -> None:
        missing = fixture_payload()
        missing["results"] = missing["results"][:-1]
        self.assertTrue(
            any(
                "exactly match registered targets" in failure
                for failure in validate_campaign_payload(
                    missing,
                    program=fixture_program(),
                    tests={},
                    regression_registry=fixture_registry(),
                )
            )
        )

        failed = fixture_payload()
        failed["results"][0]["exit_status"] = 1
        self.assertTrue(
            any(
                "infrastructure failure" in failure
                for failure in validate_campaign_payload(
                    failed,
                    program=fixture_program(),
                    tests={},
                    regression_registry=fixture_registry(),
                )
            )
        )

    def test_every_crash_requires_a_mapped_deterministic_regression(self) -> None:
        payload = fixture_payload()
        payload["results"][0]["exit_status"] = 77
        payload["results"][0]["artifact_files"] = [
            {
                "path": "fuzz/artifacts/one/crash-deadbeef",
                "sha256": "sha256:" + "d" * 64,
                "size": 4,
            }
        ]
        payload["summary"].update(
            passed=1,
            crash_artifacts=1,
        )
        failures = validate_campaign_payload(
            payload,
            program=fixture_program(),
            tests={},
            regression_registry=fixture_registry(),
        )
        self.assertTrue(any("deterministic regression" in failure for failure in failures))

        payload["regressions"] = [
            {
                "target_id": "FUZZ_TARGET_ONE",
                "artifact_sha256": "sha256:" + "d" * 64,
                "test_id": "TEST_CRASH_ONE",
                "rationale": "The minimized input is committed as a deterministic test fixture.",
            }
        ]
        payload["summary"]["regressions"] = 1
        registry = fixture_registry()
        registry["regressions"] = copy.deepcopy(payload["regressions"])
        tests = {
            "TEST_CRASH_ONE": {
                "id": "TEST_CRASH_ONE",
                "status": "mapped",
                "command": "cargo test -p crate crash_one",
            }
        }
        self.assertEqual(
            [],
            validate_campaign_payload(
                payload,
                program=fixture_program(),
                tests=tests,
                regression_registry=registry,
            ),
        )

    def test_command_and_summary_tampering_fail_closed(self) -> None:
        command = fixture_payload()
        command["results"][0]["command"] += " -ignore_crashes=1"
        self.assertTrue(
            any(
                "command does not match" in failure
                for failure in validate_campaign_payload(
                    command,
                    program=fixture_program(),
                    tests={},
                    regression_registry=fixture_registry(),
                )
            )
        )

        summary = copy.deepcopy(fixture_payload())
        summary["summary"]["crash_artifacts"] = 1
        self.assertTrue(
            any(
                "summary does not match" in failure
                for failure in validate_campaign_payload(
                    summary,
                    program=fixture_program(),
                    tests={},
                    regression_registry=fixture_registry(),
                )
            )
        )

    def test_registry_is_closed_and_requires_live_mapped_regressions(self) -> None:
        registry = fixture_registry()
        self.assertEqual(
            [], validate_crash_registry(registry, program=fixture_program(), tests={})
        )

        registry["unexpected"] = True
        self.assertTrue(
            any(
                "registry fields" in failure
                for failure in validate_crash_registry(
                    registry, program=fixture_program(), tests={}
                )
            )
        )
        registry.pop("unexpected")
        registry["regressions"] = [
            {
                "target_id": "FUZZ_TARGET_ONE",
                "artifact_sha256": "sha256:" + "d" * 64,
                "test_id": "TEST_CRASH_ONE",
                "rationale": "A minimized input is committed as a deterministic test.",
            }
        ]
        failures = validate_crash_registry(
            registry, program=fixture_program(), tests={}
        )
        self.assertTrue(any("is not a mapped test" in failure for failure in failures))

    def test_campaign_uses_only_committed_registry_rows(self) -> None:
        payload = fixture_payload()
        payload["results"][0]["exit_status"] = 77
        payload["results"][0]["artifact_files"] = [
            {
                "path": "fuzz/artifacts/one/crash-deadbeef",
                "sha256": "sha256:" + "d" * 64,
                "size": 4,
            }
        ]
        registry = fixture_registry()
        registry["regressions"] = [
            {
                "target_id": "FUZZ_TARGET_ONE",
                "artifact_sha256": "sha256:" + "d" * 64,
                "test_id": "TEST_CRASH_ONE",
                "rationale": "A minimized input is committed as a deterministic test.",
            },
            {
                "target_id": "FUZZ_SMOKE_TWO",
                "artifact_sha256": "sha256:" + "e" * 64,
                "test_id": "TEST_OLD_CRASH",
                "rationale": "A historical minimized input remains a deterministic test.",
            },
        ]
        self.assertEqual(
            registry["regressions"][:1],
            campaign_regressions(registry, payload["results"]),
        )

        payload["regressions"] = [
            {
                **registry["regressions"][0],
                "test_id": "TEST_INVENTED_MAPPING",
            }
        ]
        payload["summary"].update(
            passed=1,
            crash_artifacts=1,
            regressions=1,
        )
        failures = validate_campaign_payload(
            payload,
            program=fixture_program(),
            tests={
                "TEST_INVENTED_MAPPING": {
                    "status": "mapped",
                    "command": "cargo test invented",
                }
            },
            regression_registry=registry,
        )
        self.assertTrue(
            any("do not match committed registry" in failure for failure in failures)
        )


if __name__ == "__main__":
    unittest.main()
