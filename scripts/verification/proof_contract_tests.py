"""Unit tests for proof metadata contract freezing."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from scripts.verification.proof_contract import (
    ProofContractError,
    proof_contract_digest,
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
            "_path": Path("verification/test-catalog.toml"),
        }
        self.invariants = {
            "INV_A": {
                "schema_version": 1,
                "id": "INV_A",
                "status": "gap_open",
                "oracle": {"kind": "spec", "ref": "SPEC_A"},
                "_path": Path("verification/invariants/a.toml"),
            },
            "INV_B": {
                "schema_version": 1,
                "id": "INV_B",
                "status": "gap_open",
                "oracle": {"kind": "spec", "ref": "SPEC_B"},
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
