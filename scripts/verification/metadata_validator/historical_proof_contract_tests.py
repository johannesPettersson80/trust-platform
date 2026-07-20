"""Tests for immutable source-revision proof reconstruction."""

from __future__ import annotations

import unittest

from scripts.verification.metadata_validator.historical_proof_contract import (
    HistoricalProofContractError,
    load_historical_proof_contract,
)
from scripts.verification.proof_contract import proof_contract_digest


class HistoricalProofContractTests(unittest.TestCase):
    def test_force_red_and_green_reconstruct_the_recorded_contract(self) -> None:
        expected = "sha256:32bfa0a002ab112d30a60d7c2955072fcc210d1fc879a056a2eef3a6199122be"
        for revision in (
            "4123321a31268d04d42d887a3c8ba034fefceae6",
            "320c01d65f4cc906f603a935f9f3b0ef8ba28480",
        ):
            with self.subTest(revision=revision):
                test, invariants = load_historical_proof_contract(
                    revision,
                    "TEST_RUNTIME_FORCE_LIFECYCLE_001",
                )
                self.assertEqual(
                    proof_contract_digest(test=test, invariants=invariants),
                    expected,
                )

    def test_invalid_or_missing_revision_fails_closed(self) -> None:
        with self.assertRaisesRegex(HistoricalProofContractError, "clean full commit"):
            load_historical_proof_contract("deadbeef", "TEST_RUNTIME_FORCE_LIFECYCLE_001")
        with self.assertRaisesRegex(HistoricalProofContractError, "cannot read"):
            load_historical_proof_contract(
                "f" * 40,
                "TEST_RUNTIME_FORCE_LIFECYCLE_001",
            )


if __name__ == "__main__":
    unittest.main()
