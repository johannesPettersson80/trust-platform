"""CI runs every Python test module of `scripts/tests` (specification 22, "CI reliability and release-gate
artifact contract").

CI runs these modules from steps that name each one, in the workflows and in the scripts their jobs run, such as
`scripts/supply_chain_gate.sh`. A module that nothing names never runs: on 13 November 2026 five of the 34 ran
nowhere, and the comms conformance gate's had failed unseen since 11 November, when CI's package installs moved into
`scripts/ci_apt_install.sh`.

Wrong behaviour caught: a test module under `scripts/tests` that no workflow, and no script a workflow runs, names."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
TESTS = ROOT / "scripts" / "tests"


def ci_text() -> str:
    """The text of every workflow and of every script under `scripts/` that a workflow names."""
    workflows = [path.read_text(encoding="utf-8") for path in sorted(WORKFLOWS.glob("*.yml"))]
    named = sorted(set(re.findall(r"scripts/[\w./-]+\.(?:sh|py)", "\n".join(workflows))))
    scripts = [(ROOT / name).read_text(encoding="utf-8") for name in named if (ROOT / name).is_file()]
    return "\n".join(workflows + scripts)


class CiRunsEveryTestModuleTests(unittest.TestCase):
    def test_a_workflow_or_a_script_it_runs_names_every_test_module(self) -> None:
        text = ci_text()
        modules = sorted(path.stem for path in TESTS.glob("test_*.py"))
        # A search that finds no module would pass without checking anything.
        self.assertIn("test_workflow_caches", modules)
        unrun = [module for module in modules if not re.search(rf"\bscripts[./]tests[./]{module}\b", text)]
        self.assertEqual(unrun, [])


if __name__ == "__main__":
    unittest.main()
