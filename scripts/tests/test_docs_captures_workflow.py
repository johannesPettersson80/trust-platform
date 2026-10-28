from pathlib import Path
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "docs-captures.yml"


class DocsCapturesWorkflowContractTests(unittest.TestCase):
    def test_pull_requests_run_read_only_captures_without_refresh_pr_writes(self) -> None:
        workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)

        triggers = workflow["on"]
        self.assertIn("pull_request", triggers)
        self.assertEqual(
            triggers["pull_request"]["paths"],
            triggers["push"]["paths"],
            "PR and main-push capture paths must stay aligned",
        )
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        self.assertIn(
            "scripts/tests/test_capture_lifecycle.py",
            triggers["pull_request"]["paths"],
        )
        self.assertIn(
            "crates/trust-runtime/src/web/ui/**",
            triggers["pull_request"]["paths"],
            "runtime Web UI changes must execute the rendered capture suite",
        )

        refresh = workflow["jobs"]["refresh"]
        self.assertEqual(refresh["permissions"], {"contents": "read"})
        refresh_steps = [step.get("name") for step in refresh["steps"]]
        self.assertIn("Verify capture process lifecycle", refresh_steps)
        self.assertNotIn("Create capture refresh PR", refresh_steps)

        refresh_pr = workflow["jobs"]["create-refresh-pr"]
        self.assertEqual(refresh_pr["needs"], "refresh")
        self.assertEqual(refresh_pr["if"], "github.event_name != 'pull_request'")
        self.assertEqual(
            refresh_pr["permissions"],
            {"contents": "write", "pull-requests": "write"},
        )
        refresh_pr_steps = [step.get("name") for step in refresh_pr["steps"]]
        self.assertIn("Download refreshed captures", refresh_pr_steps)
        self.assertIn("Create capture refresh PR", refresh_pr_steps)

    def test_captures_build_programs_with_the_toolchain_the_job_builds(self) -> None:
        # The browser IDE and the terminal captures build Structured Text programs. Without a
        # toolchain a build downloads its version's, which does not exist before that version is
        # published: the browser IDE's build failed on 0.25.1's pull request (28 October 2026).
        workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)
        steps = workflow["jobs"]["refresh"]["steps"]
        names = [step.get("name") for step in steps]
        toolchain = names.index("Toolchain for compiled programs (specification 44 §4.6)")
        self.assertLess(toolchain, names.index("Refresh browser captures"))
        self.assertLess(toolchain, names.index("Refresh terminal captures"))
        self.assertIn("scripts/native/build_toolchain_libraries.sh", steps[toolchain]["run"])
        self.assertIn('echo "TRUST_TOOLCHAIN=${folder}" >> "${GITHUB_ENV}"', steps[toolchain]["run"])

        # The terminal captures run in a container, which sees the toolchain only when it is
        # handed over.
        runner = (ROOT / "scripts" / "captures" / "run-terminal-captures.sh").read_text()
        self.assertIn('-e "TRUST_TOOLCHAIN=$TRUST_TOOLCHAIN"', runner)
        self.assertIn('-v "$TRUST_TOOLCHAIN:$TRUST_TOOLCHAIN:ro"', runner)
        self.assertIn('"${toolchain_args[@]}"', runner)


if __name__ == "__main__":
    unittest.main()
