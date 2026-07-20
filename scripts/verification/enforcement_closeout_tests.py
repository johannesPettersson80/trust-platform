"""Regression tests for the atomic P16-007 enforcement close-out."""

from __future__ import annotations

import unittest
from pathlib import Path

from .conformance_alignment_live import REQUIRED_OPEN_ROWS as CONFORMANCE_OPEN_ROWS
from .fuzz_program_live import (
    REQUIRED_OPEN_POLICY_ROWS as FUZZ_OPEN_POLICY_ROWS,
    REQUIRED_OPEN_ROWS as FUZZ_OPEN_ROWS,
)
from .metadata_validator.constants import ROOT
from .mutation_program_live import (
    REQUIRED_OPEN_POLICY_ROWS as MUTATION_OPEN_POLICY_ROWS,
    REQUIRED_OPEN_ROWS as MUTATION_OPEN_ROWS,
)
from .requirement_oracle_live import REQUIRED_OPEN_ROWS as REQUIREMENT_OPEN_ROWS
from .runtime_anomaly_live import (
    REQUIRED_OPEN_POLICY_ROWS as ANOMALY_OPEN_POLICY_ROWS,
    REQUIRED_OPEN_ROWS as ANOMALY_OPEN_ROWS,
)


BOARD = ROOT / "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
POLICY = ROOT / "docs/internal/testing/checklists/plc-verification-program/policy.md"
README = ROOT / "verification/README.md"


class EnforcementCloseoutTests(unittest.TestCase):
    def test_authorized_rows_and_stop_gate_close_atomically(self) -> None:
        board = BOARD.read_text()
        for row_id in (
            "VERIF-P1B-012",
            "VERIF-P1B-014",
            *(f"VERIF-P15-{index:03d}" for index in range(1, 13)),
            "VERIF-P16-007",
        ):
            self.assertIn(f"- [x] `{row_id}`", board, row_id)
        self.assertIn("- [x] `VERIF-P16-008`", board)

        policy = POLICY.read_text()
        self.assertIn("- [x] `VERIF-STOP-012`", policy)
        self.assertIn("- [ ] `VERIF-STOP-014`", policy)

    def test_closed_ratchets_are_absent_from_every_live_open_guard(self) -> None:
        board = BOARD.read_text()
        for rows in (
            CONFORMANCE_OPEN_ROWS,
            FUZZ_OPEN_ROWS,
            MUTATION_OPEN_ROWS,
            REQUIREMENT_OPEN_ROWS,
            ANOMALY_OPEN_ROWS,
        ):
            self.assertNotIn("VERIF-P1B-012", rows)
            self.assertNotIn("VERIF-P1B-014", rows)
            for row_id in rows:
                self.assertIn(
                    f"- [ ] `{row_id}`",
                    board,
                    f"stale open-row guard for completed or missing row {row_id}",
                )
        for rows in (
            FUZZ_OPEN_POLICY_ROWS,
            MUTATION_OPEN_POLICY_ROWS,
            ANOMALY_OPEN_POLICY_ROWS,
        ):
            self.assertNotIn("VERIF-STOP-012", rows)
            self.assertIn("VERIF-STOP-014", rows)

    def test_override_procedure_cannot_disable_enforcement(self) -> None:
        text = README.read_text()
        for fragment in (
            "Enforcing verification gate",
            "must not remove `--strict`",
            "false block remains red",
            "tracked override decision",
            "rerun the enforcing gate",
        ):
            self.assertIn(fragment, text)


if __name__ == "__main__":
    unittest.main()
