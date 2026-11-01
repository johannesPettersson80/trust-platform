"""The Release workflow and main's version guard (specification 24, "Release rehearsal on pull
requests", "Release after main CI" and "Main-push version alignment"). CI's Python has no YAML
parser, so the tests read the workflows' text, which keeps its indentation."""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_version_release_evidence as verifier  # noqa: E402
import release_plan  # noqa: E402

RELEASE = ROOT / ".github" / "workflows" / "release.yml"
RELEASE_FILES = ROOT / ".github" / "actions" / "release-files" / "action.yml"
CI = ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_ONLY = "${{ needs.plan.outputs.mode == 'release' }}"
# The steps that make a release public, in the order they run.
PUBLISHING = ["Tag the release", "Create GitHub Release", "Publish to VS Code Marketplace", "Verify the release"]


def jobs(workflow: str) -> dict[str, str]:
    """Each job's text by its id."""
    body = workflow.split("\njobs:\n", 1)[1]
    parts = re.split(r"(?m)^  ([a-z0-9_-]+):\n", body)
    return dict(zip(parts[1::2], parts[2::2]))


def step_texts(job: str) -> list[str]:
    """Each step's text in order, its first line indented as the others."""
    return ["        " + step for step in re.split(r"(?m)^      - ", job)[1:]]


def steps(job: str) -> dict[str, str]:
    """Each named step's text by its name."""
    found = {}
    for text in step_texts(job):
        name = re.search(r"(?m)^        name: (.+)$", text)
        if name:
            found[name.group(1)] = text
    return found


def run_block(step: str) -> str:
    """A step's `run: |` script, dedented as the runner sees it."""
    body = step.split("        run: |\n", 1)[1]
    return "\n".join(line[10:] for line in body.splitlines()) + "\n"


def field(text: str, key: str, indent: int = 8) -> str:
    match = re.search(rf"(?m)^{' ' * indent}{key}: (.+)$", text)
    return match.group(1) if match else ""


