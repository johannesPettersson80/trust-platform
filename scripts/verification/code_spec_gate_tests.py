"""Focused enforcement-policy tests for Phase 17 code-to-spec findings."""

from __future__ import annotations

import unittest

from scripts.verification.code_spec_gate import (
    blocking_failures,
    changed_path_outcomes,
    finding_kind,
)


class CodeSpecGateTests(unittest.TestCase):
    def test_modes_keep_historical_debt_visible_but_changed_mode_nonblocking(self) -> None:
        debt_id = "CODE_" + "A" * 64
        stale_id = "CODE_" + "B" * 64
        failures = (
            f"{debt_id}: unmapped",
            f"{stale_id}: stale_spec_review body digest changed",
        )

        self.assertEqual([], blocking_failures("report", failures, {debt_id}))
        self.assertEqual(
            [f"{stale_id}: stale_spec_review body digest changed"],
            blocking_failures("changed", failures, {debt_id}),
        )
        self.assertEqual(sorted(failures), blocking_failures("full", failures, {debt_id}))

    def test_changed_paths_surface_each_planner_outcome_without_inventing_behavior(self) -> None:
        missing = "CODE_" + "A" * 64
        stale = "CODE_" + "B" * 64
        gap = "CODE_" + "C" * 64
        missing_tests = "CODE_" + "D" * 64
        unmapped = "CODE_" + "E" * 64
        facts = {
            code_id: {"path": "crates/example/src/lib.rs"}
            for code_id in (missing, stale, gap, missing_tests, unmapped)
        }
        failures = (
            f"{missing}: missing_spec new fact absent from map and frozen baseline",
            f"{stale}: stale_spec_review body digest changed",
            f"{missing_tests}: missing_tests no executable test",
            f"{unmapped}: unmapped",
        )
        mappings = {gap: {"disposition": "spec_gap", "spec_gap_ref": "GAP_EXAMPLE"}}

        outcomes = changed_path_outcomes(
            ["crates/example/src/lib.rs"],
            facts=facts,
            mappings=mappings,
            failures=failures,
        )

        self.assertEqual(
            ["missing_spec", "missing_tests", "spec_gap", "stale_spec_review", "unmapped"],
            sorted({finding_kind(outcome) for outcome in outcomes}),
        )


if __name__ == "__main__":
    unittest.main()
