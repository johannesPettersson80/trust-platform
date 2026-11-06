"""The Release workflow's plan (specification 24, "Release rehearsal on pull requests" and "Release
after main CI"). Each expected mode follows from the event and the repository state the test gives."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import release_plan  # noqa: E402

OFFICIAL = "johannesPettersson80/trust-platform"
VERSIONS = {"base": "0.26.0", "same": "0.26.0", "bumped": "0.27.0", "released": "0.26.0", "new": "0.27.0"}
# 0.30.0's rehearsal (run 41707764199, artifact 13281794816) built the head 36ecae479, whose tree 474f2f24 its release
# commit bd30e34ac shares.
TREE = "474f2f242b2039236ffb0116fbafc66bae0b811e"
REHEARSED = "36ecae4797a0f81e64197fc6bc568cb4166f0d67"
REHEARSAL = release_plan.Rehearsal(run_id=41707764199, artifact_id=13281794816, commit=REHEARSED)
BUNDLE = f"release-bundle-v0.27.0-{TREE}"
# The workflow of that pull request's run came from its test merge commit refs/pull/223/merge, 5634aa2f, which merged
# the head into main's e6f92cd33 and has the head's tree.
MERGE = "5634aa2f8e2e62273f34b582c8f7011b17d067f7"
BASE = "e6f92cd33d357c1b72b502891811866b8cf83e4b"
CALLED = f"{OFFICIAL}/.github/workflows/openot-real-databases.yml"


def plan(**kwargs):
    defaults = dict(
        repository=OFFICIAL,
        official=OFFICIAL,
        version_at=lambda sha: VERSIONS[sha],
        tag_commit=lambda tag: None,
        release_complete=lambda tag: False,
        version_commits=lambda sha: [],
        changes=lambda base, head, paths: False,
        merge_base=lambda base, head: base,
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


def pushed(**kwargs):
    run = dict(event="push", push_ref="refs/heads/main", push_sha="new", find_rehearsal=lambda version, sha: REHEARSAL)
    run.update(kwargs)
    return plan(**run)


def publish_job(verify: str) -> dict:
    return {"name": "Publish", "steps": [{"name": "Create GitHub Release", "conclusion": "success"},
                                         {"name": "Verify the release", "conclusion": verify}]}


def runs_api(test: unittest.TestCase, by_head: dict[int, list[dict]], after_ci: list[dict] | None = None,
             names: dict[int, str] | None = None):
    """GitHub's Actions API for the Release runs of the commit "new": the runs whose head is "new", the runs after CI
    since the given day with their names, and the jobs of each run's attempts."""
    jobs = dict(by_head)
    for run in after_ci or []:
        jobs[run["id"]] = run.pop("jobs")

    def get(path: str, token: str) -> dict:
        test.assertEqual(token, "token")
        if "/actions/workflows/release.yml/runs?head_sha=new&" in path:
            return {"workflow_runs": [{"id": run, "event": "push", "display_title": (names or {}).get(run, "")}
                                      for run in by_head]}
        if "/actions/workflows/release.yml/runs?event=workflow_run&created=%3E%3D2026-11-03&" in path:
            return {"workflow_runs": after_ci or []}
        run = int(path.split("/actions/runs/")[1].split("/")[0])
        test.assertIn("filter=all", path)
        return {"jobs": jobs[run]}
    return get


def rehearsal_run(**changes) -> dict:
    """GitHub's record of 0.30.0's rehearsal run, which passed."""
    run = {
        "id": 41707764199,
        "path": ".github/workflows/release.yml",
        "event": "pull_request",
        "status": "completed",
        "conclusion": "success",
        "head_sha": REHEARSED,
        "head_branch": "release/0.30.0",
        "head_commit": {"id": REHEARSED, "tree_id": TREE},
        "repository": {"full_name": OFFICIAL},
        "head_repository": {"full_name": OFFICIAL},
        "referenced_workflows": [{"path": f"{CALLED}@{MERGE}", "ref": "refs/pull/223/merge", "sha": MERGE}],
    }
    run.update(changes)
    return run


def manual_rehearsal_run(**changes) -> dict:
    """A maintainer's manual rehearsal of the same head on its branch: its workflow commit is its head."""
    called = [{"path": f"{CALLED}@{REHEARSED}", "ref": "refs/heads/release/0.30.0", "sha": REHEARSED}]
    return rehearsal_run(**{"event": "workflow_dispatch", "referenced_workflows": called, **changes})


def branch_run(event: str, branch: str) -> dict:
    """A run of `event` on `branch` whose workflow record is at that branch and the run's head, as a manual run's is."""
    return rehearsal_run(event=event, head_branch=branch, referenced_workflows=[
        {"path": f"{CALLED}@{REHEARSED}", "ref": f"refs/heads/{branch}", "sha": REHEARSED}])


def commit_record(sha: str, tree: str = TREE, parents: tuple[str, ...] = (BASE,)) -> dict:
    return {"sha": sha, "tree": {"sha": tree}, "parents": [{"sha": parent} for parent in parents]}


COMMITS = {MERGE: commit_record(MERGE, parents=(BASE, REHEARSED)), REHEARSED: commit_record(REHEARSED)}


def pull_request(**changes) -> dict:
    """GitHub's record of pull request 223, which brought the rehearsed head into main."""
    pull = {"number": 223, "state": "closed", "merged_at": "2026-11-04T00:13:03Z",
            "head": {"sha": REHEARSED, "ref": "release/0.30.0", "repo": {"full_name": OFFICIAL}},
            "base": {"ref": "main", "repo": {"full_name": OFFICIAL}}}
    pull.update(changes)
    return pull


