#!/usr/bin/env python3
"""Native regression tests for Cargo integration-test binary path portability."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME_TESTS = ROOT / "crates" / "trust-runtime" / "tests"
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
COMPILE_TIME_BINARY_PATH = re.compile(
    r'(?:option_)?env!\("CARGO_BIN_EXE_trust-runtime"\)'
)


class RuntimeTestBinaryPathContractTests(unittest.TestCase):
    def test_runtime_integration_tests_do_not_require_compile_time_binary_path(self) -> None:
        offenders: list[str] = []
        for path in sorted(RUNTIME_TESTS.rglob("*.rs")):
            if COMPILE_TIME_BINARY_PATH.search(path.read_text(encoding="utf-8")):
                offenders.append(str(path.relative_to(ROOT)))

        self.assertEqual(
            offenders,
            [],
            "runtime integration tests must resolve binary paths from the execution environment: "
            + ", ".join(offenders),
        )

    def test_ci_runs_runtime_binary_path_contract_before_clippy(self) -> None:
        workflow = CI_WORKFLOW.read_text(encoding="utf-8")
        contract_command = "python3 ./scripts/runtime_test_binary_path_contract_tests.py"
        self.assertIn(contract_command, workflow)
        self.assertLess(workflow.index(contract_command), workflow.index("  clippy:"))


if __name__ == "__main__":
    unittest.main()
