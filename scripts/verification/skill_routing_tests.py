"""Phase 15 locks for repository test-authoring skill routing."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class SkillRoutingTests(unittest.TestCase):
    def test_shared_test_authoring_skill_is_concise_and_routes_to_program_contracts(self) -> None:
        path = ROOT / ".codex/skills/trust-test-authoring/SKILL.md"
        text = path.read_text()

        self.assertLessEqual(len(text.splitlines()), 180)
        for marker in (
            "scripts/plan_tests.py",
            "missing_tests",
            "scripts/check_test_catalog_staleness.py",
            "scripts/check_vscode_test_registration.py",
            "scripts/prove.py red",
            "scripts/prove.py green",
            "spec_gap",
            "behavior-lock",
            "hardware",
            "docs/internal/testing/checklists/plc-verification-program",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, text)

        agents = (ROOT / "AGENTS.md").read_text()
        for marker in (
            "scripts/plan_tests.py",
            "missing_tests",
            "scripts/check_test_catalog_staleness.py",
            "scripts/check_vscode_test_registration.py",
            "device-in-the-loop",
        ):
            with self.subTest(agents_marker=marker):
                self.assertIn(marker, agents)

    def test_agents_and_domain_skills_route_behavior_changes_to_shared_contract(self) -> None:
        expected = {
            "AGENTS.md": "trust-test-authoring",
            ".codex/skills/st-lsp-solid/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-architecture-automation/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-remote-builder/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-hmi-contracts/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-vscode-quality/SKILL.md": "trust-test-authoring",
            ".codex/skills/vscode-ui-acceptance/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-ci-release-gates/SKILL.md": "trust-test-authoring",
        }
        for relative, marker in expected.items():
            with self.subTest(path=relative):
                self.assertIn(marker, (ROOT / relative).read_text())

    def test_skill_metadata_routes_the_eight_reviewed_scenarios(self) -> None:
        text = (ROOT / ".codex/skills/trust-test-authoring/SKILL.md").read_text()
        for scenario in (
            "bug fix",
            "refactor",
            "malformed input",
            "VS Code",
            "runtime safety",
            "hardware lab",
            "docs-only",
            "supply-chain",
        ):
            with self.subTest(scenario=scenario):
                self.assertIn(scenario, text)


if __name__ == "__main__":
    unittest.main()
