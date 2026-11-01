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
        version_commits=lambda sha: [],
        changes=lambda base, head, paths: False,
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