def needs(job: str) -> set[str]:
    value = field(job, "needs", 4)
    return {name.strip() for name in value.strip("[]").split(",") if name.strip()}


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.text = RELEASE.read_text(encoding="utf-8")
        self.jobs = jobs(self.text)

    def test_a_release_follows_main_ci_and_no_tag_push_starts_one(self) -> None:
        # Catches a release that waits for a hand-pushed tag: 0.25.1's took two days of tag and CI
        # rounds (27 to 29 October 2026).
        triggers = self.text.split("\non:\n", 1)[1].split("\n\npermissions:", 1)[0]
        self.assertIn("\n  pull_request:\n", "\n" + triggers)
        self.assertIn("\n  merge_group:\n", "\n" + triggers)
        self.assertIn(
            "  workflow_run:\n    workflows: [CI]\n    types: [completed]\n    branches: [main]\n", triggers
        )
        self.assertIn("  workflow_dispatch:", triggers)
        self.assertNotIn("tags:", triggers)
        self.assertNotRegex(triggers, r"(?m)^  push:")

    def test_the_plan_alone_decides_what_runs(self) -> None:
        # Catches a job that runs when the plan skips, or that the plan's mode cannot stop.
        plan = self.jobs["plan"]
        self.assertIn("python3 scripts/release_plan.py", plan)
        self.assertEqual(field(self.jobs["release-preflight"], "if", 4), "${{ needs.plan.outputs.mode != 'skip' }}")
        for job_id, job in self.jobs.items():
            if job_id in {"plan", "release-preflight", "conclusion"}:
                continue
            self.assertIn("plan", needs(job), job_id)
            self.assertIn("release-preflight", needs(job), job_id)

    def test_the_plan_never_runs_a_forks_code(self) -> None:
        # Catches a plan that runs the code of the commit it plans for: after CI of a fork's pull
        # request that commit is the fork's, and the plan's token and outputs decide a release. A
        # fork's own pull request makes `github.sha` its merge commit, so the plan runs the base then
        # (Codex's review of #146). The checkout keeps no credentials, and only the run after main's
        # CI gets the token.
        checkouts = [text for text in step_texts(self.jobs["plan"]) if "uses: actions/checkout@" in text]
        self.assertEqual(len(checkouts), 1)
        self.assertIn(
            "ref: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name"
            " != github.repository && github.event.pull_request.base.sha || github.sha }}",
            checkouts[0],
        )
        self.assertIn("persist-credentials: false", checkouts[0])
        self.assertNotIn("head_sha", checkouts[0])
        self.assertNotIn("head.sha", checkouts[0])
        self.assertIn("GITHUB_TOKEN: ${{ github.event_name == 'workflow_run' && github.token || '' }}", self.jobs["plan"])

    def test_every_job_checks_out_the_plans_commit(self) -> None:
        # Catches a job of a release after CI that builds main's newest commit instead of the one
        # whose CI passed, or a rehearsal that builds the pull request's merge commit.
        for job_id, job in self.jobs.items():
            if job_id in {"plan", "conclusion"}:
                continue
            checkouts = [text for text in step_texts(job) if "uses: actions/checkout@" in text]
            if job_id == "openot-real-databases":
                self.assertEqual(checkouts, [])
                self.assertIn("    with:\n      ref: ${{ needs.plan.outputs.sha }}\n", job)
                continue
            self.assertTrue(checkouts, job_id)
            for text in checkouts:
                self.assertIn("ref: ${{ needs.plan.outputs.sha }}", text, job_id)

    def test_only_a_release_tags_and_publishes_and_in_this_order(self) -> None:
        # Catches a rehearsal that publishes, and a Marketplace upload before the release exists.
        publish = steps(self.jobs["publish"])
        order = list(publish)
        self.assertEqual([name for name in order if name in PUBLISHING], PUBLISHING)
        for name in PUBLISHING:
            self.assertEqual(field(publish[name], "if"), RELEASE_ONLY, name)
        for job_id, job in self.jobs.items():
            if job_id == "publish":
                continue
            for needle in ("vsce publish", "action-gh-release", "git push", "git tag"):
                self.assertNotIn(needle, job, job_id)

    def test_the_planner_reads_the_verification_by_the_names_it_runs_under(self) -> None:
        # Catches a renamed job or step: the plan would then find no verified run and re-run every
        # release from the start, publishing a complete one again.
        publish = self.jobs["publish"]
        self.assertEqual(field(publish, "name", 4), release_plan.VERIFY_JOB)
        self.assertEqual(list(steps(publish))[-1], release_plan.VERIFY_STEP)

    def test_the_release_follows_every_build_and_test(self) -> None:
        # Catches a tag created while a platform's build or a test job still runs or has failed
        # (0.25.0's release jobs failed after its tag, 25 October 2026), and a rehearsal that keeps
        # files before every build and test passed.
        others = set(self.jobs) - {"rehearse", "publish", "conclusion"}
        self.assertEqual(needs(self.jobs["publish"]), others)
        self.assertEqual(needs(self.jobs["rehearse"]), others)

    def test_a_missing_marketplace_token_fails_a_release_before_it_builds(self) -> None:
        require = steps(self.jobs["release-preflight"])["Require the Marketplace token"]
        self.assertEqual(field(require, "if"), RELEASE_ONLY)
        self.assertIn("exit 1", require)
        self.assertNotIn("Skip Marketplace publish", self.text)

    def test_the_tag_check_runs_for_a_release_only(self) -> None:
        # A rehearsal's commit is not on main yet; the tag check would refuse it.
        check = steps(self.jobs["release-preflight"])["Validate release tag"]
        self.assertEqual(field(check, "if"), RELEASE_ONLY)

    def test_the_release_is_named_by_the_plans_tag_and_commit(self) -> None:
        # Catches a release named by the event's ref or commit: after CI they are main's newest.
        for job_id, job in self.jobs.items():
            if job_id == "plan":
                continue
            for needle in ("github.ref_name", "GITHUB_REF_NAME", "github.sha", "GITHUB_SHA"):
                self.assertNotIn(needle, job, job_id)
        publish = self.jobs["publish"]
        self.assertIn("      RELEASE_TAG: ${{ needs.plan.outputs.tag }}\n", publish)
        self.assertIn("      RELEASE_SHA: ${{ needs.plan.outputs.sha }}\n", publish)
        files = RELEASE_FILES.read_text(encoding="utf-8")
        self.assertEqual(files.count('--commit "${RELEASE_SHA}"'), 2)
        for needle in ("github.sha", "GITHUB_SHA", "github.ref_name", "GITHUB_REF_NAME"):
            self.assertNotIn(needle, files)
        for job_id in ("rehearse", "publish"):
            write = steps(self.jobs[job_id])["Write the release files"]
            self.assertIn("uses: ./.github/actions/release-files", write, job_id)
            self.assertIn("sha: ${{ needs.plan.outputs.sha }}", write, job_id)
            self.assertIn("tag: ${{ needs.plan.outputs.tag }}", write, job_id)
        release = steps(publish)["Create GitHub Release"]
        self.assertIn("tag_name: ${{ env.RELEASE_TAG }}", release)
        self.assertIn("target_commitish: ${{ env.RELEASE_SHA }}", release)

    def test_a_rerun_continues_with_the_tag_on_the_same_commit(self) -> None:
        tag = steps(self.jobs["publish"])["Tag the release"]
        self.assertIn('if [ "${peeled}" != "${RELEASE_SHA}" ]', tag)
        self.assertIn('git tag -a "${RELEASE_TAG}"', tag)
        self.assertIn("already published", steps(self.jobs["publish"])["Publish to VS Code Marketplace"])

    def test_a_failed_marketplace_upload_fails_its_step(self) -> None:
        # Catches the step that ended green after vsce failed: `exit_code=$?` after an `if` whose condition failed
        # read 0, so the release waited an hour in its verification for a target never uploaded (Codex, PR #146).
        # The step runs here with a stand-in `vsce` that answers each case as the Marketplace tool does.
        script = run_block(steps(self.jobs["publish"])["Publish to VS Code Marketplace"])
        cases = [
            ("ERROR  The Personal Access Token verification has failed.", 1, 1),
            ("ERROR  trust-platform.trust-lsp@0.27.0 already exists.", 1, 0),
            ("DONE  Published trust-platform.trust-lsp v0.27.0.", 0, 0),
        ]
        for answer, vsce_status, step_status in cases:
            with tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                (root / "vsix-artifacts").mkdir()
                (root / "vsix-artifacts" / "trust-lsp-linux-x64.vsix").write_text("vsix")
                (root / "bin").mkdir()
                vsce = root / "bin" / "vsce"
                vsce.write_text(f"#!/bin/sh\necho '{answer}'\nexit {vsce_status}\n")
                vsce.chmod(0o755)
                result = subprocess.run(
                    ["bash", "-c", script],
                    cwd=root,
                    env={**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}"},
                    capture_output=True,
                    text=True,
                    timeout=60,
                )
                self.assertEqual(result.returncode, step_status, f"{answer}: {result.stdout}{result.stderr}")

    def test_a_rehearsal_keeps_its_release_text_and_evidence(self) -> None:
        keep = steps(self.jobs["rehearse"])["Keep the rehearsal's release files"]
        self.assertIn("name: release-rehearsal-${{ needs.plan.outputs.tag }}", keep)
        self.assertIn("          path: |\n            release-notes.md\n            release-evidence/\n", keep)

    def test_a_rehearsal_holds_no_token_that_writes(self) -> None:
        # Catches a rehearsal whose candidate code, or what it installs, could read the Marketplace token
        # or write with the GitHub token (Codex's review of #146): it runs only for a rehearsal, reads, and
        # keeps no credential in its checkout.
        rehearse = self.jobs["rehearse"]
        self.assertIn("needs.plan.outputs.mode == 'rehearsal'", field(rehearse, "if", 4))
        self.assertIn("    permissions:\n      contents: read\n      actions: read\n", rehearse)
        self.assertNotIn("secrets.", rehearse)
        checkout = [text for text in step_texts(rehearse) if "uses: actions/checkout@" in text][0]
        self.assertIn("persist-credentials: false", checkout)
        self.assertEqual(field(self.jobs["publish"], "if", 4), RELEASE_ONLY)

    def test_only_the_marketplace_steps_get_the_marketplace_token(self) -> None:
        # Catches the token in a job's environment, where every step of the candidate reads it.
        self.assertEqual(self.text.count("secrets.VSCE_PAT"), 2)
        self.assertIn("secrets.VSCE_PAT", steps(self.jobs["release-preflight"])["Require the Marketplace token"])
        self.assertIn("secrets.VSCE_PAT", steps(self.jobs["publish"])["Publish to VS Code Marketplace"])
        self.assertNotIn("secrets.", RELEASE_FILES.read_text(encoding="utf-8"))

    def test_the_project_runner_never_runs_a_pull_request(self) -> None:
        # Catches a pull request's code on the project's own runner, also as a merge group, and secrets
        # handed to a workflow that needs none (Codex's review of #146).
        databases = self.jobs["openot-real-databases"]
        self.assertEqual(
            field(databases, "if", 4),
            "${{ github.event_name != 'pull_request' && github.event_name != 'merge_group' }}",
        )
        self.assertNotIn("secrets", databases)
        self.assertNotIn("secrets.", (ROOT / ".github" / "workflows" / "openot-real-databases.yml").read_text())

    def test_the_verification_outlives_its_marketplace_wait(self) -> None:
        # A job that GitHub stops first fails the release while the Marketplace still lists it.
        limit = int(field(self.jobs["publish"], "timeout-minutes", 4))
        self.assertGreaterEqual(limit, verifier.DEFAULT_MARKETPLACE_WAIT_SECONDS / 60 + 30)

    def test_one_conclusion_fails_on_any_failed_or_cancelled_job(self) -> None:
        # Catches a run that shows success while a job failed, and one that fails when the plan
        # skipped every job.
        conclusion = self.jobs["conclusion"]
        self.assertEqual(needs(conclusion), set(self.jobs) - {"conclusion"})
        self.assertEqual(field(conclusion, "if", 4), "${{ !cancelled() }}")
        self.assertIn('all(.[]; .result == "success" or .result == "skipped")', conclusion)

    def test_releases_run_one_at_a_time_and_a_push_replaces_a_rehearsal(self) -> None:
        concurrency = self.text.split("\nconcurrency:\n", 1)[1].split("\n\njobs:", 1)[0]
        self.assertIn("'release-main'", concurrency)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", concurrency)

    def test_the_release_workflow_does_not_wait_on_ci(self) -> None:
        # It starts after CI instead, and builds its own conformance result.
        self.assertNotIn("ci_run_id", self.text)
        self.assertNotIn("gh run download", self.text)
        self.assertIn("name: Runtime Validation", self.text)
        self.assertIn("name: release-conformance", self.text)


class MainPushAlignmentTests(unittest.TestCase):
    def test_the_ci_version_guard_checks_alignment_and_waits_for_nothing(self) -> None:
        # Catches the wait for a hand-tagged release, which failed main's CI after its 90 minutes
        # while 0.25.1's release still ran (29 October 2026).
        guard = jobs(CI.read_text(encoding="utf-8"))["version-release-guard"]
        self.assertIn("python3 scripts/check_release_version_alignment.py", guard)
        self.assertNotIn("check_version_release_evidence", guard)
        self.assertLessEqual(int(field(guard, "timeout-minutes", 4)), 15)


if __name__ == "__main__":
    unittest.main()
