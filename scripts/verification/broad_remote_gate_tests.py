"""Tests for producer-authentic broad remote gate evidence."""

from __future__ import annotations

import hashlib
import json
import re
import tomllib
import unittest
from pathlib import Path

from scripts.verification.broad_remote_gate import (
    PRODUCER,
    REVIEWED_GATE_COMMAND,
    BroadRemoteGateError,
    BroadRemoteGateProducer,
    CommandResult,
)
from scripts.verification.broad_remote_artifacts import REMOTE_ARTIFACT_DIR
from scripts.verification.metadata_validator.broad_remote_gate_evidence import (
    validate_broad_remote_gate_evidence,
)
from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator


FULL_SHA = "a" * 40
CASE_FILE = "verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml"
CASE_BYTES = (ROOT / CASE_FILE).read_bytes()
CASE_FILE_DIGEST = "sha256:" + hashlib.sha256(CASE_BYTES).hexdigest()
CASE_DATA = tomllib.loads(CASE_BYTES.decode())
CASE_IDS = [case["id"] for case in CASE_DATA["case"]]


class FakeExecutor:
    def __init__(
        self,
        *,
        gate_exit: int = 0,
        selected_exit: int = 0,
        dirty_remote: bool = False,
        home_available_kib: int = 70 * 1024 * 1024,
        tmp_available_kib: int = 8 * 1024 * 1024,
        stale_artifact: bool = False,
    ) -> None:
        self.gate_exit = gate_exit
        self.selected_exit = selected_exit
        self.dirty_remote = dirty_remote
        self.home_available_kib = home_available_kib
        self.tmp_available_kib = tmp_available_kib
        self.stale_artifact = stale_artifact
        self.calls: list[tuple[tuple[str, ...], bool]] = []
        self.status_calls = 0
        self.run_ids: dict[str, str] = {}

    def __call__(self, argv: tuple[str, ...], *, capture: bool) -> CommandResult:
        self.calls.append((argv, capture))
        command = argv[-1]
        if command.endswith("git status --porcelain --untracked-files=all"):
            self.status_calls += 1
            dirty = self.dirty_remote and self.status_calls == 1
            return CommandResult(0, "dirty\n" if dirty else "", "")
        if command.endswith("git rev-parse --verify 'HEAD^{commit}'"):
            return CommandResult(0, f"{FULL_SHA}\n", "")
        if command == "uname -s":
            return CommandResult(0, "Linux\n", "")
        if command == "uname -m":
            return CommandResult(0, "x86_64\n", "")
        if command.startswith("df -hT /home/johannes /tmp"):
            return CommandResult(0, "disk audit\n", "")
        if command == "df -Pk /home/johannes":
            return CommandResult(0, _df_output(self.home_available_kib), "")
        if command == "df -Pk /tmp":
            return CommandResult(0, _df_output(self.tmp_available_kib), "")
        if command.endswith("just fmt && just clippy && just test-all"):
            return CommandResult(self.gate_exit, "", "gate failed")
        if "TRUST_VERIFY_TEST_ID=" in command:
            test_id = _assignment(command, "TRUST_VERIFY_TEST_ID")
            self.run_ids[test_id] = _assignment(command, "TRUST_VERIFY_RUN_ID")
            return CommandResult(self.selected_exit, "", "selected test failed")
        if "cat -- " in command:
            test_id = Path(command.rsplit(" ", 1)[-1]).stem
            run_id = "stale-run" if self.stale_artifact else self.run_ids[test_id]
            return CommandResult(0, _case_artifact(test_id, run_id), "")
        if "rm -f -- " in command:
            return CommandResult(0, "", "")
        raise AssertionError(f"unexpected command: {argv}")


