"""Focused contract checks for the Phase 17 time.rs pilot metadata."""

from __future__ import annotations

import unittest
from pathlib import Path
import tomllib

from scripts.verification.code_spec_contract import validate_map_document
from scripts.verification.code_spec_live import load_code_spec_metadata
from scripts.verification.code_spec_rust import invoke_rust_extractor


ROOT = Path(__file__).resolve().parents[2]


class TimePilotMetadataTests(unittest.TestCase):
    def test_time_contract_invariant_tests_and_current_dt_gap_are_registered(self) -> None:
        metadata = load_code_spec_metadata(ROOT)
        source = metadata.spec_sources.get("SPEC_IEC_STANDARD_FUNCTIONS_001")
        self.assertIsNotNone(source)
        self.assertEqual("docs/specs/07-standard-functions.md", source["path"])
        self.assertTrue(source["oracle_eligible"])

        invariant = metadata.invariants.get("IEC_STANDARD_TIME_FUNCTIONS_001")
        self.assertIsNotNone(invariant)
        self.assertEqual(
            ["SPEC_IEC_STANDARD_FUNCTIONS_001"],
            invariant["spec"]["source_refs"],
        )
        expected_tests = {
            "TEST_RUNTIME_STDLIB_TIME_ARITHMETIC_001",
            "TEST_RUNTIME_STDLIB_TIME_SPLIT_001",
        }
        self.assertEqual(expected_tests, set(invariant["tests"]))
        for test_id in expected_tests:
            test = metadata.tests[test_id]
            self.assertIn("IEC_STANDARD_TIME_FUNCTIONS_001", test["invariants"])
            self.assertTrue(test["suite_tiers"])

        gap = metadata.spec_gaps.get("SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001")
        self.assertIsNotNone(gap)
        self.assertEqual("open", gap["resolution_status"])
        self.assertEqual("compiler_iec", gap["area"])

    def test_every_live_time_rs_fact_has_one_valid_exact_pilot_mapping(self) -> None:
        metadata = load_code_spec_metadata(ROOT)
        matrix = tomllib.loads((ROOT / "verification/matrix.toml").read_text())
        inventory = invoke_rust_extractor(
            ROOT,
            matrix,
            area_bindings=metadata.area_bindings,
            reject_invented_area_bindings=False,
        )
        pilot_facts = {
            code_id: code_fact
            for code_id, code_fact in inventory.facts.items()
            if code_fact.path == "crates/trust-runtime/src/stdlib/time.rs"
        }
        self.assertEqual(88, len(pilot_facts))
        pilot_mappings = [
            metadata.mappings[code_id]
            for code_id in sorted(pilot_facts)
            if code_id in metadata.mappings
        ]
        self.assertEqual(88, len(pilot_mappings))
        clock_symbols = {
            "trust_runtime::stdlib::time::current_dt",
            "trust_runtime::stdlib::time::is_runtime_clock_name",
            "trust_runtime::stdlib::time::runtime_clock_value",
        }
        for mapping in pilot_mappings:
            if mapping["qualified_symbol"] in clock_symbols:
                self.assertEqual("spec_gap", mapping["disposition"])
                self.assertEqual(
                    "SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001",
                    mapping["spec_gap_ref"],
                )
        failures = validate_map_document(
            {"schema_version": 1, "mappings": pilot_mappings},
            live_facts=pilot_facts,
            spec_sources=metadata.spec_sources,
            spec_gaps=metadata.spec_gaps,
            invariants=metadata.invariants,
            tests=metadata.tests,
            risk_acknowledgements=metadata.risk_acknowledgements,
        )
        self.assertEqual([], failures)


if __name__ == "__main__":
    unittest.main()
