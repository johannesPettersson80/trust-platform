"""Contract tests for the committed verification-tooling bypass fixtures."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.tooling_selftest_contract import (
    BYPASS_CONTRACT_PATH,
    REQUIRED_CASE_IDS,
    load_bypass_contract,
    render_fixture_report,
    validate_bypass_contract,
)
from scripts.verification.tooling_selftest_scenarios import (
    SCENARIO_HANDLERS,
    execute_bypass_case,
)


class ToolingSelftestContractTests(unittest.TestCase):
    def test_committed_contract_is_closed_and_exhaustive(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)

        self.assertEqual(validate_bypass_contract(contract), [])
        self.assertEqual({row["id"] for row in contract["cases"]}, REQUIRED_CASE_IDS)
        self.assertEqual(
            {row["executor"] for row in contract["cases"]},
            set(SCENARIO_HANDLERS),
        )

    def test_known_good_and_every_bypass_reach_the_assigned_catcher(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)
        results = [execute_bypass_case(row) for row in contract["cases"]]

        failures = [result for result in results if not result.matched]
        self.assertEqual(failures, [])
        metadata_results = [
            result
            for result in results
            if result.assigned_layer.startswith("metadata_validator")
        ]
        self.assertTrue(metadata_results)
        self.assertTrue(all(result.full_wiring_matched for result in metadata_results))

    def test_spec_source_scanner_is_explicitly_blocked_not_simulated(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)

        self.assertEqual(contract["spec_source_scanner_status"], "blocked")
        self.assertEqual(
            contract["spec_source_scanner_blocked_by"],
            ["VERIF-P1A-002", "VERIF-P1A-003", "VERIF-P1A-006"],
        )
        self.assertFalse(
            any("spec_source_scanner" in row["assigned_layer"] for row in contract["cases"])
        )

    def test_contract_tampering_is_rejected(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)
        cases = [
            ("unknown layer", "assigned_layer", "invented_layer"),
            ("valid wrong layer", "assigned_layer", "proof_producer"),
            ("valid wrong kind", "fixture_kind", "boundary"),
            ("valid wrong disposition", "expected_disposition", "report"),
            ("weakened signal", "expected_signal", "weakened"),
            ("assertion overclaim", "assertion_strength", "proved"),
            ("unknown executor", "executor", "invented_executor"),
        ]
        for label, field, value in cases:
            with self.subTest(label=label):
                tampered = copy.deepcopy(contract)
                tampered["cases"][0][field] = value
                self.assertTrue(validate_bypass_contract(tampered))

    def test_fixture_report_is_deterministic_and_bound_to_results(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)
        results = [execute_bypass_case(row) for row in contract["cases"]]

        first = render_fixture_report(contract, results)
        second = render_fixture_report(contract, list(reversed(results)))

        self.assertEqual(first, second)
        self.assertIn("Metadata proves assertion strength: `false`", first)
        self.assertIn("Spec-source scanner self-tests: `blocked`", first)
        tampered = copy.deepcopy(results)
        tampered[0] = tampered[0]._replace(matched=False)
        self.assertNotEqual(render_fixture_report(contract, tampered), first)


if __name__ == "__main__":
    unittest.main()
