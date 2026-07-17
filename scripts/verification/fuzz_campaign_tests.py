"""Tests for bounded fuzz-campaign results and crash handoff."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.fuzz_campaign_contract import validate_campaign_payload


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


class FuzzCampaignContractTests(unittest.TestCase):
    def test_complete_zero_crash_campaign_is_accepted(self) -> None:
        self.assertEqual(
            [],
            validate_campaign_payload(
                fixture_payload(),
                program=fixture_program(),
                tests={},
            ),
        )

    def test_missing_target_and_infrastructure_failure_are_rejected(self) -> None:
        missing = fixture_payload()
        missing["results"] = missing["results"][:-1]
        self.assertTrue(
            any(
                "exactly match registered targets" in failure
                for failure in validate_campaign_payload(
                    missing, program=fixture_program(), tests={}
                )
            )
        )

        failed = fixture_payload()
        failed["results"][0]["exit_status"] = 1
        self.assertTrue(
            any(
                "infrastructure failure" in failure
                for failure in validate_campaign_payload(
                    failed, program=fixture_program(), tests={}
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
        tests = {
            "TEST_CRASH_ONE": {
                "id": "TEST_CRASH_ONE",
                "status": "mapped",
                "command": "cargo test -p crate crash_one",
            }
        }
        self.assertEqual(
            [],
            validate_campaign_payload(payload, program=fixture_program(), tests=tests),
        )

    def test_command_and_summary_tampering_fail_closed(self) -> None:
        command = fixture_payload()
        command["results"][0]["command"] += " -ignore_crashes=1"
        self.assertTrue(
            any(
                "command does not match" in failure
                for failure in validate_campaign_payload(
                    command, program=fixture_program(), tests={}
                )
            )
        )

        summary = copy.deepcopy(fixture_payload())
        summary["summary"]["crash_artifacts"] = 1
        self.assertTrue(
            any(
                "summary does not match" in failure
                for failure in validate_campaign_payload(
                    summary, program=fixture_program(), tests={}
                )
            )
        )


if __name__ == "__main__":
    unittest.main()
