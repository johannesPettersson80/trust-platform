"""Tests for the standalone ignored-test staleness checker."""

from __future__ import annotations

import io
import subprocess
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

from scripts.verification.ignored_test_staleness import (
    IgnoredTestStalenessResult,
    blocking_discovery_failures,
    main,
    validate_live_ignored_test_registry,
)
from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.ignored_test_models import InventoryDiagnostic


class IgnoredTestStalenessTests(unittest.TestCase):
    def test_live_repository_has_no_unknown_ignored_test_debt(self) -> None:
        result = validate_live_ignored_test_registry(ROOT)

        self.assertEqual((), result.failures)
        self.assertEqual(0, result.unknown)

    def test_unresolved_skip_warning_is_an_incomplete_inventory_failure(self) -> None:
        failures = blocking_discovery_failures(
            [
                InventoryDiagnostic(
                    "warning",
                    "dynamic_playwright_skip",
                    "scripts/captures/example.spec.mjs",
                    10,
                    "skip has no stable identity",
                )
            ]
        )

        self.assertEqual(len(failures), 1)
        self.assertIn("dynamic_playwright_skip", failures[0])

    def test_root_wrapper_imports_and_exposes_cli(self) -> None:
        root = Path(__file__).resolve().parents[2]
        completed = subprocess.run(
            [sys.executable, "scripts/check_ignored_test_staleness.py", "--help"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("--root", completed.stdout)

    def test_unknown_debt_is_reported_without_failing(self) -> None:
        result = IgnoredTestStalenessResult(
            failures=(),
            discovered=88,
            registered=88,
            unknown=63,
            catalog_mapped=0,
        )
        output = io.StringIO()
        with patch(
            "scripts.verification.ignored_test_staleness.validate_live_ignored_test_registry",
            return_value=result,
        ), redirect_stdout(output):
            exit_code = main(["--root", str(Path.cwd())])

        self.assertEqual(exit_code, 0)
        self.assertIn("88 discovered", output.getvalue())
        self.assertIn("63 unknown (report-only)", output.getvalue())

    def test_malformed_or_stale_registry_fails(self) -> None:
        result = IgnoredTestStalenessResult(
            failures=("DISC_MISSING has no registry record",),
            discovered=88,
            registered=87,
            unknown=63,
            catalog_mapped=0,
        )
        output = io.StringIO()
        with patch(
            "scripts.verification.ignored_test_staleness.validate_live_ignored_test_registry",
            return_value=result,
        ), redirect_stderr(output):
            exit_code = main([])

        self.assertEqual(exit_code, 1)
        self.assertIn("ignored-test staleness validation failed", output.getvalue())
        self.assertIn("DISC_MISSING", output.getvalue())


if __name__ == "__main__":
    unittest.main()