def ci_run(**changes) -> dict:
    """GitHub's record of a passed CI run of the rehearsed head."""
    run = {"id": 41707700000, "path": ".github/workflows/ci.yml", "status": "completed", "conclusion": "success",
           "head_commit": {"id": REHEARSED, "tree_id": TREE}, "repository": {"full_name": OFFICIAL},
           "head_repository": {"full_name": OFFICIAL}}
    run.update(changes)
    return run


def bundle_artifact(artifact_id: int = 13281794816, run_id: int = 41707764199, **changes) -> dict:
    artifact = {"id": artifact_id, "name": BUNDLE, "expired": False, "workflow_run": {"id": run_id}}
    artifact.update(changes)
    return artifact


def artifacts_api(test: unittest.TestCase, artifacts: list[dict], runs: dict[int, dict],
                  commits: dict[str, dict] | None = None, ci: list[dict] | None = None, pulls: list[dict] | None = None):
    """GitHub's API for the rehearsals of BUNDLE: the artifacts by name, each run, the commits, the pull requests and
    the CI runs of the rehearsed head, which pass unless the test gives others."""
    def get(path: str, token: str) -> dict | list:
        test.assertEqual(token, "token")
        if path.startswith(f"/repos/{OFFICIAL}/actions/artifacts?"):
            test.assertIn(f"name={BUNDLE}&", path)
            return {"artifacts": artifacts}
        if path.startswith(f"/repos/{OFFICIAL}/git/commits/"):
            return (COMMITS if commits is None else commits)[path.rsplit("/", 1)[1]]
        if path.startswith(f"/repos/{OFFICIAL}/commits/{REHEARSED}/pulls?"):
            return [pull_request()] if pulls is None else pulls
        if path.startswith(f"/repos/{OFFICIAL}/actions/workflows/ci.yml/runs?"):
            test.assertIn(f"head_sha={REHEARSED}&", path)
            return {"workflow_runs": [ci_run()] if ci is None else ci}
        return runs[int(path.rsplit("/", 1)[1])]
    return get


