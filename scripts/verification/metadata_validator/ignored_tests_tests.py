"""Tests for the reviewed ignored-test registry contract."""

from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.metadata_validator.ignored_tests import (
    BASE_FIELDS,
    CLASS_FIELDS,
    IGNORE_CLASSES,
    IGNORE_MECHANISMS,
    load_checklist_row_ids,
    validate_ignored_test_records,
)
from scripts.verification.ignored_test_models import IgnoredTestFact


DISCOVERY_ID = "DISC_0123456789ABCDEF0123"
TEST_ID = "TEST_IGNORED_EXAMPLE"
RECORD_ID = "IGNORED_EXAMPLE_001"
ROW_ID = "VERIF-P3-003"


class IgnoredTestContractTests(unittest.TestCase):
    def test_red_protective_record_joins_exactly_one_discovered_ignore(self) -> None:
        with registry_root() as root:
            record = red_record()
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact()],
            )

        self.assertEqual(failures, [])

    def test_inventory_join_is_exhaustive_and_one_to_one(self) -> None:
        with registry_root() as root:
            first = unknown_record()
            duplicate = copy.deepcopy(first)
            duplicate["id"] = "IGNORED_EXAMPLE_002"
            duplicate["test_id"] = TEST_ID
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={first["id"]: first, duplicate["id"]: duplicate},
                tests={TEST_ID: {"discovery_id": "DISC_FFFFFFFFFFFFFFFFFFFF"}},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact(), other_ignored_fact()],
            )

        self.assertTrue(any("duplicate discovery_id" in item for item in failures), failures)
        self.assertTrue(any("catalog discovery_id does not match" in item for item in failures), failures)
        self.assertTrue(any("has no registry record" in item for item in failures), failures)

    def test_discovered_identity_and_ignore_mechanism_are_exact_bindings(self) -> None:
        with registry_root() as root:
            record = unknown_record()
            record.update(
                path="crates/elsewhere.rs",
                name="renamed",
                discovery_source_kind="rust_unit_test",
                ignore_state="conditional",
                ignore_reason="different",
                ignore_mechanism="rust_cfg_attr",
            )
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact()],
            )

        for field in (
            "path",
            "name",
            "discovery_source_kind",
            "ignore_state",
            "ignore_reason",
            "ignore_mechanism",
        ):
            self.assertTrue(any(field in item and "does not match" in item for item in failures), failures)

    def test_optional_test_id_must_bind_the_same_catalog_discovery(self) -> None:
        with registry_root() as root:
            unmapped = unknown_record()
            unmapped.pop("test_id", None)
            self.assertEqual(
                validate_ignored_test_records(
                    root=root,
                    ignored_tests={RECORD_ID: unmapped},
                    tests={},
                    checklist_row_ids={ROW_ID},
                    facts=[ignored_fact()],
                ),
                [],
            )

            mapped = unknown_record()
            mapped["test_id"] = TEST_ID
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: mapped},
                tests={TEST_ID: {"discovery_id": DISCOVERY_ID}},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact()],
            )

        self.assertEqual(failures, [])

    def test_optional_test_id_is_unique_across_distinct_registry_records(self) -> None:
        with registry_root() as root:
            first = unknown_record()
            first["test_id"] = TEST_ID
            second_fact = other_ignored_fact()
            second = unknown_record()
            second.update(
                id="IGNORED_EXAMPLE_002",
                test_id=TEST_ID,
                discovery_id=second_fact.discovery_id,
                discovery_source_kind=second_fact.discovery_source_kind,
                path=second_fact.path,
                name=second_fact.name,
                ignore_reason=second_fact.ignore_reason,
            )
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={first["id"]: first, second["id"]: second},
                tests={TEST_ID: {"discovery_id": DISCOVERY_ID}},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact(), second_fact],
            )

        self.assertTrue(any("duplicate test_id" in item for item in failures), failures)

    def test_discovered_line_changes_do_not_stale_identity(self) -> None:
        with registry_root() as root:
            moved_line = replace(ignored_fact(), line=999)
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: unknown_record()},
                tests={},
                checklist_row_ids={ROW_ID},
                facts=[moved_line],
            )

        self.assertEqual(failures, [])

    def test_red_protective_requires_real_checklist_rows_and_symptom(self) -> None:
        with registry_root() as root:
            record = red_record()
            record["linked_rows"] = ["INVENTED-ROW"]
            record["expected_red_symptom"] = ""
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
            )

        self.assertTrue(any("unknown checklist row" in item for item in failures), failures)
        self.assertTrue(any("expected_red_symptom" in item for item in failures), failures)

    def test_red_protective_rejects_ambiguous_checklist_row_id(self) -> None:
        with registry_root() as root:
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: red_record()},
                tests={},
                checklist_row_ids={ROW_ID: ("first.md:1", "second.md:2")},
            )

        self.assertTrue(any("resolves 2 times" in item for item in failures), failures)

    def test_lab_required_binds_topology_and_public_claim_impact(self) -> None:
        with registry_root() as root:
            record = lab_record()
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
            )
            self.assertEqual(failures, [])

            record["hardware_topology_ref"] = "../outside.md"
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
            )

        self.assertTrue(any("hardware_topology_ref" in item for item in failures), failures)

    def test_lab_required_env_vars_cannot_name_secret_material(self) -> None:
        with registry_root() as root:
            record = lab_record()
            record["required_env_vars"] = [
                "DEVICE_PASSWORD",
                "PRIVATE_KEY_PATH",
                "TRUST_API_TOKEN",
                "TRUST_TLS_KEY_FILE",
            ]
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
            )

        self.assertEqual(
            sum("must not name secret-bearing configuration" in item for item in failures),
            4,
            failures,
        )

    def test_topology_and_flaky_evidence_references_reject_symlink_escape(self) -> None:
        with tempfile.TemporaryDirectory() as outside_temp, registry_root() as root:
            outside = Path(outside_temp) / "outside.md"
            outside.write_text("outside\n")
            topology = root / "docs/internal/testing/lab-topology.md"
            topology.unlink()
            topology.symlink_to(outside)
            lab_failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: lab_record()},
                tests={},
                checklist_row_ids={ROW_ID},
            )

            evidence = root / "docs/internal/testing/flaky-evidence.md"
            evidence.unlink()
            evidence.symlink_to(outside)
            flaky_failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: flaky_record()},
                tests={},
                checklist_row_ids={ROW_ID},
            )

        self.assertTrue(any("hardware_topology_ref" in item for item in lab_failures), lab_failures)
        self.assertTrue(any("evidence_ref" in item for item in flaky_failures), flaky_failures)

    def test_flaky_evidence_reference_rejects_gitignored_file(self) -> None:
        with registry_root() as root:
            (root / ".gitignore").write_text("docs/internal/testing/flaky-evidence.md\n")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: flaky_record()},
                tests={},
                checklist_row_ids={ROW_ID},
            )

        self.assertTrue(any("evidence_ref path is gitignored" in item for item in failures), failures)

    def test_flaky_quarantine_requires_observation_signature_and_durable_evidence(self) -> None:
        with registry_root() as root:
            record = flaky_record()
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
            )
            self.assertEqual(failures, [])

            (root / record["evidence_ref"]).unlink()
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
            )

        self.assertTrue(any("evidence_ref" in item for item in failures), failures)

    def test_unknown_is_report_only_and_forbids_class_only_fields(self) -> None:
        with registry_root() as root:
            record = unknown_record()
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact()],
            )
            self.assertEqual(failures, [])

            record["expected_red_symptom"] = "borrowed red claim"
            failures = validate_ignored_test_records(
                root=root,
                ignored_tests={RECORD_ID: record},
                tests={},
                checklist_row_ids={ROW_ID},
                facts=[ignored_fact()],
            )

        self.assertTrue(any("forbids class-only field" in item for item in failures), failures)

    def test_schema_is_closed_and_drift_pinned_to_validator_vocabulary(self) -> None:
        root = Path(__file__).resolve().parents[3]
        schema = json.loads((root / "verification/schemas/ignored-test.schema.json").read_text())
        properties = schema["properties"]

        self.assertEqual(schema["required"], sorted(BASE_FIELDS))
        self.assertIs(schema["additionalProperties"], False)
        self.assertEqual(properties["schema_version"]["const"], 2)
        self.assertEqual(set(properties), BASE_FIELDS | {"test_id"} | CLASS_FIELDS)
        self.assertEqual(set(properties["ignore_class"]["enum"]), IGNORE_CLASSES)
        self.assertEqual(set(properties["ignore_mechanism"]["enum"]), IGNORE_MECHANISMS)

    def test_checklist_loader_finds_committed_checkbox_ids(self) -> None:
        root = Path(__file__).resolve().parents[3]
        self.assertIn(ROW_ID, load_checklist_row_ids(root))

    def test_registry_corruption_is_rejected_by_full_validator(self) -> None:
        validator = Validator()
        validator.load_records()
        record = unknown_record()
        record["schema_version"] = 1
        record["_path"] = ROOT / "verification/ignored-tests.toml"
        validator.ignored_tests[RECORD_ID] = record

        validator.validate()

        self.assertTrue(
            any(
                "must use schema_version 2" in failure.message
                for failure in validator.failures
            ),
            [failure.message for failure in validator.failures],
        )


