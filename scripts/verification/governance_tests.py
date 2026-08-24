"""Focused tests for Phase 14 governance."""

from __future__ import annotations

import copy
import unittest
from datetime import date
from pathlib import Path

from scripts.verification.governance import (
    load_governance,
    load_retirements,
    validate_changed_files,
    validate_current_governance,
    validate_governance_document,
)
from scripts.verification.metadata_validator.core import Validator


class GovernanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = Validator()
        cls.validator.load_records()
        cls.document = load_governance(Path.cwd())
        cls.retirements = load_retirements(Path.cwd())

    def validate(self, document=None, retirements=None) -> list[str]:
        return validate_governance_document(
            document or self.document,
            invariants=self.validator.invariants,
            suites=self.validator.suites,
            matrix=self.validator.matrix,
            retirements=retirements or self.retirements,
            evidence=self.validator.evidence,
        )

    def test_live_governance_document_is_structurally_valid(self) -> None:
        self.assertEqual(self.validate(), [])

    def test_owner_alias_suite_and_template_drift_fail_closed(self) -> None:
        tampered = copy.deepcopy(self.document)
        tampered["area_owner_rules"][0]["owners"] = []
        self.assertIn("owner", "\n".join(self.validate(tampered)))

        tampered = copy.deepcopy(self.document)
        tampered["suite_composition"]["proof_inheritance"] = True
        self.assertIn("proof_inheritance", "\n".join(self.validate(tampered)))

        tampered = copy.deepcopy(self.document)
        bytecode = next(row for row in tampered["coverage_templates"] if row["area"] == "bytecode_vm")
        bytecode["required_dimensions"].remove("resource_limit")
        self.assertIn("does not match matrix", "\n".join(self.validate(tampered)))

    def test_product_and_public_claim_changes_do_not_require_metadata_churn(self) -> None:
        direct_policy = copy.deepcopy(self.document)
        direct_policy["change_policy"]["product_change_requires"] = [
            "written_specification",
            "native_executable_test",
        ]
        direct_policy["change_policy"]["public_claim_change_requires"] = [
            "written_specification",
            "native_executable_test",
        ]
        product = ["crates/trust-runtime/src/runtime/cycle.rs"]
        self.assertEqual(validate_changed_files(direct_policy, product), [])

        public = ["docs/public/reference/conformance.md"]
        self.assertEqual(validate_changed_files(direct_policy, public), [])

        self.assertEqual(
            direct_policy["change_policy"]["product_change_requires"],
            ["written_specification", "native_executable_test"],
        )
        self.assertEqual(
            direct_policy["change_policy"]["public_claim_change_requires"],
            ["written_specification", "native_executable_test"],
        )

    def test_stale_metadata_unknown_grace_and_cadence_fail_closed(self) -> None:
        records = {"OLD": {"last_reviewed": "2026-01-01"}}
        failures = validate_current_governance(
            self.document,
            today=date(2026, 7, 21),
            record_groups=(("test", records),),
            ignored_tests={},
        )
        self.assertIn("is stale", "\n".join(failures))

        ignored = {
            "IGNORED": {
                "ignore_class": "unknown",
                "last_reviewed": "2026-06-01",
            }
        }
        failures = validate_current_governance(
            self.document,
            today=date(2026, 7, 21),
            record_groups=(),
            ignored_tests=ignored,
        )
        self.assertIn("unknown grace period", "\n".join(failures))

        failures = validate_current_governance(
            self.document,
            today=date(2026, 7, 21),
            record_groups=(),
            ignored_tests={},
            invariants={
                "INV": {
                    "risk": "safety_critical",
                    "oracle": {"ref": "MISSING"},
                    "last_reviewed": "2026-06-01",
                }
            },
            spec_sources={},
        )
        self.assertIn("missing-oracle grace period", "\n".join(failures))

        failures = validate_current_governance(
            self.document,
            today=date(2026, 11, 1),
            record_groups=(),
            ignored_tests={},
        )
        self.assertIn("review cadence", "\n".join(failures))

    def test_retirement_is_append_only_and_evidence_bound(self) -> None:
        registry = {
            "retirements": [
                {
                    "kind": "invariant",
                    "id": "MISSING",
                    "owner": "verification",
                    "rationale": "obsolete",
                    "replacement": "none",
                    "retired_at": "2026-07-19",
                    "evidence_refs": ["MISSING_EVIDENCE"],
                }
            ]
        }
        joined = "\n".join(self.validate(retirements=registry))
        self.assertIn("does not retain its source record", joined)
        self.assertIn("unknown evidence", joined)

    def _record_groups(self):
        return (
            ("spec source", self.validator.spec_sources),
            ("spec gap", self.validator.spec_gaps),
            ("test", self.validator.tests),
            ("ignored test", self.validator.ignored_tests),
            ("risk", self.validator.risks),
            ("invariant", self.validator.invariants),
            ("suite", self.validator.suites),
        )


if __name__ == "__main__":
    unittest.main()
