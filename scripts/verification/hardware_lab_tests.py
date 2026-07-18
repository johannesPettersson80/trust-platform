"""Contract tests for the Phase 11 hardware-lab program."""

from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.hardware_lab import (
    CASE_IDS,
    HARDWARE_LAB_PATH,
    REPORT_SCHEMA_PATH,
    load_hardware_lab_document,
    validate_hardware_lab_document,
    validate_hardware_lab_schema,
)
from scripts.verification.hardware_lab_live import build_live_hardware_lab_state
from scripts.verification.hardware_lab_report import build_payload, render_markdown
from scripts.verification.hardware_lab_validation import (
    validate_payload,
    validate_schema,
)
from scripts.verification.metadata_validator.core import Validator


ROOT = Path(__file__).resolve().parents[2]


class HardwareLabContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        validator = Validator()
        validator.load_records()
        validator.validate()
        if validator.failures:
            raise AssertionError("committed verification metadata must be valid")
        cls.validator = validator
        cls.document = load_hardware_lab_document(ROOT)

    def validate(self, document: dict) -> list[str]:
        return validate_hardware_lab_document(
            ROOT,
            document,
            ignored_tests=self.validator.ignored_tests,
            spec_sources=self.validator.spec_sources,
            suites=self.validator.suites,
            gate_inventory=self.validator.gate_inventory,
        )

    def test_live_program_is_closed_and_maps_all_reviewed_cases(self) -> None:
        self.assertEqual([], self.validate(self.document))
        self.assertEqual(CASE_IDS, tuple(row["id"] for row in self.document["cases"]))
        self.assertEqual(6, len(self.document["cases"]))
        self.assertTrue(all(row["proof_status"] == "skipped_unproven" for row in self.document["cases"]))

    def test_every_lab_required_ignored_test_is_mapped_exactly_once(self) -> None:
        expected = {
            key
            for key, row in self.validator.ignored_tests.items()
            if row.get("ignore_class") == "lab_required"
        }
        mapped = [
            ignored_id
            for row in self.document["cases"]
            for ignored_id in row["ignored_test_ids"]
        ]
        self.assertEqual(expected, set(mapped))
        self.assertEqual(len(mapped), len(set(mapped)))

    def test_missing_lab_binding_fails_closed(self) -> None:
        changed = copy.deepcopy(self.document)
        changed["cases"][0]["ignored_test_ids"] = []
        failures = self.validate(changed)
        self.assertTrue(any("lab-required ignored-test partition" in item for item in failures), failures)

    def test_skipped_case_cannot_claim_evidence_or_execution(self) -> None:
        changed = copy.deepcopy(self.document)
        changed["cases"][0]["evidence_ids"] = ["EVID_INVENTED"]
        changed["cases"][0]["proof_status"] = "passed"
        failures = self.validate(changed)
        self.assertTrue(any("must remain skipped_unproven" in item for item in failures), failures)
        self.assertTrue(any("cannot carry evidence" in item for item in failures), failures)

    def test_gpio_case_uses_existing_manual_script_without_hardware_claim(self) -> None:
        gpio = next(row for row in self.document["cases"] if row["protocol"] == "gpio")
        self.assertEqual("manual_script", gpio["binding_kind"])
        self.assertEqual("scripts/gpio_hardware_test.sh examples/communication/gpio", gpio["command"])
        self.assertEqual([], gpio["ignored_test_ids"])
        self.assertEqual("skipped_unproven", gpio["proof_status"])

    def test_gpio_strict_or_passing_claim_fails_closed(self) -> None:
        changed = copy.deepcopy(self.document)
        gpio = next(row for row in changed["cases"] if row["protocol"] == "gpio")
        gpio["binding_kind"] = "strict_harness"
        gpio["proof_status"] = "passed"
        failures = self.validate(changed)
        self.assertTrue(any("GPIO" in item or "gpio" in item for item in failures), failures)

    def test_public_claim_boundary_cannot_be_promoted(self) -> None:
        changed = copy.deepcopy(self.document)
        changed["public_claim_status"] = "hardware_qualified"
        failures = self.validate(changed)
        self.assertTrue(any("preview_unverified" in item for item in failures), failures)

    def test_report_is_live_derived_and_renders_every_unproven_case(self) -> None:
        state = build_live_hardware_lab_state(ROOT, branch="plc-verification-program", timestamp="2026-07-19T04:00:00+02:00")
        payload = build_payload(state)
        self.assertEqual([], validate_payload(payload, expected_state=state))
        self.assertEqual(6, payload["summary"]["skipped_unproven"])
        markdown = render_markdown(payload, json_digest="0" * 64)
        for case_id in CASE_IDS:
            self.assertIn(case_id, markdown)
        self.assertIn("No hardware execution is claimed", markdown)

    def test_semantic_report_tamper_is_rejected(self) -> None:
        state = build_live_hardware_lab_state(ROOT, branch="plc-verification-program", timestamp="2026-07-19T04:00:00+02:00")
        payload = build_payload(state)
        payload["cases"][0]["proof_status"] = "passed"
        payload["summary"]["skipped_unproven"] = 5
        failures = validate_payload(payload, expected_state=state)
        self.assertTrue(any("live hardware-lab state" in item or "skipped_unproven" in item for item in failures), failures)

    def test_report_schema_matches_validator_contract(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text(encoding="utf-8"))
        self.assertEqual([], validate_schema(schema))

    def test_manifest_schema_matches_validator_contract(self) -> None:
        schema = json.loads((ROOT / "verification/schemas/hardware-lab.schema.json").read_text(encoding="utf-8"))
        self.assertEqual([], validate_hardware_lab_schema(schema))

    def test_output_paths_cannot_escape_workspace(self) -> None:
        state = build_live_hardware_lab_state(ROOT, branch="plc-verification-program", timestamp="2026-07-19T04:00:00+02:00")
        payload = build_payload(state)
        payload["output_paths"]["json"] = "../outside.json"
        failures = validate_payload(payload)
        self.assertTrue(any("output paths" in item for item in failures), failures)


if __name__ == "__main__":
    unittest.main()
