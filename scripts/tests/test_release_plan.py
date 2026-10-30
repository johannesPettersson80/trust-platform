"""The Release workflow's plan (specification 24, "Release rehearsal on pull requests" and "Release
after main CI"). Each expected mode follows from the event and the repository state the test gives."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import release_plan  # noqa: E402

OFFICIAL = "johannesPettersson80/trust-platform"
VERSIONS = {"base": "0.26.0", "same": "0.26.0", "bumped": "0.27.0", "released": "0.26.0", "new": "0.27.0"}


def plan(**kwargs):
    defaults = dict(
        repository=OFFICIAL,
        official=OFFICIAL,
        version_at=lambda sha: VERSIONS[sha],
        tag_commit=lambda tag: None,
        release_complete=lambda tag: False,
    )
    defaults.update(kwargs)
    return release_plan.plan(**defaults)


def after_ci(**kwargs):
    run = dict(
        event="workflow_run",
        run_conclusion="success",
        run_event="push",
        run_branch="main",
        run_sha="new",
        run_head_repository=OFFICIAL,
    )
    run.update(kwargs)
    return plan(**run)


class ReleasePlanTests(unittest.TestCase):
    def test_a_pull_request_is_rehearsed_only_when_it_changes_the_version(self) -> None:
        # Catches a rehearsal of every pull request (hours of release builds for nothing) and a
        # version change merged without one (0.25.0's release jobs first ran after its tag).
        kept = plan(event="pull_request", pr_base="base", pr_head="same", pr_head_repository=OFFICIAL)
        self.assertEqual((kept.mode, kept.sha), ("skip", "same"))
        bumped = plan(event="pull_request", pr_base="base", pr_head="bumped", pr_head_repository=OFFICIAL)
        self.assertEqual((bumped.mode, bumped.sha, bumped.tag), ("rehearsal", "bumped", "v0.27.0"))

    def test_a_forks_pull_request_does_not_run_on_the_projects_runners(self) -> None:
        fork = plan(event="pull_request", pr_base="base", pr_head="bumped", pr_head_repository="someone/fork")
        self.assertEqual(fork.mode, "skip")

    def test_a_manual_run_rehearses_and_never_releases(self) -> None:
        self.assertEqual(plan(event="workflow_dispatch", dispatch_sha="bumped").mode, "rehearsal")

    def test_main_ci_releases_a_version_without_a_tag(self) -> None:
        # Catches a release that waits for a hand-pushed tag (0.25.1's guard expired waiting).
        result = after_ci()
        self.assertEqual((result.mode, result.sha, result.tag), ("release", "new", "v0.27.0"))

    def test_only_a_successful_ci_run_of_a_main_push_in_the_official_repository_releases(self) -> None:
        # Catches a release from failed or cancelled CI, from a branch, from a fork's copy of the
        # workflow, or after CI of another repository's commits named main.
        self.assertEqual(after_ci(run_conclusion="failure").mode, "skip")
        self.assertEqual(after_ci(run_conclusion="cancelled").mode, "skip")
        self.assertEqual(after_ci(run_event="pull_request").mode, "skip")
        self.assertEqual(after_ci(run_branch="work/next").mode, "skip")
        self.assertEqual(after_ci(repository="someone/fork").mode, "skip")
        self.assertEqual(after_ci(run_head_repository="someone/fork").mode, "skip")

    def test_a_version_tagged_on_another_commit_was_released_before(self) -> None:
        # Every push to main after a release keeps its version: that must release nothing.
        result = after_ci(run_sha="released", tag_commit=lambda tag: ("tag", "older"))
        self.assertEqual((result.mode, result.tag), ("skip", "v0.26.0"))

    def test_a_rerun_resumes_an_incomplete_release_and_skips_a_complete_one(self) -> None:
        # Catches a re-run that cannot finish a release whose Marketplace step failed after the tag,
        # and one that publishes a complete release again.
        on_this = lambda tag: ("tag", "new")
        self.assertEqual(after_ci(tag_commit=on_this).mode, "release")
        self.assertEqual(after_ci(tag_commit=on_this, release_complete=lambda tag: True).mode, "skip")

    def test_a_release_is_complete_only_once_a_run_of_its_commit_verified_it(self) -> None:
        # Catches the re-run that skips a release whose Marketplace publishing failed: the GitHub
        # release and its evidence exist from the step before, but no run passed the verification
        # (Codex, PR #146). Each answer is what GitHub's Actions API gives for the runs of the commit
        # and for the jobs of each run's attempts.
        def job(verify: str) -> dict:
            return {"name": "Publish", "steps": [{"name": "Create GitHub Release", "conclusion": "success"},
                                                  {"name": "Verify the release", "conclusion": verify}]}

        def api(runs: dict[int, list[dict]]):
            def get(path: str, token: str) -> dict:
                self.assertEqual(token, "token")
                if "/actions/workflows/release.yml/runs?head_sha=new&" in path:
                    return {"workflow_runs": [{"id": run} for run in runs]}
                run = int(path.split("/actions/runs/")[1].split("/")[0])
                self.assertIn("filter=all", path)
                return {"jobs": runs[run]}
            return get

        complete = lambda runs: release_plan.release_complete(OFFICIAL, "new", "token", get=api(runs))
        self.assertFalse(complete({}), "no Release run of the commit")
        self.assertFalse(complete({7: [job("failure")]}), "the Marketplace step or its verification failed")
        self.assertFalse(complete({7: [job("skipped")]}), "a rehearsal of the commit verifies nothing")
        self.assertFalse(complete({7: [{"name": "Build", "steps": [{"name": "Verify the release", "conclusion": "success"}]}]}))
        self.assertTrue(complete({7: [job("failure"), job("success")]}), "a later attempt verified it")
        self.assertTrue(complete({6: [job("skipped")], 7: [job("success")]}), "another run verified it")

    def test_a_lightweight_tag_on_the_commit_fails_the_plan(self) -> None:
        with self.assertRaises(ValueError):
            after_ci(tag_commit=lambda tag: ("commit", "new"))

    def test_ls_remote_lines_tell_annotated_from_lightweight_tags(self) -> None:
        annotated = "aaa\trefs/tags/v0.27.0\nbbb\trefs/tags/v0.27.0^{}\n"
        self.assertEqual(release_plan.parse_tag_listing(annotated, "v0.27.0"), ("tag", "bbb"))
        self.assertEqual(release_plan.parse_tag_listing("ccc\trefs/tags/v0.27.0\n", "v0.27.0"), ("commit", "ccc"))
        self.assertIsNone(release_plan.parse_tag_listing("ddd\trefs/tags/v0.27.01\n", "v0.27.0"))
        self.assertIsNone(release_plan.parse_tag_listing("", "v0.27.0"))


if __name__ == "__main__":
    unittest.main()
