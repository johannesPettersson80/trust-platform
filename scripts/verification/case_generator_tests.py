"""Focused tests for decision-table case generation scope."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.case_generator import CaseGenerationError, generate_case_file
from scripts.verification.case_generator_v2 import generate_case_file as generate_case_file_v2


class _Validator:
    def __init__(self, invariant: dict[str, object]) -> None:
        self.invariants = {str(invariant["id"]): invariant}


class CaseGeneratorTests(unittest.TestCase):
    def test_v1_keeps_bytecode_only_scope(self) -> None:
        invariant = {
            "_path": Path("verification/invariants/compiler_iec/IEC_PREC_001.toml"),
            "id": "IEC_PREC_001",
            "title": "Expression precedence",
            "area": "compiler_iec",
            "owner": "trust-hir",
            "contract_kind": "decision_table",
            "last_reviewed": "2026-07-16",
            "behavior": [
                {
                    "partition": {"equals": "MULTIPLICATIVE_BEFORE_ADDITIVE"},
                    "case_family": "ordering_or_lifecycle",
                    "oracle_ref": "SPEC_IEC_EXPRESSIONS_001#operator-precedence",
                    "outcome": "accept_value",
                    "delta": {"target": "set_to_expected"},
                }
            ],
        }

        with self.assertRaisesRegex(CaseGenerationError, "scoped to bytecode_vm"):
            generate_case_file("IEC_PREC_001", _Validator(invariant))

    def test_v2_generates_non_bytecode_decision_table_cases(self) -> None:
        invariant = {
            "_path": Path("verification/invariants/compiler_iec/IEC_PREC_001.toml"),
            "id": "IEC_PREC_001",
            "title": "Expression precedence",
            "area": "compiler_iec",
            "owner": "trust-hir",
            "contract_kind": "decision_table",
            "last_reviewed": "2026-07-16",
            "behavior": [
                {
                    "partition": {"equals": "MULTIPLICATIVE_BEFORE_ADDITIVE"},
                    "case_family": "ordering_or_lifecycle",
                    "oracle_ref": "SPEC_IEC_EXPRESSIONS_001#operator-precedence",
                    "outcome": "accept_value",
                    "delta": {"target": "set_to_expected"},
                }
            ],
        }

        record = generate_case_file_v2("IEC_PREC_001", _Validator(invariant))

        self.assertEqual(record["generator"], "gen_cases_v2.py v1")
        self.assertEqual(record["area"], "compiler_iec")
        self.assertEqual(len(record["case"]), 1)
        self.assertEqual(
            record["case"][0]["input"]["scenario"],
            "MULTIPLICATIVE_BEFORE_ADDITIVE",
        )


if __name__ == "__main__":
    unittest.main()
