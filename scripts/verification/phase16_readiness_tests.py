"""Tests for the report-only Phase 16 product-change fence."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.phase16_readiness import (
    PHASE16_PILOT_ROW,
    PHASE16_READINESS_ROWS,
    PHASE16_REVIEW_ROW,
    main,
    validate_phase16_readiness,
)


def board_text(*, readiness: str = "x", review: str = " ", pilot: str = " ") -> str:
    rows = [f"- [{readiness}] `{row}` fixture" for row in PHASE16_READINESS_ROWS]
    rows.append(f"- [{review}] `{PHASE16_REVIEW_ROW}` fixture")
    rows.append(f"- [{pilot}] `{PHASE16_PILOT_ROW}` fixture")
    return "\n".join(rows) + "\n"


class Phase16ReadinessTests(unittest.TestCase):
    def test_product_path_is_rejected_until_independent_review_is_accepted(self) -> None:
        failures = validate_phase16_readiness(
            board_text(),
            ["crates/trust-runtime/src/stdlib/timers.rs"],
        )

        self.assertTrue(any(PHASE16_REVIEW_ROW in failure for failure in failures))
        self.assertTrue(any("timers.rs" in failure for failure in failures))

    def test_root_hmi_assets_are_product_paths(self) -> None:
        failures = validate_phase16_readiness(
            board_text(),
            ["hmi/overview.toml"],
        )

        self.assertTrue(any(PHASE16_REVIEW_ROW in failure for failure in failures))
        self.assertTrue(any("hmi/overview.toml" in failure for failure in failures))

    def test_vendored_code_and_root_dependency_manifests_are_product_paths(self) -> None:
        for path in (
            "third_party/tiverse-mmap/src/lib.rs",
            "Cargo.toml",
            "Cargo.lock",
        ):
            with self.subTest(path=path):
                failures = validate_phase16_readiness(board_text(), [path])

                self.assertTrue(
                    any(PHASE16_REVIEW_ROW in failure for failure in failures)
                )
                self.assertTrue(any(path in failure for failure in failures))

    def test_open_readiness_rows_keep_product_paths_blocked(self) -> None:
        failures = validate_phase16_readiness(
            board_text(readiness=" "),
            ["crates/trust-hir/src/lib.rs"],
        )

        for row in PHASE16_READINESS_ROWS:
            self.assertTrue(any(row in failure for failure in failures))

    def test_reviewed_readiness_allows_the_queued_product_vertical(self) -> None:
        self.assertEqual(
            validate_phase16_readiness(
                board_text(review="x"),
                ["crates/trust-runtime/src/stdlib/timers.rs"],
            ),
            [],
        )

    def test_verification_only_change_is_allowed_while_review_is_pending(self) -> None:
        self.assertEqual(
            validate_phase16_readiness(
                board_text(),
                ["scripts/verification/prover.py"],
            ),
            [],
        )

    def test_missing_duplicate_and_premature_pilot_rows_fail_closed(self) -> None:
        missing = board_text().replace(
            f"- [ ] `{PHASE16_REVIEW_ROW}` fixture\n", ""
        )
        duplicate = board_text() + f"- [ ] `{PHASE16_REVIEW_ROW}` duplicate\n"
        premature = board_text(pilot="x")

        self.assertTrue(any("exactly once" in failure for failure in validate_phase16_readiness(missing, [])))
        self.assertTrue(any("exactly once" in failure for failure in validate_phase16_readiness(duplicate, [])))
        self.assertTrue(any(PHASE16_PILOT_ROW in failure for failure in validate_phase16_readiness(premature, [])))

    def test_cli_returns_nonzero_for_a_blocked_product_change(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "board.md"
            path.write_text(board_text())

            status = main(
                [
                    "--board",
                    str(path),
                    "--changed-file=editors/vscode/src/extension.ts",
                ]
            )

        self.assertEqual(status, 1)


if __name__ == "__main__":
    unittest.main()