def base_record() -> dict:
    return {
        "schema_version": 2,
        "id": RECORD_ID,
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_integration_test",
        "path": "crates/example/tests/ignored.rs",
        "name": "ignored_example",
        "ignore_state": "ignored",
        "ignore_reason": "known ignored fixture",
        "ignore_mechanism": "rust_attribute",
        "owner": "verification",
        "area": "verification",
        "status": "gap_open",
        "ignore_class": "unknown",
        "reason": "Current behavior has not been rerun under the verification program.",
        "unblock_condition": "Rerun and choose removal, re-enable, or a reviewed class.",
        "last_reviewed": "2026-07-10",
    }


def unknown_record() -> dict:
    return base_record()


def red_record() -> dict:
    record = base_record()
    record.update(
        ignore_class="red_protective",
        status="mapped",
        linked_rows=[ROW_ID],
        expected_red_symptom="The protective assertion must fail until the reviewed fix lands.",
    )
    return record


def lab_record() -> dict:
    record = base_record()
    record.update(
        ignore_class="lab_required",
        status="mapped",
        required_env_vars=["TRUST_LAB_TARGET"],
        hardware_topology="A configured target reachable from the validation host.",
        hardware_topology_ref="docs/internal/testing/lab-topology.md",
        public_claim_impact="Blocks verified hardware interoperability claims.",
    )
    return record


