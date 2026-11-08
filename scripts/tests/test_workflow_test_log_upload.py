"""A native test job's log upload never fails a job whose tests passed, and a failed upload is tried once more (G107;
specification 22, "CI reliability and release-gate artifact contract"). A runner's DNS failure (`ENOTFOUND`) failed a macOS
Test job after all its tests had passed. Wrong behaviour caught: a log upload without `continue-on-error`, and no second
attempt after a failed upload."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CI = ROOT / ".github" / "workflows" / "ci.yml"


def step(text: str, name: str) -> str:
    """The lines of the workflow step named `name`, up to the next step of the same indentation."""
    match = re.search(rf"^(\s*)- name: {re.escape(name)}\n", text, re.MULTILINE)
    if match is None:
        return ""
    indent = match.group(1)
    rest = text[match.end():]
    end = re.search(rf"^{indent}- |^\S", rest, re.MULTILINE)
    return match.group(0) + (rest[: end.start()] if end else rest)


class TestLogUploadTest(unittest.TestCase):
    def test_a_failed_upload_does_not_fail_the_job(self) -> None:
        first = step(CI.read_text(encoding="utf-8"), "Upload test suite log")
        self.assertIn("uses: actions/upload-artifact", first)
        self.assertIn("id: test-log", first)
        self.assertIn("continue-on-error: true", first)

    def test_a_failed_upload_is_tried_once_more(self) -> None:
        second = step(CI.read_text(encoding="utf-8"), "Upload test suite log (second attempt)")
        self.assertIn("uses: actions/upload-artifact", second)
        self.assertIn("steps.test-log.outcome == 'failure'", second)
        self.assertIn("continue-on-error: true", second)
        self.assertIn("overwrite: true", second)

    def test_the_check_finds_an_upload_that_fails_the_job(self) -> None:
        # The rule's own red: the step as main wrote it before G107.
        before = """      - name: Upload test suite log
        if: ${{ always() }}
        uses: actions/upload-artifact@v7
        with:
          name: test-log-${{ matrix.os }}
      - name: Mark gate success
"""
        self.assertNotIn("continue-on-error: true", step(before, "Upload test suite log"))
        self.assertEqual(step(before, "Upload test suite log (second attempt)"), "")


if __name__ == "__main__":
    unittest.main()
