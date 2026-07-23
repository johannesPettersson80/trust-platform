#!/usr/bin/env python3
"""Run the focused malformed-input contract for mutation manifest refresh."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromName(
        "scripts.verification.mutation_program_cli_tests."
        "MutationProgramCliTests."
        "test_unsafe_source_path_is_rejected_without_mutating_manifest"
    )
    raise SystemExit(not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful())
