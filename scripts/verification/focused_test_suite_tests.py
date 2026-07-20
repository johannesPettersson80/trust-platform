"""Tests for the canonical verification-tooling Python suite."""

from __future__ import annotations

import tempfile
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from scripts.verification.focused_test_suite import (
    discover_test_modules,
    run_focused_test_suite,
)
from scripts.verification.metadata_validator.constants import ROOT


class FocusedTestSuiteTests(unittest.TestCase):
    def test_discovery_is_recursive_deterministic_and_excludes_production_modules(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            verification = root / "scripts/verification"
            (verification / "metadata_validator").mkdir(parents=True)
            for relative in (
                "zeta_tests.py",
                "alpha_tests.py",
                "metadata_validator/evidence_proof_tests.py",
                "metadata_validator/ignored_tests.py",
                "helper.py",
            ):
                (verification / relative).write_text("\n")

            modules = discover_test_modules(root)

        self.assertEqual(
            modules,
            [
                "scripts.verification.alpha_tests",
                "scripts.verification.metadata_validator.evidence_proof_tests",
                "scripts.verification.zeta_tests",
            ],
        )

    def test_live_suite_includes_phase3_review_fix_regressions(self) -> None:
        modules = set(discover_test_modules(ROOT))

        self.assertIn("scripts.verification.ignored_test_js_skip_lexical_tests", modules)
        self.assertIn("scripts.verification.ignored_test_source_contract_tests", modules)

    def test_runner_propagates_a_discovered_test_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            test_path = root / "scripts/verification/failing_tests.py"
            test_path.parent.mkdir(parents=True)
            test_path.write_text("\n")
            result = Mock()
            result.wasSuccessful.return_value = False
            runner = Mock()
            runner.run.return_value = result

            with (
                patch(
                    "scripts.verification.focused_test_suite.unittest.defaultTestLoader.loadTestsFromNames",
                    return_value=Mock(),
                ),
                patch(
                    "scripts.verification.focused_test_suite.unittest.TextTestRunner",
                    return_value=runner,
                ),
            ):
                passed = run_focused_test_suite(root)

        self.assertFalse(passed)

    def test_direct_entrypoint_loads_from_repository_root(self) -> None:
        result = subprocess.run(
            [sys.executable, "scripts/run_verification_focused_tests.py", "--list"],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("scripts.verification.ignored_test_js_skip_lexical_tests", result.stdout)


if __name__ == "__main__":
    unittest.main()
