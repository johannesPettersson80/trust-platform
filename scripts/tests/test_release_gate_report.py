"""Release Gate Report and the jobs that change detection skips (specification 22, "Pull-request
checks and change detection"). Each test runs the script as CI's Release Gate Report runs it."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "generate_release_gate_report.py"


def report(markers: list[str], *args: str) -> tuple[int, dict]:
    """The script's exit code and JSON report, with gate markers for `markers` present."""
    with tempfile.TemporaryDirectory() as folder:
        base = Path(folder)
        for marker in markers:
            (base / "gates" / marker).mkdir(parents=True)
            (base / "gates" / marker / "marker.txt").write_text("ok\n")
        (base / "gates").mkdir(exist_ok=True)
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--output-dir", str(base / "report"),
             "--gate-artifacts-dir", str(base / "gates"), *args],
            capture_output=True, text=True,
        )
        written = json.loads((base / "report/release-gate-report.json").read_text())
        return result.returncode, written


class SkippedJobTests(unittest.TestCase):
    def test_a_job_the_detection_skipped_passes_without_its_markers(self) -> None:
        # Catches a pull request that changes only the extension failing the report for the three
        # Test jobs and Conformance that the change detection rightly skipped.
        code, written = report(
            ["gate-fmt"],
            "--required-gate", "gate-fmt",
            "--required-gate", "gate-test-ubuntu-latest",
            "--required-gate", "gate-conformance",
            "--job-status", "fmt=success",
            "--job-status", "test=skipped",
            "--job-status", "conformance=skipped",
            "--skipped-job", "test", "--skipped-job", "conformance",
            "--skipped-gate", "gate-test-ubuntu-latest", "--skipped-gate", "gate-conformance",
        )
        self.assertEqual(code, 0)
        self.assertEqual(written["verdict"], "PASS")
        self.assertEqual(
            written["skipped_by_change_detection"],
            {"jobs": ["conformance", "test"], "gates": ["gate-conformance", "gate-test-ubuntu-latest"]},
        )

    def test_a_skip_excuses_nothing_else(self) -> None:
        # Catches a skip that hides a failure: a skipped job that ran and failed, a job skipped
        # without the detection naming it, and another job's missing marker all still fail.
        code, written = report(
            [],
            "--required-gate", "gate-fmt",
            "--required-gate", "gate-conformance",
            "--job-status", "test=failure",
            "--job-status", "clippy=skipped",
            "--job-status", "conformance=skipped",
            "--skipped-job", "test", "--skipped-job", "conformance",
            "--skipped-gate", "gate-conformance",
        )
        self.assertEqual(code, 1)
        self.assertEqual(written["verdict"], "FAIL")
        self.assertEqual(written["failed_jobs"], ["clippy", "test"])
        self.assertEqual(written["missing_required_gate_artifacts"], ["gate-fmt"])


if __name__ == "__main__":
    unittest.main()
