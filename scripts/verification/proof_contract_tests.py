"""Unit tests for proof metadata contract freezing."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from scripts.verification.proof_contract import (
    PROOF_CONTRACT_VERSION,
    ProofContractError,
    proof_contract_digest,
    proof_contract_payload,
)


class ProofContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.test = {
            "schema_version": 2,
            "id": "TEST_PROOF",
            "title": "Proof target",
            "command": "python3 test.py",
            "invariants": ["INV_A", "INV_B"],
            "suite_tiers": ["pr"],
            "status": "mapped",
            "spec_gap_ref": "SPEC_GAP_A",
            "last_reviewed": "2026-07-12",
            "_path": Path("verification/test-catalog.toml"),
        }
        self.invariants = {
            "INV_A": {
                "schema_version": 1,
                "id": "INV_A",
                "status": "gap_open",
                "proof_level": "S0",
                "tests": [],
                "gates": ["pr"],
                "evidence_refs": [],
                "spec_gap_refs": ["SPEC_GAP_A"],
                "missing": ["proof"],
                "coverage": {"cells": [{"dimension": "happy_path", "state": "gap_open"}]},
                "oracle": {"kind": "spec", "ref": "SPEC_A"},
                "behavior": [{"outcome": "accept_value", "oracle_ref": "SPEC_A#rule"}],
                "last_reviewed": "2026-07-12",
                "_path": Path("verification/invariants/a.toml"),
            },
            "INV_B": {
                "schema_version": 1,
                "id": "INV_B",
                "status": "gap_open",
                "proof_level": "S0",
                "tests": [],
                "gates": ["pr"],
                "evidence_refs": [],
                "spec_gap_refs": [],
                "missing": [],
                "oracle": {"kind": "spec", "ref": "SPEC_B"},
                "behavior": [{"outcome": "accept_value", "oracle_ref": "SPEC_B#rule"}],
                "_path": Path("verification/invariants/b.toml"),
            },
        }

    def test_digest_is_canonical_and_excludes_loader_private_fields(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        reordered_test = dict(reversed(list(self.test.items())))
        reordered_invariants = {
            "INV_B": dict(reversed(list(self.invariants["INV_B"].items()))),
            "INV_A": dict(reversed(list(self.invariants["INV_A"].items()))),
        }
        reordered_test["_path"] = Path("elsewhere/catalog.toml")
        reordered_invariants["INV_A"]["_path"] = Path("elsewhere/a.toml")

        actual = proof_contract_digest(
            test=reordered_test,
            invariants=reordered_invariants,
        )

        self.assertEqual(actual, expected)
        self.assertRegex(actual, r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(
            proof_contract_payload(test=self.test, invariants=self.invariants)["contract_version"],
            PROOF_CONTRACT_VERSION,
        )

    def test_arbitrary_catalog_row_drift_changes_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        changed = copy.deepcopy(self.test)
        changed["expected_result"] = "different reviewed expectation"

        actual = proof_contract_digest(test=changed, invariants=self.invariants)

        self.assertNotEqual(actual, expected)

    def test_command_or_invariant_list_drift_changes_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        for field, value in (
            ("command", "python3 other.py"),
            ("invariants", ["INV_B", "INV_A"]),
        ):
            with self.subTest(field=field):
                changed = copy.deepcopy(self.test)
                changed[field] = value
                self.assertNotEqual(
                    proof_contract_digest(test=changed, invariants=self.invariants),
                    expected,
                )

    def test_linked_invariant_content_drift_changes_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        changed = copy.deepcopy(self.invariants)
        changed["INV_A"]["oracle"]["ref"] = "SPEC_OTHER"

        actual = proof_contract_digest(test=self.test, invariants=changed)

        self.assertNotEqual(actual, expected)

    def test_catalog_and_invariant_lifecycle_progression_preserves_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        changed_test = copy.deepcopy(self.test)
        changed_test.update(
            {
                "status": "validated",
                "suite_tiers": ["pr", "nightly", "release"],
                "spec_gap_ref": None,
                "last_reviewed": "2026-07-13",
            }
        )
        changed_invariants = copy.deepcopy(self.invariants)
        changed_invariants["INV_A"].update(
            {
                "status": "validated",
                "proof_level": "G2",
                "tests": ["TEST_PROOF"],
                "gates": ["pr", "nightly"],
                "evidence_refs": ["EVID_RED", "EVID_GREEN", "EVID_BROAD"],
                "spec_gap_refs": [],
                "missing": [],
                "coverage": {"cells": [{"dimension": "happy_path", "state": "covered"}]},
                "last_reviewed": "2026-07-13",
            }
        )

        actual = proof_contract_digest(test=changed_test, invariants=changed_invariants)

        self.assertEqual(actual, expected)

    def test_coverage_scope_drift_changes_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        changes = []

        changed_dimension = copy.deepcopy(self.invariants)
        changed_dimension["INV_A"]["coverage"]["cells"][0]["dimension"] = "boundary"
        changes.append(changed_dimension)

        added_cell = copy.deepcopy(self.invariants)
        added_cell["INV_A"]["coverage"]["cells"].append(
            {
                "dimension": "boundary",
                "state": "gap_open",
                "rationale": "A second reviewed coverage obligation.",
            }
        )
        changes.append(added_cell)

        added_decision = copy.deepcopy(self.invariants)
        added_decision["INV_A"]["coverage"]["cells"][0]["decision_ref"] = "SPEC_DECISION"
        changes.append(added_decision)

        for changed in changes:
            with self.subTest(changed=changed):
                self.assertNotEqual(
                    proof_contract_digest(test=self.test, invariants=changed),
                    expected,
                )

    def test_coverage_lifecycle_fields_preserve_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        changed = copy.deepcopy(self.invariants)
        cell = changed["INV_A"]["coverage"]["cells"][0]
        cell.update(
            {
                "state": "covered",
                "rationale": "The committed proof now covers this dimension.",
                "spec_gap_ref": None,
            }
        )

        self.assertEqual(
            proof_contract_digest(test=self.test, invariants=changed),
            expected,
        )

    def test_behavior_and_oracle_drift_change_digest(self) -> None:
        expected = proof_contract_digest(test=self.test, invariants=self.invariants)
        for mutate in (
            lambda records: records["INV_A"]["oracle"].__setitem__("ref", "SPEC_OTHER"),
            lambda records: records["INV_A"]["behavior"][0].__setitem__("outcome", "reject"),
        ):
            with self.subTest(mutate=mutate):
                changed = copy.deepcopy(self.invariants)
                mutate(changed)
                self.assertNotEqual(
                    proof_contract_digest(test=self.test, invariants=changed),
                    expected,
                )

    def test_missing_or_duplicate_invariant_reference_is_rejected(self) -> None:
        for invariant_ids, expected in (
            (["INV_A", "INV_MISSING"], "unknown invariant INV_MISSING"),
            (["INV_A", "INV_A"], "duplicate invariant INV_A"),
        ):
            with self.subTest(invariant_ids=invariant_ids):
                changed = copy.deepcopy(self.test)
                changed["invariants"] = invariant_ids
                with self.assertRaisesRegex(ProofContractError, expected):
                    proof_contract_digest(test=changed, invariants=self.invariants)


if __name__ == "__main__":
    unittest.main()
