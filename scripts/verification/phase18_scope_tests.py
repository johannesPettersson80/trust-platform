"""Regression tests for the post-closure specification-and-test audit scope."""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class Phase18ScopeTests(unittest.TestCase):
    def test_per_function_spec_gap_framework_is_absent(self) -> None:
        forbidden = [
            "verification/code-spec-map.toml",
            "verification/code-spec-debt-baseline.toml",
            "verification/code-spec-private-inputs.toml",
            "verification/code-spec-risk-acknowledgements.toml",
            "scripts/code_spec_coverage_gate.py",
            "scripts/verification/code_spec_contract.py",
            "scripts/verification/p18_zero_debt/burn_down_ledger.py",
        ]
        present = [path for path in forbidden if (ROOT / path).exists()]
        self.assertEqual([], present, f"per-function framework remains: {present}")

    def test_blanket_historical_code_gaps_are_absent(self) -> None:
        payload = tomllib.loads((ROOT / "verification/spec-gaps.toml").read_text())
        ids = {row["id"] for row in payload.get("spec_gaps", [])}
        blanket = sorted(value for value in ids if value.startswith("SPEC_GAP_P17_HISTORICAL_CODE_"))
        self.assertEqual([], blanket, f"blanket code gaps remain: {blanket}")

    def test_phase18_is_a_direct_specification_and_native_test_audit(self) -> None:
        closure = (
            ROOT
            / "docs/internal/testing/evidence/plc-verification-program/2026-07-19/p16-final-closure.md"
        ).read_text()
        self.assertIn("specification gaps: 44 closed, 0 open", closure)

        board = (
            ROOT
            / "docs/internal/testing/checklists/plc-verification-program/phase18-zero-debt-execution-board.md"
        ).read_text()
        self.assertIn("Post-Closure Behavior Delta", board)
        self.assertIn("written specification -> native executable test", board)
        self.assertIn("`already_covered`", board)
        self.assertIn("`missing_spec`", board)
        self.assertIn("`missing_test`", board)
        self.assertIn("`behavior_defect`", board)
        self.assertIn("`external_manual`", board)
        self.assertNotIn("every behavior-bearing code fact", board)
        self.assertNotIn("Eliminate all 4,233 current `reviewed_nonmapping`", board)
        for false_gap in (
            "IEC_PROJECT_EXAMPLE_CONFORMANCE_001",
            "RT_CONNECTOR_REPORT_PROJECTION_001",
            "RT_EXECUTION_LANGUAGE_CONFORMANCE_001",
            "RT_HOST_SECURITY_WEB_CONFORMANCE_001",
            "RT_STDLIB_CORE_HELPER_002",
            "RT_STDLIB_FB_RUNTIME_002",
        ):
            with self.subTest(false_gap=false_gap):
                self.assertNotIn(false_gap, board)

    def test_metadata_state_cannot_create_product_work(self) -> None:
        board = (
            ROOT
            / "docs/internal/testing/checklists/plc-verification-program/phase18-zero-debt-execution-board.md"
        ).read_text()
        self.assertIn("cannot create a behavior-ledger row", board)
        for forbidden_work_generator in (
            "Freeze deterministic case and mutation inputs",
            "promote an invariant",
            "current targeted proof is required",
            "invariant-to-test authority debts",
        ):
            with self.subTest(forbidden=forbidden_work_generator):
                self.assertNotIn(forbidden_work_generator, board)

    def test_retired_artifact_campaigns_are_not_active_metadata_gates(self) -> None:
        core = (
            ROOT / "scripts/verification/metadata_validator/core.py"
        ).read_text()
        for retired_gate in (
            "validate_mutation_program_contract(ROOT",
            "validate_committed_mutation_metadata(",
        ):
            with self.subTest(retired_gate=retired_gate):
                self.assertNotIn(retired_gate, core)
        self.assertIn("validate_ui_acceptance_document(", core)
        self.assertIn("changed_paths_since_evidence(", core)

        metadata_gate = (ROOT / "scripts/verification_metadata_gate.sh").read_text()
        self.assertNotIn("gen_cases.py", metadata_gate)
        self.assertNotIn("gen_cases_v2.py", metadata_gate)


if __name__ == "__main__":
    unittest.main()
