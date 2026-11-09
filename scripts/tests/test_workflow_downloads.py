"""Every download in the workflows is tried again on a transient failure (G106; specification 22, "Pull-request checks and
change detection"). GitHub's release server answered 504 to the actionlint download once and failed a Format job that had
no fault of its own. Wrong behaviour caught: a `curl` in a workflow without `--retry`."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"


def curl_lines_without_retry() -> list[str]:
    """Each line of a workflow that runs `curl` without `--retry`, as `file:line: text`."""
    offenders = []
    for workflow in sorted(WORKFLOWS.glob("*.yml")):
        for number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), start=1):
            text = line.strip()
            if text.startswith("#") or not re.search(r"(^|[\s;&|(])curl\s", text):
                continue
            if "--retry" not in text:
                offenders.append(f"{workflow.name}:{number}: {text}")
    return offenders


class WorkflowDownloadsTest(unittest.TestCase):
    def test_every_curl_in_the_workflows_retries_a_transient_failure(self) -> None:
        self.assertEqual(curl_lines_without_retry(), [])

    def test_the_check_finds_a_curl_without_retry(self) -> None:
        # The rule's own red: a download written as main wrote the actionlint download before G106.
        line = 'curl -fsSL -o "${RUNNER_TEMP}/${archive}" \\'
        self.assertIsNotNone(re.search(r"(^|[\s;&|(])curl\s", line))
        self.assertNotIn("--retry", line)


if __name__ == "__main__":
    unittest.main()
