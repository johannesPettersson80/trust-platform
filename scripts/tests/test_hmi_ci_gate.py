"""Specification 43 section 9: execute the configured release report with HMI failures."""
from pathlib import Path
import json
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class HmiReleaseGateTest(unittest.TestCase):
    def run_report(self, status="success", include_marker=True):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        report_job = workflow.split("  release-gate-report:", 1)[1]
        gates = re.findall(r"--required-gate ([\w-]+)", report_job)
        jobs = re.findall(r'--job-status "([\w-]+)=', report_job)
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            artifacts = base / "artifacts"
            artifacts.mkdir()
            for gate in gates:
                if gate != "gate-hmi-screen-tests" or include_marker:
                    (artifacts / gate).mkdir()
            args = [sys.executable, str(ROOT / "scripts/generate_release_gate_report.py"),
                    "--output-dir", str(base / "report"), "--gate-artifacts-dir", str(artifacts)]
            for gate in gates:
                args.extend(["--required-gate", gate])
            for job in jobs:
                args.extend(["--job-status", job + "=" + (status if job == "hmi-screen-tests" else "success")])
            result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
            report = json.loads((base / "report/release-gate-report.json").read_text())
            return result.returncode, report

    def test_hmi_failure_cancellation_or_skip_prevents_release_success(self):
        for status in ["failure", "cancelled", "skipped", ""]:
            with self.subTest(status=status):
                code, report = self.run_report(status=status)
                self.assertNotEqual(code, 0, report)
                self.assertIn("hmi-screen-tests", report["failed_jobs"])

    def test_missing_hmi_artifact_prevents_release_success(self):
        code, report = self.run_report(include_marker=False)
        self.assertNotEqual(code, 0, report)
        self.assertIn("gate-hmi-screen-tests", report["missing_required_gate_artifacts"])

    def test_successful_hmi_evidence_allows_the_report_to_pass(self):
        code, report = self.run_report()
        self.assertEqual(code, 0, report)
        self.assertEqual(report["verdict"], "PASS")


if __name__ == "__main__":
    unittest.main()
