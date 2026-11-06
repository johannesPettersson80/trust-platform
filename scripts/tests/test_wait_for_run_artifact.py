"""The Release workflow's package builds wait only for their own platform's toolchain bundle (specification 24;
checklist W43): `scripts/wait_for_run_artifact.py` and the build job that runs it."""

from __future__ import annotations

import sys
import unittest
import urllib.error
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import wait_for_run_artifact as waiting  # noqa: E402

BUNDLE = "toolchain-darwin-arm64"
JOB = "Toolchain bundle (darwin-arm64)"


class FakeGitHub:
    """A run whose artifacts and jobs are answered from lists, one answer per request, the last one repeated."""

    def __init__(self, artifacts: list, jobs: list) -> None:
        self.artifacts = list(artifacts)
        self.jobs = list(jobs)
        self.sleeps = 0
        self.now = 0.0

    def get(self, path: str, token: str) -> dict:
        assert token == "token"
        answers = self.artifacts if "/artifacts?" in path else self.jobs
        answer = answers.pop(0) if len(answers) > 1 else answers[0]
        if isinstance(answer, Exception):
            raise answer
        return answer

    def sleep(self, seconds: float) -> None:
        self.sleeps += 1
        self.now += seconds

    def clock(self) -> float:
        return self.now

    def wait(self, timeout: float = 600) -> str | None:
        return waiting.wait(
            "owner/repo", "42", BUNDLE, JOB, "token", timeout, get=self.get, sleep=self.sleep, clock=self.clock
        )


def artifacts(*names: str) -> dict:
    return {"total_count": len(names), "artifacts": [{"name": name, "expired": False} for name in names]}


def job(status: str, conclusion: str | None = None) -> dict:
    return {"jobs": [{"name": JOB, "status": status, "conclusion": conclusion}]}


class WaitTests(unittest.TestCase):
    def test_a_build_takes_its_bundle_once_the_bundle_job_uploads_it(self) -> None:
        # Catches a build that fails or packages without the bundle while its bundle job still runs.
        github = FakeGitHub([artifacts(), artifacts(), artifacts(BUNDLE)], [job("in_progress")])
        self.assertIsNone(github.wait())
        self.assertEqual(github.sleeps, 2)

    def test_a_bundle_already_uploaded_needs_no_wait(self) -> None:
        github = FakeGitHub([artifacts("toolchain-linux-x64", BUNDLE)], [job("in_progress")])
        self.assertIsNone(github.wait())
        self.assertEqual(github.sleeps, 0)

    def test_another_platforms_bundle_is_not_this_ones(self) -> None:
        # Catches a darwin-arm64 build that takes the linux-x64 bundle for its own.
        github = FakeGitHub([artifacts("toolchain-linux-x64"), artifacts(BUNDLE)], [job("in_progress")])
        self.assertIsNone(github.wait())
        self.assertEqual(github.sleeps, 1)

    def test_a_build_stops_at_once_when_its_bundle_job_failed(self) -> None:
        # Catches a build that waits out its timeout, an hour and a half, for a bundle that will never come.
        github = FakeGitHub([artifacts()], [job("completed", "failure")])
        problem = github.wait()
        self.assertEqual(problem, f"{JOB} ended failure without uploading {BUNDLE}")
        self.assertEqual(github.sleeps, 0)

    def test_a_bundle_uploaded_as_its_job_ends_is_taken(self) -> None:
        # The upload is listed only after the job's success: the artifact is asked for once more before giving up.
        github = FakeGitHub([artifacts(), artifacts(BUNDLE)], [job("completed", "success")])
        self.assertIsNone(github.wait())

    def test_the_wait_runs_out(self) -> None:
        github = FakeGitHub([artifacts()], [job("queued")])
        self.assertEqual(github.wait(timeout=60), f"{BUNDLE} was not uploaded within 60 seconds")
        self.assertEqual(github.sleeps, 3)

    def test_an_unanswered_request_is_asked_again(self) -> None:
        # Catches one failed API call failing a release build.
        github = FakeGitHub([urllib.error.URLError("reset"), artifacts(BUNDLE)], [job("in_progress")])
        self.assertIsNone(github.wait())
        self.assertEqual(github.sleeps, 1)


class WorkflowTests(unittest.TestCase):
    def test_each_build_starts_beside_the_bundles_and_waits_for_its_own(self) -> None:
        # Catches every build waiting for every toolchain bundle again (6 November 2026: 49 minutes for the slowest),
        # and a build that packages its extension before its platform's bundle is uploaded.
        workflow = yaml.safe_load((ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8"))
        build = workflow["jobs"]["build"]
        self.assertNotIn("toolchain-bundle", build["needs"])
        names = [step.get("name", "") for step in build["steps"]]
        wait = names.index("Wait for this platform's toolchain bundle")
        carry = names.index("Carry the toolchain bundle (specification 44 §4.6)")
        self.assertEqual(carry, wait + 1, "the bundle is carried right after its wait")
        run = build["steps"][wait]["run"]
        self.assertIn('--artifact "toolchain-${{ matrix.platform }}"', run)
        self.assertIn('--job "Toolchain bundle (${{ matrix.platform }})"', run)
        bundle_job = workflow["jobs"]["toolchain-bundle"]
        self.assertEqual(bundle_job["name"], "Toolchain bundle (${{ matrix.platform }})", "the job the wait names")
        self.assertIn(
            "name: toolchain-${{ matrix.platform }}",
            (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8"),
        )
        # The jobs that publish or keep the packages still need every bundle and every build.
        for name in ("rehearse", "publish"):
            needs = workflow["jobs"][name]["needs"]
            self.assertIn("toolchain-bundle", needs, name)
            self.assertIn("build", needs, name)


if __name__ == "__main__":
    unittest.main()
