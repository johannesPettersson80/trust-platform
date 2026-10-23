from pathlib import Path
import re
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]
RENDER_SCRIPT = ROOT / "scripts" / "render_diagrams.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "diagrams.yml"


class DiagramWorkflowContractTests(unittest.TestCase):
    def test_renderer_pins_the_full_environment_and_fails_closed(self) -> None:
        source = RENDER_SCRIPT.read_text()

        self.assertRegex(
            source,
            r'plantuml/plantuml@sha256:[0-9a-f]{64}',
        )
        self.assertNotIn("plantuml/plantuml:latest", source)
        self.assertNotIn("/releases/latest/", source)
        self.assertIn("PLANTUML_JAR_SHA256=", source)
        self.assertIn("PLANTUML_ALLOW_HOST_RENDERER", source)
        self.assertIn("noncanonical host renderer", source)
        self.assertIn("requires Docker or Podman", source)

        subprocess.run(["bash", "-n", str(RENDER_SCRIPT)], check=True)

    def test_workflow_verifies_in_prs_without_pushing(self) -> None:
        source = WORKFLOW.read_text()

        self.assertRegex(source, r"(?m)^  pull_request:\s*$")
        self.assertRegex(
            source,
            r"(?ms)^permissions:\s*\n  contents: read\s*$",
        )
        self.assertIn("Verify generated diagrams are current", source)
        self.assertIn("git status --porcelain --untracked-files=all", source)
        self.assertNotIn("contents: write", source)
        self.assertNotIn("git-auto-commit", source)
        self.assertNotRegex(source, re.compile(r"(?m)^\s*git push(?:\s|$)"))


if __name__ == "__main__":
    unittest.main()
