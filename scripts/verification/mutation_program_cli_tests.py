"""Executable entrypoint tests for the Phase 10 mutation report."""

from __future__ import annotations

import subprocess
import unittest

from .metadata_validator.constants import ROOT


class MutationProgramCliTests(unittest.TestCase):
    def test_documented_entrypoints_load_from_repository_root(self) -> None:
        for script in (
            "scripts/report_mutation_program.py",
            "scripts/validate_mutation_program_report.py",
        ):
            result = subprocess.run(
                ["python3", script, "--help"],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stderr)


if __name__ == "__main__":
    unittest.main()
