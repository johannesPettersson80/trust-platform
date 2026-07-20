"""Contract tests for the committed verification-tooling bypass fixtures."""

from __future__ import annotations

import copy
import unittest
from unittest import mock

from scripts.verification import tooling_selftest_spec_sources
from scripts.verification.tooling_selftest_contract import (
    BYPASS_CONTRACT_PATH,
    REQUIRED_CASE_IDS,
    load_bypass_contract,
    render_fixture_report,
    validate_bypass_contract,
)
from scripts.verification.tooling_selftest_scenarios import (
    _RESULT_CACHE,
    SCENARIO_HANDLERS,
    execute_bypass_case,
)
from scripts.verification.prover import ProofError, ProofProducer


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

    def test_spec_source_scanner_uses_real_production_fixtures(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)

        self.assertEqual(contract["spec_source_scanner_status"], "mapped")
        self.assertEqual(contract["spec_source_scanner_blocked_by"], [])
        scanner_cases = [
            row
            for row in contract["cases"]
            if row["assigned_layer"] == "spec_source_scanner"
        ]
        self.assertEqual(
            {row["id"] for row in scanner_cases},
            {
                "P6A_GOOD_SPEC_SOURCE_SCAN_001",
                "P6A_BAD_SPEC_SOURCE_MISSING_REGISTERED_PATH_001",
                "P6A_BAD_SPEC_SOURCE_UNCLOSED_FENCE_001",
                "P6A_BAD_SPEC_SOURCE_STALE_CLAIM_TEXT_001",
                "P6A_BAD_SPEC_SOURCE_ESCAPING_INCLUDE_001",
                "P6A_BOUNDARY_SPEC_SOURCE_UNREVIEWED_PROSE_001",
            },
        )

    def test_spec_source_known_good_calls_production_discovery_and_analysis(self) -> None:
        with mock.patch.object(
            tooling_selftest_spec_sources,
            "discover_spec_documents",
            wraps=tooling_selftest_spec_sources.discover_spec_documents,
        ) as discovery, mock.patch.object(
            tooling_selftest_spec_sources,
            "analyze_spec_sources",
            wraps=tooling_selftest_spec_sources.analyze_spec_sources,
        ) as analysis:
            result = tooling_selftest_spec_sources.spec_source_scan_known_good()

        self.assertEqual(result.disposition, "accept")
        self.assertEqual(discovery.call_count, 1)
        self.assertEqual(analysis.call_count, 1)

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
        self.assertIn("Spec-source scanner self-tests: `mapped`", first)
        tampered = copy.deepcopy(results)
        tampered[0] = tampered[0]._replace(matched=False)
        self.assertNotEqual(render_fixture_report(contract, tampered), first)

    def test_rejected_red_proof_with_created_evidence_fails_fixture(self) -> None:
        contract = load_bypass_contract(BYPASS_CONTRACT_PATH)
        row = next(
            row
            for row in contract["cases"]
            if row["id"] == "P6A_BAD_COMPILE_ERROR_AS_RED_001"
        )

        def write_evidence_then_reject(producer: ProofProducer, _test_id: str) -> None:
            producer.evidence_dir.mkdir(parents=True)
            (producer.evidence_dir / "unexpected.toml").write_text("unexpected evidence\n")
            raise ProofError("compile error", failure_kind="compile_error")

        _RESULT_CACHE.pop(row["id"], None)
        try:
            with mock.patch.object(
                ProofProducer,
                "red",
                autospec=True,
                side_effect=write_evidence_then_reject,
            ):
                result = execute_bypass_case(row)
        finally:
            _RESULT_CACHE.pop(row["id"], None)

        self.assertEqual(result.actual_disposition, "reject")
        self.assertIn("evidence-created", result.actual_signal)
        self.assertFalse(result.matched)


if __name__ == "__main__":
    unittest.main()
