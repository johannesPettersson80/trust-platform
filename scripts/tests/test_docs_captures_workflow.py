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


if __name__ == "__main__":
    unittest.main()
