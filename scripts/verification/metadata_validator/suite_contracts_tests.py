"""Tests for Phase 5 suite command contracts."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.metadata_validator.suites import validate_suite_records


SUITE_PATH = Path("verification/suites/test.toml")


def inventory_record(
    inventory_id: str = "GATE_TEST",
    *,
    suite_id: str = "veryquick",
    command: str = "just test-fast",
    source_kind: str = "gate_script",
    environment: str = "trust_builder",
    artifact_kind: str = "machine_local",
    artifact_paths: list[str] | None = None,
    artifact_retention: str = "machine_local",
    required_env: list[str] | None = None,
    command_role: str = "entrypoint",
) -> dict:
    return {
        "id": inventory_id,
        "command": command,
        "owner": "verification",
        "duration_class": "fast",
        "environment": environment,
        "required_env": required_env or [],
        "command_role": command_role,
        "artifact_kind": artifact_kind,
        "artifact_paths": artifact_paths if artifact_paths is not None else ["target/gate-artifacts/test-fast/"],
        "artifact_retention": artifact_retention,
        "disposition": "assigned",
        "suite_ids": [suite_id],
        "source_kind": source_kind,
        "path": "scripts/test_gate.sh",
        "name": "test_gate",
    }


def suite(
    suite_id: str = "veryquick",
    *,
    inventory_ids: list[str] | None = None,
    commands: list[str] | None = None,
    command_bindings: list[str] | None = None,
) -> dict:
    ids = inventory_ids if inventory_ids is not None else ["GATE_TEST"]
    command_values = commands if commands is not None else ["just test-fast"]
    return {
        "_path": SUITE_PATH,
        "schema_version": 2,
        "id": suite_id,
        "title": "Test suite",
        "area": "suite",
        "owner": "verification",
        "status": "mapped",
        "last_reviewed": "2026-07-10",
        "purpose": "Test fixture.",
        "duration_class": "fast",
        "environment": "trust_builder",
        "commands": command_values,
        "command_bindings": (
            command_bindings
            if command_bindings is not None
            else ids[: len(command_values)]
        ),
        "inventory_ids": ids,
        "evidence_destination": "target/gate-artifacts/test-fast/",
        "includes": [],
        "excludes": [],
        "approved_proof_producers": [],
    }


def failures(record: dict, inventory: dict[str, dict] | None = None) -> list[str]:
    result: list[str] = []
    validate_suite_records(
        fail=lambda _path, message: result.append(message),
        suites={record["id"]: record},
        inventory=inventory if inventory is not None else {"GATE_TEST": inventory_record()},
    )
    return result


class SuiteContractTests(unittest.TestCase):
    def test_valid_binding_projects_command_from_inventory(self) -> None:
        self.assertEqual(failures(suite()), [])

    def test_commands_are_exact_ordered_inventory_projection(self) -> None:
        record = suite(commands=["just something-else"])

        self.assertIn(
            "commands must equal the ordered command_bindings projection",
            "\n".join(failures(record)),
        )

    def test_suite_inventory_join_is_exhaustive_and_two_way(self) -> None:
        record = suite(inventory_ids=[], commands=[])
        record["command_bindings"] = ["GATE_TEST"]

        joined = "\n".join(failures(record))
        self.assertIn("command binding GATE_TEST is absent from inventory_ids", joined)
        self.assertIn("gate inventory row GATE_TEST assigned to veryquick is not referenced", joined)

    def test_suite_inventory_join_rejects_unknown_ids(self) -> None:
        record = suite(inventory_ids=["GATE_UNKNOWN"], commands=[])

        self.assertIn(
            "inventory_ids references unknown gate inventory id GATE_UNKNOWN",
            "\n".join(failures(record)),
        )

    def test_suite_inventory_join_rejects_known_rows_assigned_elsewhere(self) -> None:
        record = suite(
            inventory_ids=["GATE_OTHER", "GATE_TEST"],
            commands=["just test-fast"],
            command_bindings=["GATE_TEST"],
        )
        inventory = {
            "GATE_TEST": inventory_record(),
            "GATE_OTHER": inventory_record("GATE_OTHER", suite_id="nightly"),
        }

        self.assertIn(
            "inventory_ids row GATE_OTHER is not directly assigned to veryquick",
            "\n".join(failures(record, inventory)),
        )

    def test_command_bindings_exactly_cover_direct_entrypoints(self) -> None:
        inventory = {
            "GATE_ENTRYPOINT": inventory_record("GATE_ENTRYPOINT"),
            "GATE_HELPER": inventory_record(
                "GATE_HELPER",
                command="scripts/helper_gate.sh",
                command_role="helper",
            ),
        }
        valid = suite(
            inventory_ids=["GATE_ENTRYPOINT", "GATE_HELPER"],
            commands=["just test-fast"],
            command_bindings=["GATE_ENTRYPOINT"],
        )
        self.assertEqual(failures(valid, inventory), [])

        missing = suite(
            inventory_ids=["GATE_ENTRYPOINT", "GATE_HELPER"],
            commands=[],
            command_bindings=[],
        )
        self.assertIn(
            "command_bindings must exactly cover directly assigned entrypoint rows; missing=GATE_ENTRYPOINT",
            "\n".join(failures(missing, inventory)),
        )

        helper_bound = suite(
            inventory_ids=["GATE_ENTRYPOINT", "GATE_HELPER"],
            commands=["just test-fast", "scripts/helper_gate.sh"],
            command_bindings=["GATE_ENTRYPOINT", "GATE_HELPER"],
        )
        self.assertIn(
            "command_bindings must exactly cover directly assigned entrypoint rows; extra=GATE_HELPER",
            "\n".join(failures(helper_bound, inventory)),
        )

    def test_milestone_suite_cannot_be_empty_or_placeholder(self) -> None:
        record = suite(inventory_ids=[], commands=[])
        record["placeholder"] = True

        joined = "\n".join(failures(record, {}))
        self.assertIn("milestone suite must configure commands", joined)
        self.assertIn("milestone suite cannot be a placeholder", joined)
        self.assertIn("unexpected fields: placeholder", joined)

    def test_supporting_local_is_the_commandless_exception(self) -> None:
        record = suite("supporting_local", inventory_ids=[], commands=[])
        record["duration_class"] = "manual"
        record["environment"] = "local"

        self.assertEqual(failures(record, {}), [])

    def test_hardware_suite_requires_strict_opt_in(self) -> None:
        item = inventory_record(
            "GATE_HARDWARE",
            suite_id="hardware_lab",
            command="scripts/runtime_device_in_loop_gate.sh",
            environment="github_or_lab_runner",
            artifact_kind="lab_report",
            artifact_paths=["target/gate-artifacts/device-in-the-loop/"],
            artifact_retention="machine_local",
        )
        record = suite(
            "hardware_lab",
            inventory_ids=["GATE_HARDWARE"],
            commands=[item["command"]],
        )
        record["duration_class"] = "lab"
        record["environment"] = "github_or_lab_runner"

        self.assertIn(
            "must require TRUST_DIT_REQUIRE_HARDWARE=1",
            "\n".join(failures(record, {"GATE_HARDWARE": item})),
        )

    def test_hardware_suite_binds_strict_script_and_keeps_skip_capable_workflow_as_helper(self) -> None:
        script = inventory_record(
            "GATE_HARDWARE_SCRIPT",
            suite_id="hardware_lab",
            command="scripts/runtime_device_in_loop_gate.sh",
            environment="github_or_lab_runner",
            artifact_kind="lab_report",
            artifact_paths=["target/gate-artifacts/device-in-the-loop/"],
            artifact_retention="machine_local",
            required_env=["TRUST_DIT_REQUIRE_HARDWARE=1"],
        )
        workflow = inventory_record(
            "GATE_HARDWARE_WORKFLOW",
            suite_id="hardware_lab",
            command="workflow job .github/workflows/protocol-device-in-loop.yml#protocol-device-in-loop",
            source_kind="github_workflow_job",
            environment="github_self_hosted_linux",
            artifact_kind="ci_artifact",
            artifact_paths=["protocol-device-in-loop-results"],
            artifact_retention="repository_default",
            command_role="helper",
        )
        workflow["path"] = ".github/workflows/protocol-device-in-loop.yml"
        inventory = {
            "GATE_HARDWARE_SCRIPT": script,
            "GATE_HARDWARE_WORKFLOW": workflow,
        }
        record = suite(
            "hardware_lab",
            inventory_ids=["GATE_HARDWARE_SCRIPT", "GATE_HARDWARE_WORKFLOW"],
            commands=[script["command"]],
            command_bindings=["GATE_HARDWARE_SCRIPT"],
        )
        record["duration_class"] = "lab"
        record["environment"] = "github_or_lab_runner"

        self.assertEqual(failures(record, inventory), [])

    def test_hardware_opt_in_is_forbidden_outside_hardware_suite(self) -> None:
        item = inventory_record(required_env=["TRUST_DIT_REQUIRE_HARDWARE=1"])

        self.assertIn(
            "hardware opt-in outside hardware_lab",
            "\n".join(failures(suite(), {"GATE_TEST": item})),
        )

    def test_nightly_mutation_claim_requires_the_reviewed_runner_entrypoint(self) -> None:
        item = inventory_record(
            "GATE_MUTATION_BYTECODE_VALIDATOR",
            suite_id="nightly",
            command="python3 scripts/bytecode_validator_mutation.py",
            source_kind="reviewed_command",
            environment="github_nightly",
            artifact_kind="committed_file",
            artifact_paths=[
                "docs/internal/testing/evidence/plc-verification-program/2026-07-09/bytecode-validator-mutation-report.json"
            ],
            artifact_retention="committed",
        )
        record = suite(
            "nightly",
            inventory_ids=["GATE_MUTATION_BYTECODE_VALIDATOR"],
            commands=[item["command"]],
            command_bindings=["GATE_MUTATION_BYTECODE_VALIDATOR"],
        )
        record["duration_class"] = "long"
        record["environment"] = "github_nightly"

        self.assertEqual(
            failures(record, {"GATE_MUTATION_BYTECODE_VALIDATOR": item}),
            [],
        )

    def test_release_rejects_machine_local_and_target_artifacts(self) -> None:
        item = inventory_record("GATE_RELEASE", suite_id="release")
        record = suite("release", inventory_ids=["GATE_RELEASE"], commands=[item["command"]])
        record["duration_class"] = "long"

        joined = "\n".join(failures(record, {"GATE_RELEASE": item}))
        self.assertIn("release command must name durable evidence or a CI artifact", joined)
        self.assertIn("release command artifact cannot use target/", joined)
        self.assertIn("release evidence_destination cannot use target/", joined)

    def test_valid_release_ci_artifact_uses_workflow_inventory(self) -> None:
        item = inventory_record(
            "GATE_RELEASE",
            suite_id="release",
            source_kind="github_workflow_job",
            environment="github_release",
            artifact_kind="ci_artifact",
            artifact_paths=["release-runtime-vm-validation"],
            artifact_retention="repository_default",
        )
        item["path"] = ".github/workflows/release.yml"
        record = suite("release", inventory_ids=["GATE_RELEASE"], commands=[item["command"]])
        record["duration_class"] = "long"
        record["environment"] = "github_release"
        record["evidence_destination"] = "ci-artifact:release-runtime-vm-validation"

        self.assertEqual(failures(record, {"GATE_RELEASE": item}), [])

    def test_full_validator_delegates_suite_contract(self) -> None:
        record = suite(commands=["forged command"])
        record["_path"] = ROOT / SUITE_PATH
        validator = Validator()
        validator.suites = {"veryquick": record}
        validator.gate_inventory = {"GATE_TEST": inventory_record()}

        validator.validate_suites()

        self.assertTrue(
            any("ordered command_bindings projection" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_unknown_include_still_fails_without_defining_composition(self) -> None:
        record = suite()
        record["includes"] = ["not_a_suite"]

        self.assertIn("includes unknown suite not_a_suite", "\n".join(failures(record)))


if __name__ == "__main__":
    unittest.main()