def flaky_record() -> dict:
    record = base_record()
    record.update(
        ignore_class="flaky_quarantined",
        status="mapped",
        last_observed_failure="2026-07-09",
        failure_signature="connection reset while awaiting deterministic acknowledgement",
        evidence_ref="docs/internal/testing/flaky-evidence.md",
    )
    return record


def ignored_fact():
    return IgnoredTestFact(
        discovery_id=DISCOVERY_ID,
        native_id="crates/example/tests/ignored.rs#ignored_example",
        discovery_source_kind="rust_integration_test",
        name="ignored_example",
        path="crates/example/tests/ignored.rs",
        line=10,
        package="example",
        command_hint="cargo test -p example --test ignored ignored_example -- --exact",
        ignore_state="ignored",
        ignore_mechanism="rust_attribute",
        ignore_reason="known ignored fixture",
        reference_candidates=(),
    )


def other_ignored_fact():
    return IgnoredTestFact(
        discovery_id="DISC_FEDCBA9876543210FEDC",
        native_id="crates/example/src/lib.rs#other_ignored",
        discovery_source_kind="rust_unit_test",
        name="other_ignored",
        path="crates/example/src/lib.rs",
        line=20,
        package="example",
        command_hint="cargo test -p example other_ignored -- --exact",
        ignore_state="ignored",
        ignore_mechanism="rust_attribute",
        ignore_reason="ignore",
        reference_candidates=(),
    )


class registry_root:
    def __init__(self) -> None:
        self._temp = tempfile.TemporaryDirectory()
        self.root = Path(self._temp.name)

    def __enter__(self) -> Path:
        for relative in (
            "crates/example/tests/ignored.rs",
            "docs/internal/testing/lab-topology.md",
            "docs/internal/testing/flaky-evidence.md",
        ):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture\n")
        return self.root

    def __exit__(self, exc_type, exc, tb) -> None:
        self._temp.cleanup()


if __name__ == "__main__":
    unittest.main()