class BroadRemoteGateProducerTests(unittest.TestCase):
    def test_success_executes_exact_gate_and_builds_bound_record(self) -> None:
        executor = FakeExecutor()
        written: list[dict[str, object]] = []
        clock = iter(
            (
                "2026-07-12T12:00:00Z",
                "2026-07-12T12:10:00Z",
            )
        )
        monotonic = iter((100.0, 700.25))
        producer = producer_fixture(
            executor=executor,
            evidence_writer=lambda **kwargs: written.append(kwargs["record"])
            or kwargs["evidence_index_path"],
            utc_now=lambda: next(clock),
            monotonic=lambda: next(monotonic),
        )

        result = producer.run(["INV"])

        self.assertEqual(len(written), 1)
        self.assertEqual(result.record, written[0])
        self.assertEqual(result.record["producer"], PRODUCER)
        self.assertEqual(result.record["command"], REVIEWED_GATE_COMMAND)
        self.assertEqual(result.record["command_exit_status"], 0)
        self.assertEqual(result.record["gate_duration_milliseconds"], 600250)
        self.assertEqual(result.record["platform"], "trust-builder-linux-x86_64")
        self.assertEqual(result.record["commit"], FULL_SHA)
        self.assertEqual(result.record["remote_commit"], FULL_SHA)
        self.assertEqual(result.record["linked_invariants"], ["INV"])
        self.assertEqual(result.record["linked_tests"], ["TEST_A", "TEST_B"])
        executed = result.record["executed_tests"]
        self.assertEqual([entry["test_id"] for entry in executed], ["TEST_A", "TEST_B"])
        for entry in executed:
            test_id = entry["test_id"]
            self.assertEqual(entry["discovery_id"], f"DISC_{test_id}")
            self.assertEqual(entry["command"], test_command(test_id))
            self.assertEqual(entry["case_file_digest"], CASE_FILE_DIGEST)
            self.assertTrue(entry["per_case_summary"])
            self.assertTrue(all(item.endswith(":passed") for item in entry["per_case_summary"]))
            self.assertRegex(entry["case_artifact_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(result.evidence_path, ROOT / "verification/evidence-index.toml")
        gate_calls = [call for call in executor.calls if call[0][-1].endswith("just test-all")]
        self.assertEqual(len(gate_calls), 1)
        self.assertFalse(gate_calls[0][1])
        selected_calls = [
            call
            for call in executor.calls
            if "TRUST_VERIFY_TEST_ID=" in call[0][-1]
        ]
        self.assertEqual(len(selected_calls), 2)
        self.assertTrue(all(not call[1] for call in selected_calls))
        self.assertEqual(executor.status_calls, 2)

    def test_feature_gated_catalog_command_is_executed_verbatim(self) -> None:
        command = (
            "cargo test -p trust-runtime --features opcua --test timer "
            "TEST_A -- --exact"
        )
        tests = {
            "TEST_A": dict(test_record("TEST_A"), command=command),
        }
        producer = producer_fixture(
            executor=FakeExecutor(),
            tests=tests,
            invariants={"INV": {"id": "INV", "area": "runtime_safety", "tests": ["TEST_A"]}},
        )

        result = producer.run(["INV"])

        self.assertEqual(result.record["executed_tests"][0]["command"], command)
        self.assertTrue(
            any(call[0][-1].endswith(" " + command) for call in producer.executor.calls)
        )

    def test_failed_selected_catalog_command_writes_no_evidence(self) -> None:
        executor = FakeExecutor(selected_exit=17)
        written: list[dict[str, object]] = []
        producer = producer_fixture(
            executor=executor,
            evidence_writer=lambda **kwargs: written.append(kwargs["record"]),
        )

        with self.assertRaisesRegex(BroadRemoteGateError, "selected catalog command"):
            producer.run(["INV"])

        self.assertEqual(written, [])
        self.assertEqual(executor.status_calls, 2)

    def test_stale_case_artifact_writes_no_evidence(self) -> None:
        executor = FakeExecutor(stale_artifact=True)
        written: list[dict[str, object]] = []
        producer = producer_fixture(
            executor=executor,
            evidence_writer=lambda **kwargs: written.append(kwargs["record"]),
        )

        with self.assertRaisesRegex(BroadRemoteGateError, "TRUST_VERIFY_RUN_ID mismatch"):
            producer.run(["INV"])

        self.assertEqual(written, [])
        self.assertEqual(executor.status_calls, 2)

    def test_unapproved_producer_fails_before_remote_execution(self) -> None:
        executor = FakeExecutor()
        producer = producer_fixture(executor=executor, approved=[])

        with self.assertRaisesRegex(BroadRemoteGateError, "does not allowlist"):
            producer.run(["INV"])

        self.assertEqual(executor.calls, [])

    def test_link_set_must_be_nonempty_and_is_derived_from_invariants(self) -> None:
        producer = producer_fixture(
            executor=FakeExecutor(),
            invariants={"INV": {"id": "INV", "tests": []}},
        )

        with self.assertRaisesRegex(BroadRemoteGateError, "has no linked tests"):
            producer.run(["INV"])

    def test_ignored_or_non_rust_link_is_rejected_before_remote_execution(self) -> None:
        ignored_executor = FakeExecutor()
        ignored = producer_fixture(
            executor=ignored_executor,
            ignored_tests={"IGNORED": {"discovery_id": "DISC_TEST_A"}},
        )
        with self.assertRaisesRegex(BroadRemoteGateError, "ignored by the broad gate"):
            ignored.run(["INV"])
        self.assertEqual(ignored_executor.calls, [])

        non_rust_executor = FakeExecutor()
        tests = {
            "TEST_A": dict(test_record("TEST_A"), discovery_source_kind="vscode_test"),
            "TEST_B": test_record("TEST_B"),
        }
        non_rust = producer_fixture(executor=non_rust_executor, tests=tests)
        with self.assertRaisesRegex(BroadRemoteGateError, "outside the reviewed Rust gate"):
            non_rust.run(["INV"])
        self.assertEqual(non_rust_executor.calls, [])

        hostile_tiers_executor = FakeExecutor()
        hostile_tiers = producer_fixture(
            executor=hostile_tiers_executor,
            tests={
                "TEST_A": dict(test_record("TEST_A"), suite_tiers=7),
                "TEST_B": test_record("TEST_B"),
            },
        )
        with self.assertRaisesRegex(BroadRemoteGateError, "not assigned to suite pr"):
            hostile_tiers.run(["INV"])
        self.assertEqual(hostile_tiers_executor.calls, [])

    def test_dirty_or_wrong_remote_revision_fails_before_gate(self) -> None:
        executor = FakeExecutor(dirty_remote=True)
        producer = producer_fixture(executor=executor)

        with self.assertRaisesRegex(BroadRemoteGateError, "remote worktree is dirty"):
            producer.run(["INV"])

        self.assertFalse(any(call[0][-1].endswith("just test-all") for call in executor.calls))

    def test_insufficient_remote_disk_fails_before_gate(self) -> None:
        executor = FakeExecutor(home_available_kib=59 * 1024 * 1024)
        producer = producer_fixture(executor=executor)

        with self.assertRaisesRegex(BroadRemoteGateError, "insufficient free space"):
            producer.run(["INV"])

        self.assertFalse(any(call[0][-1].endswith("just test-all") for call in executor.calls))

    def test_failed_gate_still_checks_remote_and_local_postconditions(self) -> None:
        executor = FakeExecutor(gate_exit=9)
        revisions = iter((FULL_SHA, FULL_SHA))
        written: list[dict[str, object]] = []
        producer = producer_fixture(
            executor=executor,
            revision_provider=lambda: next(revisions),
            evidence_writer=lambda **kwargs: written.append(kwargs["record"]),
        )

        with self.assertRaisesRegex(BroadRemoteGateError, "exit status 9"):
            producer.run(["INV"])

        self.assertEqual(executor.status_calls, 2)
        self.assertEqual(written, [])

    def test_local_revision_change_after_gate_prevents_evidence(self) -> None:
        executor = FakeExecutor()
        revisions = iter((FULL_SHA, "b" * 40))
        written: list[dict[str, object]] = []
        producer = producer_fixture(
            executor=executor,
            revision_provider=lambda: next(revisions),
            evidence_writer=lambda **kwargs: written.append(kwargs["record"]),
        )

        with self.assertRaisesRegex(BroadRemoteGateError, "changed during execution"):
            producer.run(["INV"])

        self.assertEqual(written, [])


class BroadRemoteGateEvidenceContractTests(unittest.TestCase):
    def test_evidence_schema_pins_remote_revision_timing_and_clean_flags(self) -> None:
        schema = json.loads((ROOT / "verification/schemas/evidence.schema.json").read_text())
        properties = schema["properties"]

        self.assertEqual(properties["remote_commit"]["pattern"], "^[0-9a-f]{40}$")
        self.assertEqual(properties["gate_duration_milliseconds"], {"type": "integer", "minimum": 0})
        for field in (
            "local_source_clean_before",
            "local_source_clean_after",
            "remote_source_clean_before",
            "remote_source_clean_after",
        ):
            self.assertEqual(properties[field], {"const": True})
        executed = properties["executed_tests"]
        self.assertFalse(executed["items"]["additionalProperties"])
        self.assertEqual(executed["items"]["properties"]["exit_status"], {"const": 0})

    def test_semantic_contract_rejects_command_timing_and_link_tampering(self) -> None:
        record = evidence_record()
        self.assertEqual(contract_failures(record), [])

        tampered = dict(record, command="just test-all")
        self.assertIn("reviewed command", "\n".join(contract_failures(tampered)))

        tampered = dict(record, gate_duration_milliseconds=-1)
        self.assertIn("non-negative integer duration", "\n".join(contract_failures(tampered)))

        tampered = dict(record, gate_duration_milliseconds=1)
        self.assertIn("duration does not match", "\n".join(contract_failures(tampered)))

        tampered = dict(record, date="2026-07-11", last_reviewed="2026-07-11")
        self.assertIn("date must match", "\n".join(contract_failures(tampered)))

        tampered = dict(record, home_available_kib=1)
        self.assertIn("reviewed minimum", "\n".join(contract_failures(tampered)))

        tampered = dict(record, linked_tests=["TEST_A"])
        self.assertIn("exactly match executed_tests", "\n".join(contract_failures(tampered)))

        tampered = dict(record)
        tampered["executed_tests"] = [dict(record["executed_tests"][0], exit_status=1)]
        self.assertIn("exit_status must equal 0", "\n".join(contract_failures(tampered)))

        tampered = dict(record)
        tampered["executed_tests"] = [
            dict(
                record["executed_tests"][0],
                per_case_summary=["CASE_ONE:passed", "CASE_ONE:passed"],
            ),
            record["executed_tests"][1],
        ]
        self.assertIn("duplicate case ids", "\n".join(contract_failures(tampered)))

        tampered = dict(record, invented="field")
        self.assertIn("fields drifted", "\n".join(contract_failures(tampered)))

    def test_full_validator_wiring_rejects_corrupt_producer_record(self) -> None:
        validator = Validator()
        validator.load_records()
        fixture = live_evidence_record(validator)
        fixture["command"] = "just test-all"
        validator.evidence[fixture["id"]] = fixture

        validator.validate()

        self.assertTrue(
            any("must use the reviewed command" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_historical_record_survives_later_invariant_test_addition(self) -> None:
        record = evidence_record()
        failures = contract_failures(
            record,
            invariant_tests=["TEST_A", "TEST_B", "TEST_LATER"],
        )

        self.assertEqual(failures, [])


def producer_fixture(
    *,
    executor: FakeExecutor,
    approved: list[str] | None = None,
    invariants: dict[str, dict[str, object]] | None = None,
    tests: dict[str, dict[str, object]] | None = None,
    ignored_tests: dict[str, dict[str, object]] | None = None,
    evidence_writer=None,
    revision_provider=None,
    utc_now=None,
    monotonic=None,
) -> BroadRemoteGateProducer:
    return BroadRemoteGateProducer(
        root=ROOT,
        tests=tests
        or {
            "TEST_A": test_record("TEST_A"),
            "TEST_B": test_record("TEST_B"),
        },
        ignored_tests=ignored_tests or {},
        invariants=invariants
        or {
            "INV": {
                "id": "INV",
                "area": "runtime_safety",
                "tests": ["TEST_B", "TEST_A"],
            }
        },
        suites={"pr": {"approved_proof_producers": approved if approved is not None else [PRODUCER]}},
        executor=executor,
        evidence_writer=evidence_writer or (lambda **kwargs: kwargs["evidence_index_path"]),
        revision_provider=revision_provider or (lambda: FULL_SHA),
        utc_now=utc_now,
        monotonic=monotonic,
        validate_metadata=False,
    )


def evidence_record() -> dict[str, object]:
    return {
        "schema_version": 1,
        "id": "EVID_BROAD_REMOTE_PR_20260712_AAAAAAAAAAAA",
        "title": "Reviewed PR broad gate for INV",
        "area": "bytecode_vm",
        "owner": "verification",
        "status": "mapped",
        "kind": "committed_file",
        "path": "verification/evidence-index.toml",
        "command": REVIEWED_GATE_COMMAND,
        "commit": FULL_SHA,
        "remote_commit": FULL_SHA,
        "platform": "trust-builder-linux-x86_64",
        "date": "2026-07-12",
        "suite_id": "pr",
        "producer": PRODUCER,
        "generated_report_version": "broad-remote-gate-v1",
        "linked_invariants": ["INV"],
        "linked_tests": ["TEST_A", "TEST_B"],
        "linked_spec_gaps": [],
        "last_reviewed": "2026-07-12",
        "proof_kind": "none",
        "proof_scope": "broad_remote_gate",
        "command_exit_status": 0,
        "executed_tests": [
            executed_test("TEST_A"),
            executed_test("TEST_B"),
        ],
        "gate_started_at": "2026-07-12T12:00:00Z",
        "gate_finished_at": "2026-07-12T12:10:00Z",
        "gate_duration_milliseconds": 600000,
        "local_source_clean_before": True,
        "local_source_clean_after": True,
        "remote_source_clean_before": True,
        "remote_source_clean_after": True,
        "disk_preflight_passed": True,
        "home_available_kib": 70 * 1024 * 1024,
        "tmp_available_kib": 8 * 1024 * 1024,
    }


def contract_failures(
    record: dict[str, object],
    *,
    invariant_tests: list[str] | None = None,
) -> list[str]:
    failures: list[str] = []
    validate_broad_remote_gate_evidence(
        fail=lambda _path, message: failures.append(message),
        path=Path("verification/evidence-index.toml"),
        record=record,
        invariants={
            "INV": {
                "id": "INV",
                "area": "bytecode_vm",
                "tests": invariant_tests or ["TEST_A", "TEST_B"],
            }
        },
        tests={
            "TEST_A": test_record("TEST_A"),
            "TEST_B": test_record("TEST_B"),
        },
        ignored_tests={},
    )
    return failures


def test_record(test_id: str) -> dict[str, object]:
    return {
        "id": test_id,
        "status": "mapped",
        "discovery_id": f"DISC_{test_id}",
        "discovery_source_kind": "rust_integration_test",
        "suite_tiers": ["pr"],
        "command": test_command(test_id),
        "case_file": CASE_FILE,
        "case_file_digest": CASE_FILE_DIGEST,
    }


def test_command(test_id: str) -> str:
    return f"cargo test -p trust-runtime --test timer {test_id} -- --exact"


def executed_test(test_id: str) -> dict[str, object]:
    return {
        "test_id": test_id,
        "discovery_id": f"DISC_{test_id}",
        "discovery_source_kind": "rust_integration_test",
        "command": test_command(test_id),
        "run_id": f"run-{test_id}",
        "case_file_digest": CASE_FILE_DIGEST,
        "case_artifact_digest": "sha256:" + "b" * 64,
        "per_case_summary": [f"{case_id}:passed" for case_id in CASE_IDS],
        "exit_status": 0,
    }


def _df_output(available_kib: int) -> str:
    return (
        "Filesystem 1024-blocks Used Available Capacity Mounted on\n"
        f"/dev/root 100000000 1 {available_kib} 1% /\n"
    )


def _assignment(command: str, name: str) -> str:
    match = re.search(rf"(?:^| ){name}=([^ ]+)", command)
    if match is None:
        raise AssertionError(f"missing {name} in {command}")
    return match.group(1).strip("'\"")


def _case_artifact(test_id: str, run_id: str) -> str:
    cases = [
        {
            "id": case_id,
            "family": next(case["family"] for case in CASE_DATA["case"] if case["id"] == case_id),
            "result": "passed",
            "spec_gap_ref": None,
            "observed_error": None,
            "observed_status": None,
            "state_delta": "not_applicable",
            "before": None,
            "after": None,
        }
        for case_id in CASE_IDS
    ]
    return json.dumps(
        {
            "schema_version": 1,
            "test_id": test_id,
            "case_file": CASE_FILE,
            "case_file_digest": CASE_FILE_DIGEST,
            "helper_version": "verification-cases v1",
            "case_provenance_kind": "generated_decision_table_v1",
            "trace_definition_digest": None,
            "trust_verify_test_id": test_id,
            "trust_verify_run_id": run_id,
            "trust_verify_case_file_digest": CASE_FILE_DIGEST,
            "trust_verify_artifact_dir": REMOTE_ARTIFACT_DIR,
            "cases": cases,
        },
        sort_keys=True,
    ) + "\n"


def live_evidence_record(validator: Validator) -> dict[str, object]:
    test = validator.tests["TEST_BYTECODE_CONTAINER_INVALID_MAGIC"]
    invariant = validator.invariants[test["invariants"][0]]
    record = evidence_record()
    record["id"] = "EVID_BROAD_REMOTE_FULL_VALIDATOR_FIXTURE"
    record["area"] = invariant["area"]
    record["linked_invariants"] = [invariant["id"]]
    record["linked_tests"] = [test["id"]]
    record["executed_tests"] = [
        {
            **executed_test(test["id"]),
            "discovery_id": test["discovery_id"],
            "discovery_source_kind": test["discovery_source_kind"],
            "command": test["command"],
        }
    ]
    record["title"] = "Reviewed PR broad gate for " + invariant["id"]
    record["_path"] = ROOT / "verification/evidence-index.toml"
    return record


if __name__ == "__main__":
    unittest.main()
