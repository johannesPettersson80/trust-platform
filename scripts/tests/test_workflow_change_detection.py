"""Every workflow's pull-request checks run on large changes too (specification 22, "Pull-request
checks and change detection")."""

from __future__ import annotations

import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = sorted((ROOT / ".github" / "workflows").glob("*.yml"))


def load(path: Path) -> dict:
    return yaml.load(path.read_text(), Loader=yaml.BaseLoader)


class WorkflowChangeDetectionTests(unittest.TestCase):
    def test_no_workflow_filters_pull_requests_or_main_pushes_by_path(self) -> None:
        # Catches a trigger path filter coming back: GitHub compares only the first 300 changed
        # files, so 0.25.0's 10,252-file pull request ran no documentation workflow.
        for path in WORKFLOWS:
            triggers = load(path).get("on", {})
            for event in ("pull_request", "push"):
                trigger = triggers.get(event) if isinstance(triggers, dict) else None
                if isinstance(trigger, dict):
                    self.assertNotIn("paths", trigger, f"{path.name} filters {event} by path")
                    self.assertNotIn("paths-ignore", trigger, f"{path.name} filters {event} by path")

    def test_a_workflow_with_change_detection_concludes_on_every_job(self) -> None:
        # Catches a conclusion that skips a job (its failure would not count), one that does not run
        # after a failure (a skipped conclusion reads as passed), and a changes job left out of it (a
        # failed detection would skip every check and pass).
        checking_pull_requests = [
            path for path in WORKFLOWS
            if "changes" in load(path).get("jobs", {}) and "pull_request" in load(path).get("on", {})
        ]
        self.assertTrue(checking_pull_requests)
        for path in checking_pull_requests:
            jobs = load(path)["jobs"]
            conclusion = jobs.get("conclusion")
            self.assertIsNotNone(conclusion, f"{path.name} has no conclusion job")
            needs = conclusion["needs"]
            needs = [needs] if isinstance(needs, str) else needs
            checks = {name for name in jobs if name not in {"conclusion"}}
            checks -= {name for name, job in jobs.items() if job.get("if") == "github.event_name != 'pull_request'"}
            self.assertEqual(set(needs), checks, f"{path.name}'s conclusion must need every check")
            self.assertEqual(conclusion["if"], "${{ !cancelled() }}", path.name)
            self.assertIn("all(.result == \"success\" or .result == \"skipped\")", conclusion["steps"][0]["run"])

    def test_every_listed_path_file_belongs_to_a_workflow(self) -> None:
        # Catches a path list no workflow reads, or a workflow reading a missing one.
        listed = {path.name for path in (ROOT / ".github" / "paths").glob("*.txt")}
        used = set()
        for path in WORKFLOWS:
            text = path.read_text()
            for name in listed:
                if f".github/paths/{name}" in text:
                    used.add(name)
        self.assertEqual(listed, used)


if __name__ == "__main__":
    unittest.main()
