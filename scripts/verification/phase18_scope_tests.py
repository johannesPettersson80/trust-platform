"""Regression tests for the corrected Phase 18 verification scope."""

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

    def test_phase18_is_a_post_closure_behavior_delta(self) -> None:
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
        self.assertNotIn("every behavior-bearing code fact", board)
        self.assertNotIn("Eliminate all 4,233 current `reviewed_nonmapping`", board)

    def test_behavior_invariants_have_specs_and_only_reviewed_mapping_debt(self) -> None:
        missing_specifications: list[str] = []
        missing_tests: list[str] = []
        for path in sorted((ROOT / "verification/invariants").glob("*/*.toml")):
            invariant = tomllib.loads(path.read_text())
            invariant_id = invariant["id"]
            spec = invariant.get("spec", {})
            if spec.get("status") != "specified" or not spec.get("source_refs"):
                missing_specifications.append(invariant_id)
            if not invariant.get("tests"):
                missing_tests.append(invariant_id)

        self.assertEqual(
            [],
            missing_specifications,
            f"invariants without written specification authority: {missing_specifications}",
        )
        self.assertEqual(
            [
                "IEC_PROJECT_EXAMPLE_CONFORMANCE_001",
                "RT_CONNECTOR_REPORT_PROJECTION_001",
                "RT_EXECUTION_LANGUAGE_CONFORMANCE_001",
                "RT_HOST_SECURITY_WEB_CONFORMANCE_001",
                "RT_STDLIB_CORE_HELPER_002",
                "RT_STDLIB_FB_RUNTIME_002",
            ],
            sorted(missing_tests),
            f"unexpected invariant-to-test debt: {missing_tests}",
        )


if __name__ == "__main__":
    unittest.main()