class ReleasePlanTests(unittest.TestCase):
    def setUp(self) -> None:
        # The plan waits between its questions about a commit's pull requests; the tests record the waits instead.
        self.waits: list[float] = []
        patcher = mock.patch.object(release_plan.time, "sleep", side_effect=self.waits.append)
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_a_pull_request_is_rehearsed_only_when_it_changes_the_version(self) -> None:
        # Catches a rehearsal of every pull request (hours of release builds for nothing) and a
        # version change merged without one (0.25.0's release jobs first ran after its tag).
        kept = plan(event="pull_request", pr_base="base", pr_head="same", pr_head_repository=OFFICIAL)
        self.assertEqual((kept.mode, kept.sha), ("skip", "same"))
        bumped = plan(event="pull_request", pr_base="base", pr_head="bumped", pr_head_repository=OFFICIAL)
        self.assertEqual((bumped.mode, bumped.sha, bumped.tag), ("rehearsal", "bumped", "v0.27.0"))

    def test_a_branch_from_before_a_version_bump_does_not_change_the_version(self) -> None:
        # Catches the plan of 6 November 2026 (W44): #281 branched from 0.31.0 before main's bump to 0.32.0, its head
        # differed from main's version, and it was rehearsed as 0.31.0 on every platform. The version a pull request
        # sets is compared with where it branched.
        stale = plan(
            event="pull_request",
            pr_base="bumped",
            pr_head="same",
            pr_head_repository=OFFICIAL,
            merge_base=lambda base, head: "base",
        )
        self.assertEqual(stale.mode, "skip")
        bumping = plan(
            event="pull_request",
            pr_base="released",
            pr_head="bumped",
            pr_head_repository=OFFICIAL,
            merge_base=lambda base, head: "base",
        )
        self.assertEqual((bumping.mode, bumping.version), ("rehearsal", "0.27.0"))

    def test_a_pull_request_that_keeps_the_version_builds_the_linux_package_only_when_it_changes_one(self) -> None:
        # Specification 24: a pull request that changes what the packages hold builds the linux-x64 package for its
        # acceptance journeys. Catches every pull request building all five targets (hours of runners), a change to the
        # packages merged without its journeys, and a documentation change that builds packages.
        package = lambda base, head, paths: paths.name == "package.txt"
        packaged = plan(event="pull_request", pr_base="base", pr_head="same", pr_head_repository=OFFICIAL, changes=package)
        self.assertEqual((packaged.mode, packaged.sha, packaged.platforms), ("package", "same", ("linux-x64",)))
        self.assertEqual(packaged.matrices(), {
            "toolchain_targets": [{"os": "ubuntu-latest", "platform": "linux-x64"}],
            "build_targets": [{"os": "ubuntu-latest", "target": "x86_64-unknown-linux-gnu", "platform": "linux-x64"}],
        })
        documents = plan(event="pull_request", pr_base="base", pr_head="same", pr_head_repository=OFFICIAL)
        self.assertEqual(documents.mode, "skip")

    def test_a_pull_request_that_changes_the_release_workflow_is_rehearsed_with_every_target(self) -> None:
        # Catches a change to the release machinery that merges without its rehearsal: until 1 November a pull request
        # that kept the version ran no release job, so the merge queue's changes to release.yml were untested.
        release = lambda base, head, paths: paths.name == "release.txt"
        rehearsed = plan(event="pull_request", pr_base="base", pr_head="same", pr_head_repository=OFFICIAL, changes=release)
        self.assertEqual((rehearsed.mode, rehearsed.platforms), ("rehearsal", release_plan.ALL_PLATFORMS))

    def test_a_rehearsal_and_a_release_build_every_platform(self) -> None:
        # Specification 24: the five platforms of the packages, each on its runner; only linux-arm64 cross-compiles.
        # Catches a release or a rehearsal that loses a platform, or a native build marked as cross.
        for result in (plan(event="pull_request", pr_base="base", pr_head="bumped", pr_head_repository=OFFICIAL), after_ci()):
            matrices = result.matrices()
            self.assertEqual(
                [entry["platform"] for entry in matrices["build_targets"]],
                ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64"],
                result.mode,
            )
            self.assertEqual([entry["platform"] for entry in matrices["build_targets"] if entry.get("cross")], ["linux-arm64"])
            self.assertEqual(
                [entry["os"] for entry in matrices["toolchain_targets"]],
                ["ubuntu-latest", "ubuntu-24.04-arm", "macos-15-intel", "macos-latest", "windows-latest"],
            )

    def test_a_run_that_builds_nothing_still_names_a_target(self) -> None:
        # GitHub refuses a matrix with nothing in it; the jobs of a skipped run are skipped whatever their matrix names.
        skipped = plan(event="merge_group")
        self.assertEqual(skipped.mode, "skip")
        self.assertTrue(all(skipped.matrices().values()))

    def test_the_lists_name_what_the_packages_hold_and_not_the_documentation(self) -> None:
        # Specification 24: the extension, the crates, the examples and the libraries are in the packages; pages,
        # specifications and agent rules are not. Catches a list that misses part of a package, or that builds packages
        # for a documentation change.
        import ci_changes

        def listed(name: str, path: str) -> bool:
            patterns = ci_changes.read_patterns((ROOT / ".github" / "paths" / name).read_text())
            return any(ci_changes.pattern_regex(pattern).match(path) for pattern in patterns)

        for path in ("editors/vscode/src/extension.ts", "crates/trust-runtime/src/lib.rs", "examples/hmi_tank_level/runtime.toml",
                     "libraries/oscat/README.md", "Cargo.lock", "rust-toolchain.toml"):
            self.assertTrue(listed("package.txt", path), path)
        for path in ("docs/public/index.md", "docs/specs/24-release-evidence.md", "AGENTS.md", ".codex/skills/x/SKILL.md", "README.md"):
            self.assertFalse(listed("package.txt", path), path)
            self.assertFalse(listed("release.txt", path), path)

    def test_a_forks_pull_request_does_not_run_on_the_projects_runners(self) -> None:
        fork = plan(event="pull_request", pr_base="base", pr_head="bumped", pr_head_repository="someone/fork")
        self.assertEqual(fork.mode, "skip")

    def test_a_manual_run_rehearses_and_never_releases(self) -> None:
        self.assertEqual(plan(event="workflow_dispatch", dispatch_sha="bumped").mode, "rehearsal")

    def test_a_merge_group_neither_rehearses_nor_releases(self) -> None:
        # Specification 22, "Change and release workflow": the pull request was rehearsed, and the
        # release follows main's CI on the commit the queue merged. Catches a merge group that
        # rehearses again (hours in the queue) or releases before main's CI passed.
        result = plan(event="merge_group", version_at=lambda sha: self.fail("a merge group reads no version"))
        self.assertEqual((result.mode, result.sha, result.tag), ("skip", "", ""))

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
        job = publish_job
        complete = lambda runs: release_plan.release_complete(
            OFFICIAL, "new", "token", since="2026-11-03", get=runs_api(self, runs)
        )
        self.assertFalse(complete({}), "no Release run of the commit")
        self.assertFalse(complete({7: [job("failure")]}), "the Marketplace step or its verification failed")
        self.assertFalse(complete({7: [job("skipped")]}), "a rehearsal of the commit verifies nothing")
        self.assertFalse(complete({7: [{"name": "Build", "steps": [{"name": "Verify the release", "conclusion": "success"}]}]}))
        self.assertTrue(complete({7: [job("failure"), job("success")]}), "a later attempt verified it")
        self.assertTrue(complete({6: [job("skipped")], 7: [job("success")]}), "another run verified it")

    def test_a_release_after_ci_counts_for_the_commit_its_name_gives(self) -> None:
        # Specification 24: GitHub gives a run after main's CI main's newest commit as its head, so the run of 0.30.0's
        # CI-passed c41008869 (41716540236, 3 November 2026) had the head e6f92cd33. Catches a completeness check that
        # looks only at the runs whose head is the commit: it would miss such a verified release and publish it again.
        named = lambda sha, verify, run_id=8: {"id": run_id, "display_title": f"Release after CI of {sha}",
                                               "jobs": [publish_job(verify)]}
        complete = lambda after_ci: release_plan.release_complete(
            OFFICIAL, "new", "token", since="2026-11-03", get=runs_api(self, {}, after_ci)
        )
        self.assertTrue(complete([named("new", "success")]))
        self.assertFalse(complete([named("new", "failure")]), "its verification failed")
        self.assertFalse(complete([named("other", "success")]), "it released another commit")

    def test_a_release_after_ci_never_counts_for_the_commit_that_was_mains_head(self) -> None:
        # The runs 41699252466 and 41695006229 had the head c41008869 while they ran after the CI of 55f0612c8 and
        # 3e2ba4f3c (3 November 2026). Catches a check that counts such a run for its head: when two version changes
        # merge close together, the first one's verified release would make the second look complete, and its Publish
        # job would refuse it.
        def get(path: str, token: str) -> dict:
            if "head_sha=new" in path:
                return {"workflow_runs": [{"id": 9, "event": "workflow_run", "display_title": "Release after CI of older"}]}
            if "event=workflow_run" in path:
                return {"workflow_runs": [{"id": 9, "display_title": "Release after CI of older"}]}
            return {"jobs": [publish_job("success")]}

        self.assertFalse(release_plan.release_complete(OFFICIAL, "new", "token", since="2026-11-03", get=get))
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(release_plan.check_unreleased(OFFICIAL, "new", "token", since="2026-11-03", get=get), 0)

    def test_the_completeness_check_reads_every_page_of_runs_after_ci_and_fails_when_it_cannot(self) -> None:
        # Catches a check that reads only the newest 100 runs after CI, and one that calls a release incomplete when it
        # could not read them all (it then publishes it again).
        pages = {1: [{"id": run, "display_title": "Release after CI of other"} for run in range(100)],
                 2: [{"id": 500, "display_title": "Release after CI of new"}]}

        def get(path: str, token: str) -> dict:
            if "head_sha=new" in path:
                return {"workflow_runs": []}
            if "event=workflow_run" in path:
                return {"workflow_runs": pages.get(int(path.rsplit("page=", 1)[1]), [])}
            return {"jobs": [publish_job("success")]}

        self.assertTrue(release_plan.release_complete(OFFICIAL, "new", "token", since="2026-11-03", get=get))

        # GitHub lists at most 1,000 runs for a filtered query: pages 1 to 10 hold 100 runs each and page 11 comes back
        # empty. Catches a check that takes the empty page 11 for the end of the list and calls the release incomplete.
        def capped(path: str, token: str) -> dict:
            if "event=workflow_run" not in path:
                return {"workflow_runs": []}
            page = int(path.rsplit("page=", 1)[1])
            return {"workflow_runs": [{"id": page * 1000 + run, "display_title": "x"} for run in range(100)]
                    if page <= 10 else []}

        with self.assertRaisesRegex(ValueError, "cannot tell whether new was released"):
            release_plan.release_complete(OFFICIAL, "new", "token", since="2026-11-03", get=capped)

    def test_only_a_push_run_counts_for_its_head(self) -> None:
        # Specification 24: besides the runs after main's CI by their name, only a run that a push of the commit to main
        # started counts by its head. A pull request's or a manual run's workflow comes from the code it runs, so it can
        # name a job Publish and pass a step "Verify the release". Catches a completeness check that such a run fools
        # into skipping a release.
        for event, counts in (("push", True), ("pull_request", False), ("workflow_dispatch", False)):
            def get(path: str, token: str) -> dict:
                if "head_sha=new" in path:
                    return {"workflow_runs": [{"id": 7, "event": event, "display_title": "Release after CI of new"}]}
                if "event=workflow_run" in path:
                    return {"workflow_runs": []}
                return {"jobs": [publish_job("success")]}

            self.assertEqual(
                release_plan.release_complete(OFFICIAL, "new", "token", since="2026-11-03", get=get), counts, event
            )

    def test_the_publish_job_refuses_a_release_that_a_run_verified(self) -> None:
        # Specification 24, "A release never happens twice": the Publish job asks once it holds the release
        # concurrency group. Catches a re-run of a promotion that publishes after the release after main's CI
        # completed the release, or the reverse.
        verified = runs_api(self, {}, [{"id": 8, "display_title": "Release after CI of new",
                                        "html_url": "https://github.com/o/r/actions/runs/8",
                                        "jobs": [publish_job("success")]}])
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(release_plan.check_unreleased(OFFICIAL, "new", "token", since="2026-11-03", get=verified), 1)
        self.assertIn("Release run https://github.com/o/r/actions/runs/8 already verified the release of new", errors.getvalue())
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(release_plan.check_unreleased(
                OFFICIAL, "new", "token", since="2026-11-03", get=runs_api(self, {7: [publish_job("failure")]})), 0)

    def test_a_push_promotes_the_release_whose_tree_a_passed_rehearsal_built(self) -> None:
        # Specification 24, "Release after main CI" (owner, 2 November 2026: build once). 0.29.0's and 0.30.0's
        # release commits had the tree of their rehearsed heads, yet main's CI tested them again for about two hours
        # and the release rebuilt every package for about one and a half. Catches a promotion that waits for main's
        # CI, one that builds the packages again, and one that does not name the rehearsal whose files it publishes.
        asked = []
        result = pushed(find_rehearsal=lambda version, sha: asked.append((version, sha)) or REHEARSAL)
        self.assertEqual(asked, [("0.27.0", "new")])
        self.assertEqual((result.mode, result.sha, result.tag, result.rehearsal), ("release", "new", "v0.27.0", REHEARSAL))
        self.assertEqual(result.platforms, (), "a promoted release builds no platform")
        self.assertIn("promotes the files of rehearsal run 41707764199", result.reason)

    def test_a_push_without_a_passed_rehearsal_of_its_tree_waits_for_main_ci(self) -> None:
        # Specification 24: a commit whose tree no passed rehearsal of its version built (a pull request merged between
        # the rehearsal and the release, the rehearsal failed, or it built another version) is released after main's
        # CI, as before. Catches a release from untested code.
        result = pushed(find_rehearsal=lambda version, sha: None)
        self.assertEqual((result.mode, result.sha, result.rehearsal), ("skip", "new", None))
        self.assertIn("v0.27.0 follows main's CI", result.reason)
        self.assertEqual(after_ci().mode, "release", "main's CI then releases it, building every package")
        self.assertIsNone(after_ci().rehearsal)

    def test_a_push_never_releases_a_version_twice(self) -> None:
        # Catches a promotion of a version that is complete or was released from an earlier commit: every push after a
        # release keeps its version. Neither looks for a rehearsal.
        # The lookups are recorded, not failed: an error of the lookup only means no promotion (coordinator, B).
        asked = []
        unused = lambda version, sha: asked.append((version, sha))
        complete = pushed(tag_commit=lambda tag: ("tag", "new"), release_complete=lambda tag: True, find_rehearsal=unused)
        self.assertEqual((complete.mode, complete.reason), ("skip", "v0.27.0 is released and complete"))
        earlier = pushed(push_sha="released", tag_commit=lambda tag: ("tag", "older"), find_rehearsal=unused)
        self.assertEqual(asked, [], "a released version looks for no rehearsal")
        self.assertEqual((earlier.mode, earlier.tag), ("skip", "v0.26.0"))
        resumed = pushed(tag_commit=lambda tag: ("tag", "new"))
        self.assertEqual((resumed.mode, resumed.rehearsal), ("release", REHEARSAL), "an incomplete promotion resumes")

    def test_main_ci_promotes_the_rehearsal_that_a_push_would(self) -> None:
        # Specification 24, review finding of 4 November 2026: a promotion that tagged, released and published some VSIX
        # packages and then failed was finished by a release after main's CI that built every package again; it
        # replaced every GitHub asset while the Marketplace kept the rehearsal's packages. Catches a release after CI
        # that rebuilds a commit whose tree a passed rehearsal built, and one that looks for a rehearsal of a version
        # that is released.
        asked = []
        result = after_ci(tag_commit=lambda tag: ("tag", "new"),
                          find_rehearsal=lambda version, sha: asked.append((version, sha)) or REHEARSAL)
        self.assertEqual(asked, [("0.27.0", "new")])
        self.assertEqual((result.mode, result.sha, result.rehearsal, result.platforms), ("release", "new", REHEARSAL, ()))
        self.assertIn("promotes the files of rehearsal run 41707764199", result.reason)
        built = after_ci(find_rehearsal=lambda version, sha: None)
        self.assertEqual((built.mode, built.rehearsal, built.platforms), ("release", None, release_plan.ALL_PLATFORMS),
                         "without a rehearsal of its tree, it builds every package")
        asked = []
        unused = lambda version, sha: asked.append((version, sha))
        complete = after_ci(tag_commit=lambda tag: ("tag", "new"), release_complete=lambda tag: True, find_rehearsal=unused)
        self.assertEqual(asked, [], "a complete release looks for no rehearsal")
        self.assertEqual(complete.mode, "skip")

    def test_main_ci_after_a_completed_promotion_releases_nothing(self) -> None:
        # Specification 24: the promotion runs for the push, so its run's head is the commit, and its Publish job's
        # verification makes the release complete. Catches a release after main's CI that publishes the promoted
        # version again, with packages built anew.
        complete = lambda tag: release_plan.release_complete(
            OFFICIAL, "new", "token", since="2026-11-03", get=runs_api(self, {41760000000: [publish_job("success")]})
        )
        result = after_ci(tag_commit=lambda tag: ("tag", "new"), release_complete=complete)
        self.assertEqual((result.mode, result.reason), ("skip", "v0.27.0 is released and complete"))

    def test_only_a_push_to_main_of_the_official_repository_promotes(self) -> None:
        # Catches a promotion from a branch push or from a fork's copy of the workflow.
        asked = []
        unused = lambda version, sha: asked.append((version, sha))
        self.assertEqual(pushed(push_ref="refs/heads/work/next", find_rehearsal=unused).mode, "skip")
        self.assertEqual(pushed(push_ref="refs/tags/v0.27.0", find_rehearsal=unused).mode, "skip")
        self.assertEqual(pushed(repository="someone/fork", find_rehearsal=unused).mode, "skip")
        self.assertEqual(asked, [], "only a push to main of the official repository looks for a rehearsal")

    def test_a_push_keeps_the_rules_of_a_release_after_main_ci(self) -> None:
        # Catches a promotion past a version whose release was dropped, or onto a lightweight tag.
        with self.assertRaises(ValueError):
            pushed(tag_commit=lambda tag: ("commit", "new"))
        with self.assertRaisesRegex(ValueError, "v0.27.0 at mid has no tag"):
            pushed(
                version_at=lambda sha: {"new": "0.28.0", "mid": "0.27.0", "released": "0.26.0"}[sha],
                tag_commit=lambda tag: {"v0.26.0": ("tag", "released")}.get(tag),
                version_commits=lambda sha: ["new", "mid", "released"],
            )

    def test_the_rehearsal_is_found_by_its_bundle_and_githubs_record_of_its_run(self) -> None:
        # Specification 24: GitHub lists the bundle named for the version and the tree, and its run passed. Catches a
        # finder that misses 0.30.0's rehearsal, which its release could have promoted.
        found = release_plan.find_rehearsal(
            OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(self, [bundle_artifact()], {41707764199: rehearsal_run()})
        )
        self.assertEqual(found, REHEARSAL)
        dispatched = release_plan.find_rehearsal(
            OFFICIAL, "0.27.0", TREE, "token",
            get=artifacts_api(self, [bundle_artifact()], {41707764199: manual_rehearsal_run()}),
        )
        self.assertEqual(dispatched, REHEARSAL, "a maintainer's manual rehearsal counts as well")

    def test_a_rehearsal_whose_workflow_or_ci_did_not_run_on_this_tree_is_not_promoted(self) -> None:
        # Specification 24, review finding of 4 November 2026: a pull request's run executes the workflow of its test
        # merge commit, not of its head, so a pull request into a branch with another Release workflow could keep a
        # forged release-bundle-v<version>-<tree>. The plan takes the workflow's commit from GitHub's record of the local
        # reusable workflow the run called, and wants a passed CI run of the same head on the tree. Each case would
        # promote files that no run of this tree's workflow kept, or that CI never passed.
        forged = dict(COMMITS, **{MERGE: commit_record(MERGE, tree="f" * 40, parents=(BASE, REHEARSED))})
        cases = {
            "a workflow from a merge commit of another tree": dict(commits=forged),
            "no record of the workflow's commit": dict(runs=rehearsal_run(referenced_workflows=[])),
            "this repository's workflow called at another ref": dict(runs=rehearsal_run(referenced_workflows=[
                {"path": f"{CALLED}@{MERGE}", "ref": "refs/heads/release/0.30.0", "sha": MERGE}])),
            "two records of the called workflow": dict(runs=rehearsal_run(referenced_workflows=[
                {"path": f"{CALLED}@{MERGE}", "ref": "refs/pull/223/merge", "sha": MERGE},
                {"path": f"{CALLED}@{BASE}", "ref": "refs/pull/223/merge", "sha": BASE}])),
            "a merge commit that does not merge the head": dict(
                commits=dict(COMMITS, **{MERGE: commit_record(MERGE, parents=(BASE, "e" * 40))})),
            "a manual run whose workflow is not its head's": dict(runs=manual_rehearsal_run(referenced_workflows=[
                {"path": f"{CALLED}@{MERGE}", "ref": "refs/heads/release/0.30.0", "sha": MERGE}])),
            "no passed CI run of the head": dict(ci=[]),
            "a CI run of another tree": dict(ci=[ci_run(head_commit={"id": REHEARSED, "tree_id": "f" * 40})]),
            "a CI run that failed": dict(ci=[ci_run(conclusion="failure")]),
            "a fork's CI run": dict(ci=[ci_run(head_repository={"full_name": "someone/fork"})]),
        }
        for case, given in cases.items():
            found = release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(
                self, [bundle_artifact()], {41707764199: given.get("runs", rehearsal_run())},
                commits=given.get("commits"), ci=given.get("ci")))
            self.assertIsNone(found, case)

    def test_a_rehearsal_that_did_not_build_and_test_this_tree_is_not_promoted(self) -> None:
        # Specification 24: only a passed rehearsal of this repository whose head commit has the tree, by GitHub's own
        # record, under the exact bundle name of this version and tree. Each case would publish packages that no passed
        # rehearsal built from this tree.
        cases = {
            "a failed rehearsal": ([bundle_artifact()], rehearsal_run(conclusion="failure")),
            "a rehearsal still running": ([bundle_artifact()], rehearsal_run(status="in_progress", conclusion=None)),
            "a cancelled rehearsal": ([bundle_artifact()], rehearsal_run(conclusion="cancelled")),
            # These three pass every other check: a workflow record at their own branch and head, CI passed.
            "a run after main's CI": ([bundle_artifact()], branch_run("workflow_run", "main")),
            "a push": ([bundle_artifact()], branch_run("push", "main")),
            "a merge group": ([bundle_artifact()], branch_run("merge_group", "gh-readonly-queue/main/pr-223")),
            "another workflow's artifact": ([bundle_artifact()], rehearsal_run(path=".github/workflows/ci.yml")),
            "a fork's run": ([bundle_artifact()], rehearsal_run(head_repository={"full_name": "someone/fork"})),
            "a head of another tree under this tree's name": (
                [bundle_artifact()], rehearsal_run(head_commit={"id": REHEARSED, "tree_id": "f" * 40})),
            "a head record of another commit": (
                [bundle_artifact()], rehearsal_run(head_commit={"id": "e" * 40, "tree_id": TREE})),
            "an expired bundle": ([bundle_artifact(expired=True)], rehearsal_run()),
            "a rehearsal of another version": ([bundle_artifact(name=f"release-bundle-v0.26.0-{TREE}")], rehearsal_run()),
            "no bundle": ([], rehearsal_run()),
        }
        for case, (artifacts, run) in cases.items():
            found = release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token",
                                                get=artifacts_api(self, artifacts, {41707764199: run}))
            self.assertIsNone(found, case)

    def test_a_pull_requests_rehearsal_needs_its_one_pull_request_merged_into_main(self) -> None:
        # Specification 24, coordinator's decision of 4 November 2026: the run must belong to the one pull request of its
        # head, from its head branch, merged into this repository's main when the release plans. After a merge GitHub
        # lists only the merged pull request; one into another base shows only while the head is not merged, and is
        # open. Each case would promote a rehearsal that no pull request merged into main ran for.
        other_base = {"ref": "work/other-release", "repo": {"full_name": OFFICIAL}}
        copy = {"sha": REHEARSED, "ref": "work/copy", "repo": {"full_name": OFFICIAL}}
        cases = {
            "only an open pull request of the head into another base, with another Release workflow": (
                rehearsal_run(), [pull_request(number=224, state="open", merged_at=None, base=other_base)]),
            "a pull request not merged": (rehearsal_run(), [pull_request(state="open", merged_at=None)]),
            "two candidates, more than GitHub lists after a merge": (
                rehearsal_run(), [pull_request(), pull_request(number=224, state="open", merged_at=None, head=copy,
                                                               base=other_base)]),
            "no pull request": (rehearsal_run(), []),
            "a run of another branch of the merged head": (rehearsal_run(head_branch="work/copy"), [pull_request()]),
            "a merged pull request into another base, which GitHub does not list for a commit of main": (
                rehearsal_run(), [pull_request(base=other_base)]),
            "a pull request into another repository's main": (
                rehearsal_run(), [pull_request(base={"ref": "main", "repo": {"full_name": "someone/fork"}})]),
            "a pull request of another head": (
                rehearsal_run(), [pull_request(head={"sha": "e" * 40, "ref": "release/0.30.0",
                                                     "repo": {"full_name": OFFICIAL}})]),
        }
        for case, (run, pulls) in cases.items():
            found = release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(
                self, [bundle_artifact()], {41707764199: run}, pulls=pulls))
            self.assertIsNone(found, case)
        # A manual rehearsal needs no pull request: its ref is the branch whose head it rehearsed.
        manual = release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(
            self, [bundle_artifact()], {41707764199: manual_rehearsal_run()}, pulls=[]))
        self.assertEqual(manual, REHEARSAL)

    def test_an_explicit_call_of_this_repositorys_workflow_proves_no_workflow_commit(self) -> None:
        # Specification 24, coordinator's review of 4 November 2026: a forged run in a pull request from the release
        # branch into a base with another release.yml can call openot-real-databases.yml of this repository explicitly
        # at the real pull request's merge commit (with `if: false`); after the merge GitHub lists only pull request 223
        # for the head, so the merged-pull-request check passes for it. Only GitHub's record of a local call proves the
        # workflow's commit: the calling commit's full sha in its path, and the run's own ref. Each case would promote a
        # forged rehearsal.
        cases = {
            "an explicit call by ref name": {"path": f"{CALLED}@refs/pull/223/merge", "ref": "refs/pull/223/merge",
                                             "sha": MERGE},
            "an explicit call by sha": {"path": f"{CALLED}@{MERGE}", "ref": "", "sha": MERGE},
            "an explicit call by sha, recorded without a ref": {"path": f"{CALLED}@{MERGE}", "sha": MERGE},
            "a local call of another pull request's run": {"path": f"{CALLED}@{MERGE}", "ref": "refs/pull/224/merge",
                                                          "sha": MERGE},
            "a path that names another commit": {"path": f"{CALLED}@{BASE}", "ref": "refs/pull/223/merge", "sha": MERGE},
            "a short sha": {"path": f"{CALLED}@{MERGE[:8]}", "ref": "refs/pull/223/merge", "sha": MERGE[:8]},
        }
        for case, record in cases.items():
            found = release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(
                self, [bundle_artifact()], {41707764199: rehearsal_run(referenced_workflows=[record])}))
            self.assertIsNone(found, case)
        # GitHub's own record of run 41707764199: its local call at its own ref, with the merge commit's full sha.
        self.assertEqual(release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(
            self, [bundle_artifact()], {41707764199: rehearsal_run()})), REHEARSAL)

    def test_github_is_asked_again_while_it_lists_no_pull_request_of_the_head(self) -> None:
        # Specification 24, coordinator's review: a push's plan runs seconds after the merge, when GitHub may not list
        # the merged pull request yet. Catches a plan that gives up at once, and one that keeps asking: three questions
        # over about 30 seconds, then no promotion.
        api = artifacts_api(self, [bundle_artifact()], {41707764199: rehearsal_run()})
        for answers, found, asked, waits in (([[], [pull_request()]], REHEARSAL, 2, [15]),
                                             ([[], [], [], [pull_request()]], None, 3, [15, 15])):
            questions = []

            def get(path: str, token: str) -> dict | list:
                if "/pulls?" in path:
                    questions.append(path)
                    return answers.pop(0)
                return api(path, token)

            self.waits.clear()
            self.assertEqual(release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token", get=get), found)
            self.assertEqual((len(questions), self.waits), (asked, waits))

    def test_a_release_that_began_with_a_rehearsals_files_continues_with_them(self) -> None:
        # Specification 24, coordinator's review: a run after main's CI that continues a promotion which stopped
        # halfway takes the rehearsal whose files the published release holds, by its release-provenance.json, while
        # that rehearsal qualifies; a newer rehearsal of the same tree would stop at the published-release check and
        # wait for a maintainer. Catches a continuation that takes another rehearsal's files.
        older, newer = bundle_artifact(5, 41), bundle_artifact(9, 42)
        runs = {41: rehearsal_run(id=41), 42: rehearsal_run(id=42)}
        find = lambda **kwargs: release_plan.find_rehearsal(
            OFFICIAL, "0.27.0", TREE, "token", get=artifacts_api(self, [older, newer], runs), **kwargs)
        self.assertEqual(find(prefer=41).artifact_id, 5)
        self.assertEqual(find().artifact_id, 9, "without a published release, the highest id")
        runs[41] = rehearsal_run(id=41, conclusion="failure")
        self.assertEqual(find(prefer=41).artifact_id, 9, "a preferred rehearsal that no longer qualifies")

        record = lambda **fields: json.dumps({"promoted": True, "rehearsal": {"workflow_run_id": "41"}, **fields}).encode()
        release = {"assets": [{"name": "release-provenance.json", "browser_download_url": "https://github.com/x/p.json"}]}
        def named(answer, assets=release):
            return release_plan.promoted_rehearsal_run(OFFICIAL, "v0.27.0", "token", release_of=lambda *args: assets,
                                                       fetch=lambda url: answer)
        self.assertEqual(named(record()), 41)
        self.assertIsNone(named(record(promoted=False, rehearsal=None)), "a release that built its packages")
        self.assertIsNone(named(record(), assets=None), "no published release")
        self.assertIsNone(named(record(), assets={"assets": []}), "a release without its provenance yet")
        with mock.patch.object(release_plan, "tree_of", return_value=TREE), \
                mock.patch.object(release_plan, "promoted_rehearsal_run", return_value=41) as preferred, \
                mock.patch.object(release_plan, "find_rehearsal", return_value=REHEARSAL) as finder:
            self.assertEqual(release_plan.lookup_rehearsal(OFFICIAL, "0.27.0", "new", "token"), REHEARSAL)
        preferred.assert_called_once_with(OFFICIAL, "v0.27.0", "token")
        finder.assert_called_once_with(OFFICIAL, "0.27.0", TREE, "token", prefer=41)

    def test_an_error_of_the_promotion_lookup_never_fails_the_plan(self) -> None:
        # Specification 24, coordinator's review: a failed GitHub request or answer while looking for a rehearsal means
        # no promotion, with a notice that names it; a push's run then releases nothing, and the release after main's CI
        # builds. Catches a lookup error that fails the plan and so blocks the release that builds, and one that a
        # promotion swallows without a word. A failed completeness check still fails the plan.
        def broken(version: str, sha: str):
            raise urllib.error.URLError("api.github.com is unreachable")

        push = pushed(find_rehearsal=broken)
        self.assertEqual((push.mode, push.rehearsal), ("skip", None))
        self.assertIn("the promotion lookup failed (URLError: <urlopen error api.github.com is unreachable>)", push.notice)
        self.assertIn("v0.27.0 follows main's CI", push.notice)
        built = after_ci(find_rehearsal=broken)
        self.assertEqual((built.mode, built.rehearsal, built.platforms), ("release", None, release_plan.ALL_PLATFORMS))
        self.assertIn("the promotion lookup failed (URLError", built.notice)
        self.assertIn("v0.27.0 builds every package", built.notice)
        self.assertEqual(pushed().notice, "", "a lookup that worked says nothing")

        def unknown(tag: str) -> bool:
            raise urllib.error.URLError("api.github.com is unreachable")

        with self.assertRaises(urllib.error.URLError):
            after_ci(tag_commit=lambda tag: ("tag", "new"), release_complete=unknown)
        output = io.StringIO()
        with mock.patch.object(release_plan, "plan", return_value=push), contextlib.redirect_stdout(output), \
                mock.patch.dict(os.environ, {"GITHUB_OUTPUT": ""}):
            self.assertEqual(release_plan.main(["--event", "push"]), 0)
        self.assertIn("::notice::release plan: the promotion lookup failed (URLError", output.getvalue())

    def test_the_passed_rehearsals_bundle_of_the_highest_id_is_promoted(self) -> None:
        # Catches a promotion of an older rehearsal's files when a newer one passed, and a finder that gives up when the
        # bundle of the highest id comes from a failed run although an older one passed.
        older, newer = bundle_artifact(5, 41), bundle_artifact(9, 42)
        runs = {41: rehearsal_run(id=41), 42: rehearsal_run(id=42)}
        self.assertEqual(release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token",
                                                     get=artifacts_api(self, [older, newer], runs)).artifact_id, 9)
        runs[42] = rehearsal_run(id=42, conclusion="failure")
        self.assertEqual(release_plan.find_rehearsal(OFFICIAL, "0.27.0", TREE, "token",
                                                     get=artifacts_api(self, [older, newer], runs)).artifact_id, 5)

    def test_the_plan_writes_the_rehearsal_a_promotion_takes(self) -> None:
        # The Release workflow reads `rehearsal_run` and `rehearsal_artifact`. Catches a renamed or missing output: a
        # promoted release would then build nothing and download nothing, or a release after CI would look promoted.
        for result, expected in (
            (release_plan.Plan("release", "new", "0.27.0", "promoted", rehearsal=REHEARSAL),
             ["rehearsal_run=41707764199", "rehearsal_artifact=13281794816"]),
            (release_plan.Plan("release", "new", "0.27.0", "built", release_plan.ALL_PLATFORMS),
             ["rehearsal_run=", "rehearsal_artifact="]),
        ):
            with tempfile.TemporaryDirectory() as folder:
                output = Path(folder) / "output"
                with mock.patch.object(release_plan, "plan", return_value=result), \
                        mock.patch.dict(os.environ, {"GITHUB_OUTPUT": str(output)}), \
                        contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(release_plan.main(["--event", "push"]), 0)
                written = output.read_text(encoding="utf-8").splitlines()
            for line in expected + ["mode=release", "sha=new", "tag=v0.27.0"]:
                self.assertIn(line, written)

    def test_a_version_whose_release_the_queue_dropped_stops_the_next_release(self) -> None:
        # Catches a release that skips a version for good: GitHub keeps one waiting run per concurrency
        # group, so a later main CI run replaces the waiting release of an earlier version bump (Codex,
        # PR #146). Main's first-parent line here: "new" brings 0.28.0, "mid" brought 0.27.0 and has no
        # tag, "released" brought 0.26.0, which is tagged.
        versions = {"new": "0.28.0", "mid": "0.27.0", "released": "0.26.0"}
        tags = {"v0.26.0": ("tag", "released")}
        with self.assertRaisesRegex(ValueError, "v0.27.0 at mid has no tag; run the Release of that commit again"):
            after_ci(
                version_at=lambda sha: versions[sha],
                tag_commit=lambda tag: tags.get(tag),
                version_commits=lambda sha: ["new", "mid", "released"],
            )

    def test_commits_that_keep_the_version_do_not_count_as_skipped_releases(self) -> None:
        # Catches a check that reads every Cargo.toml change as a version: a dependency change keeps it.
        versions = {"new": "0.27.0", "deps": "0.26.0", "released": "0.26.0"}
        result = after_ci(
            version_at=lambda sha: versions[sha],
            tag_commit=lambda tag: {"v0.26.0": ("tag", "released")}.get(tag),
            version_commits=lambda sha: ["new", "deps", "released"],
        )
        self.assertEqual((result.mode, result.tag), ("release", "v0.27.0"))

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
