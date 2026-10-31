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

    def test_every_pull_request_workflow_runs_in_the_merge_queue_as_on_a_pull_request(self) -> None:
        # Specification 22, "Change and release workflow": the merge queue runs a pull request's checks
        # again on the merged result. Catches a workflow that never runs for a merge group (a required
        # check from it holds the queue until the queue gives up), and a job or step that runs for a pull
        # request but not for a merge group, or the other way round.
        for path in WORKFLOWS:
            workflow = load(path)
            triggers = workflow.get("on", {})
            if not (isinstance(triggers, dict) and "pull_request" in triggers):
                continue
            self.assertIn("merge_group", triggers, f"{path.name} does not run in the merge queue")
            for job_id, job in workflow.get("jobs", {}).items():
                conditions = [job.get("if", "")] + [step.get("if", "") for step in job.get("steps", [])]
                for condition in conditions:
                    if "pull_request" in condition:
                        self.assertIn("merge_group", condition, f"{path.name} {job_id}: {condition}")

    def test_each_required_check_is_one_job_that_runs_on_pull_requests_and_in_the_merge_queue(self) -> None:
        # Catches a required check that no job reports (a renamed job blocks every pull request), one that
        # two jobs report (Render Diagrams and Docs Captures both named theirs "Conclusion"), and one from a
        # workflow that does not run in the merge queue.
        required = [
            line.strip()
            for line in (ROOT / ".github" / "required-checks.txt").read_text().splitlines()
            if line.strip() and not line.startswith("#")
        ]
        self.assertTrue(required)
        reported: dict[str, list[str]] = {}
        for path in WORKFLOWS:
            workflow = load(path)
            for job_id, job in workflow.get("jobs", {}).items():
                reported.setdefault(job.get("name", job_id), []).append(path.name)
        for check in required:
            self.assertEqual(len(reported.get(check, [])), 1, f"{check!r} is reported by {reported.get(check)}")
            triggers = load(ROOT / ".github" / "workflows" / reported[check][0]).get("on", {})
            self.assertIn("pull_request", triggers, check)
            self.assertIn("merge_group", triggers, check)

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
