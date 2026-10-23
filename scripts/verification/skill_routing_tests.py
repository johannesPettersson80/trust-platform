"""Locks for the repository's agent rulebook and test-authoring skill routing."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SKILLS = ROOT / ".codex/skills"


def flat(path: Path) -> str:
    """The file's text with line wrapping removed, so a reflowed sentence still matches."""
    return " ".join(path.read_text().split())


class SkillRoutingTests(unittest.TestCase):
    def test_shared_test_authoring_skill_is_concise_and_routes_to_program_contracts(self) -> None:
        path = SKILLS / "trust-test-authoring/SKILL.md"
        self.assertLessEqual(len(path.read_text().splitlines()), 180)
        text = flat(path)
        for marker in (
            "written specification",
            "native executable test",
            "behavior-lock",
            "hardware",
            "docs/internal/testing/checklists/plc-verification-program",
            "Never derive product work",
            "Planner, catalog, denominator, and evidence tooling",
            "nonblocking maintenance and cannot invent product requirements or tests",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, text)
        self.assertNotIn("scripts/plan_tests.py", text)

        agents = flat(ROOT / "AGENTS.md")
        for marker in (
            "scripts/check_vscode_test_registration.py",
            "device-in-the-loop",
        ):
            with self.subTest(agents_marker=marker):
                self.assertIn(marker, agents)

    def test_agents_and_domain_skills_route_behavior_changes_to_shared_contract(self) -> None:
        expected = {
            "AGENTS.md": "trust-test-authoring",
            ".codex/skills/st-lsp-solid/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-hmi-contracts/SKILL.md": "trust-test-authoring",
            ".codex/skills/trust-vscode-quality/SKILL.md": "trust-test-authoring",
        }
        for relative, marker in expected.items():
            with self.subTest(path=relative):
                self.assertIn(marker, flat(ROOT / relative))
        agents = flat(ROOT / "AGENTS.md")
        self.assertIn("written specification", agents)
        self.assertIn("native executable test", agents)
        self.assertIn("cannot create product work", agents)
        self.assertNotIn("`unmapped`) block", agents)
        self.assertNotIn("uncataloged-test rejection", agents)

    def test_skill_metadata_routes_the_eight_reviewed_scenarios(self) -> None:
        text = flat(SKILLS / "trust-test-authoring/SKILL.md")
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

    def test_every_skill_is_well_formed_and_listed_in_agents(self) -> None:
        agents = (ROOT / "AGENTS.md").read_text()
        # The shared rulebook grows when approved workflow requirements change.
        # Pin the safety instructions themselves, not their wrapping or line count.
        for heading in (
            "## Hard rule: all logic is in Rust",
            "## How to work",
            "## Working in several checkouts",
            "## Releases",
        ):
            with self.subTest(required_section=heading):
                self.assertIn(heading, agents)
        for skill in sorted(path for path in SKILLS.iterdir() if path.is_dir()):
            with self.subTest(skill=skill.name):
                text = (skill / "SKILL.md").read_text()
                self.assertTrue(text.startswith("---\n"))
                header = text.split("---\n", 2)[1]
                fields = dict(line.split(": ", 1) for line in header.strip().splitlines())
                self.assertEqual(fields.get("name"), skill.name)
                self.assertRegex(skill.name, r"^[a-z0-9-]{1,64}$")
                description = fields.get("description", "")
                self.assertTrue(0 < len(description) <= 1024, "a skill description holds 1 to 1024 characters")
                self.assertNotRegex(description, r"[<>]")
                self.assertLessEqual(len(text.splitlines()), 500)
                self.assertIn(f"`{skill.name}`", agents, "AGENTS.md lists every skill")


if __name__ == "__main__":
    unittest.main()
