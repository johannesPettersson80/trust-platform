"""Change detection for pull-request checks (specification 22, "Pull-request checks and change
detection"). Every expected value follows from the change each test makes."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import ci_changes  # noqa: E402


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=repo, check=True, capture_output=True, text=True
    ).stdout.strip()


def repository() -> tuple[tempfile.TemporaryDirectory, Path]:
    holder = tempfile.TemporaryDirectory()
    repo = Path(holder.name)
    git(repo, "init", "-q", "-b", "main")
    git(repo, "config", "user.email", "ci@example.com")
    git(repo, "config", "user.name", "CI")
    (repo / "README.md").write_text("start\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "start")
    return holder, repo


class ChangeDetectionTests(unittest.TestCase):
    def test_globs_cross_directories_only_with_two_stars(self) -> None:
        # Catches `*` matching across directories, which would make a narrow area run on any change.
        diagrams = ci_changes.pattern_regex("docs/diagrams/**/*.puml")
        self.assertTrue(diagrams.match("docs/diagrams/flow.puml"))
        self.assertTrue(diagrams.match("docs/diagrams/a/b/flow.puml"))
        self.assertFalse(diagrams.match("docs/diagrams/flow.svg"))
        one_level = ci_changes.pattern_regex("scripts/*.py")
        self.assertTrue(one_level.match("scripts/ci_changes.py"))
        self.assertFalse(one_level.match("scripts/tests/test_ci_changes.py"))
        tree = ci_changes.pattern_regex("editors/vscode/**")
        self.assertTrue(tree.match("editors/vscode/src/extension.ts"))
        self.assertFalse(tree.match("editors/vscodex/file"))

    def test_a_relevant_file_after_the_first_300_still_runs_the_checks(self) -> None:
        # The failure it prevents: GitHub's path filters compare only the first 300 changed files, so
        # 0.25.0's large pull request ran no documentation workflow (28 October 2026).
        holder, repo = repository()
        with holder:
            base = git(repo, "rev-parse", "HEAD")
            git(repo, "checkout", "-q", "-b", "big")
            for index in range(400):
                (repo / f"a{index:03d}.txt").write_text(f"{index}\n")
            (repo / "zz").mkdir()
            (repo / "zz" / "page.md").write_text("docs\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "big")
            head = git(repo, "rev-parse", "HEAD")
            relevant, matched = ci_changes.decide("pull_request", base, head, ["zz/**"], cwd=repo)
            self.assertTrue(relevant)
            self.assertEqual(matched, ["zz/page.md"])
            relevant, matched = ci_changes.decide("pull_request", base, head, ["docs/**"], cwd=repo)
            self.assertFalse(relevant)
            self.assertEqual(matched, [])

    def test_a_push_compares_with_the_commit_it_is_given(self) -> None:
        # Catches a push compared with the start of history, which would rerun every check on each push.
        holder, repo = repository()
        with holder:
            (repo / "docs").mkdir()
            (repo / "docs" / "a.md").write_text("a\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "docs")
            before = git(repo, "rev-parse", "HEAD")
            (repo / "src.rs").write_text("fn main() {}\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "code")
            after = git(repo, "rev-parse", "HEAD")
            self.assertEqual(ci_changes.decide("push", before, after, ["docs/**"], cwd=repo), (False, []))
            self.assertEqual(ci_changes.decide("push", before, after, ["*.rs"], cwd=repo), (True, ["src.rs"]))

    def test_a_push_compares_with_the_newest_passed_run_of_its_workflow(self) -> None:
        # Checklist W24: a newer push to main replaces a run that still waits. A docs commit whose run
        # was replaced, followed by a code commit, was compared only with that docs commit, so its own
        # change was never checked on main. The newest passed run's commit covers both.
        holder, repo = repository()
        with holder:
            passed = git(repo, "rev-parse", "HEAD")
            (repo / "docs").mkdir()
            (repo / "docs" / "a.md").write_text("a\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "docs, whose run a newer push replaced")
            replaced = git(repo, "rev-parse", "HEAD")
            (repo / "src.rs").write_text("fn main() {}\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "code")
            head = git(repo, "rev-parse", "HEAD")
            asked = []

            def passed_head(repository: str, workflow: str) -> str:
                asked.append((repository, workflow))
                return passed

            base = ci_changes.compared_base("push", replaced, "diagrams.yml", "owner/repo", passed_head)
            self.assertEqual(base, passed)
            self.assertEqual(asked, [("owner/repo", "diagrams.yml")])
            self.assertEqual(ci_changes.decide("push", base, head, ["docs/**"], cwd=repo), (True, ["docs/a.md"]))
            # The push's previous commit would have missed it.
            self.assertEqual(ci_changes.decide("push", replaced, head, ["docs/**"], cwd=repo), (False, []))

    def test_a_push_without_a_known_passed_run_checks_everything(self) -> None:
        # Catches a push that skips its checks when GitHub lists no passed run, or cannot list them.
        base = ci_changes.compared_base("push", "1" * 40, "diagrams.yml", "owner/repo", lambda *_: None)
        self.assertEqual(base, "")
        self.assertEqual(ci_changes.decide("push", base, "HEAD", ["x/**"]), (True, []))

    def test_only_a_push_that_names_its_workflow_asks_for_a_passed_run(self) -> None:
        # Catches a pull request or merge group compared with main's passed run instead of its own base.
        def never(*_: str) -> str:
            raise AssertionError("no passed run is asked for")

        self.assertEqual(ci_changes.compared_base("pull_request", "base", "diagrams.yml", "o/r", never), "base")
        self.assertEqual(ci_changes.compared_base("merge_group", "base", "diagrams.yml", "o/r", never), "base")
        self.assertEqual(ci_changes.compared_base("push", "before", "", "o/r", never), "before")

    def test_the_newest_passed_run_is_asked_of_github(self) -> None:
        # Catches a request for another branch, event or status, and an answer taken for a commit when
        # GitHub failed, listed nothing or gave something else.
        sha = "0123456789abcdef0123456789abcdef01234567"
        calls = []

        def answering(returncode: int, stdout: str):
            def run(argv, **_):
                calls.append(argv)
                return subprocess.CompletedProcess(argv, returncode, stdout=stdout, stderr="")

            return run

        self.assertEqual(ci_changes.last_passed_head("owner/repo", "diagrams.yml", answering(0, sha + "\n")), sha)
        self.assertEqual(
            calls[0][:3],
            ["gh", "api", "repos/owner/repo/actions/workflows/diagrams.yml/runs?branch=main&event=push&status=success&per_page=1"],
        )
        for returncode, stdout in [(1, sha), (0, ""), (0, "null"), (0, "main")]:
            self.assertIsNone(
                ci_changes.last_passed_head("owner/repo", "diagrams.yml", answering(returncode, stdout)),
                f"{returncode} {stdout!r}",
            )
        self.assertIsNone(ci_changes.last_passed_head("", "diagrams.yml", answering(0, sha)))

    def test_a_merge_group_compares_with_the_main_it_was_built_on(self) -> None:
        # Specification 22, "Change and release workflow": the merge queue builds the pull request on
        # top of the current main. Catches a merge group that runs every check whatever it changes
        # (each queued pull request would wait for every workflow), and one that counts main's own
        # newer commits as its change.
        holder, repo = repository()
        with holder:
            git(repo, "checkout", "-q", "-b", "pr")
            (repo / "docs").mkdir()
            (repo / "docs" / "a.md").write_text("a\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "docs")
            git(repo, "checkout", "-q", "main")
            (repo / "src.rs").write_text("fn main() {}\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "code on main")
            main = git(repo, "rev-parse", "HEAD")
            git(repo, "merge", "-q", "--no-ff", "-m", "merge group", "pr")
            group = git(repo, "rev-parse", "HEAD")
            self.assertEqual(ci_changes.decide("merge_group", main, group, ["docs/**"], cwd=repo), (True, ["docs/a.md"]))
            self.assertEqual(ci_changes.decide("merge_group", main, group, ["*.rs"], cwd=repo), (False, []))

    def test_a_file_moved_out_of_a_watched_tree_still_counts(self) -> None:
        # Catches rename detection reporting only the new path: moving docs/public/a.md to archive/a.md
        # matched no docs path, so the site kept the old page (Codex's review of #146).
        holder, repo = repository()
        with holder:
            (repo / "docs" / "public").mkdir(parents=True)
            (repo / "docs" / "public" / "a.md").write_text("a page that stays the same when moved\n" * 5)
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "page")
            before = git(repo, "rev-parse", "HEAD")
            (repo / "archive").mkdir()
            git(repo, "mv", "docs/public/a.md", "archive/a.md")
            git(repo, "commit", "-q", "-m", "move")
            after = git(repo, "rev-parse", "HEAD")
            self.assertEqual(
                ci_changes.decide("push", before, after, ["docs/public/**"], cwd=repo),
                (True, ["docs/public/a.md"]),
            )

    def test_manual_scheduled_and_unknown_previous_runs_check_everything(self) -> None:
        # Catches a dispatched or scheduled run that skips its checks, and a new branch's first push
        # (no previous commit) that checks nothing.
        self.assertEqual(ci_changes.decide("workflow_dispatch", "", "HEAD", ["x/**"]), (True, []))
        self.assertEqual(ci_changes.decide("schedule", "", "HEAD", ["x/**"]), (True, []))
        self.assertEqual(ci_changes.decide("push", ci_changes.NULL_SHA, "HEAD", ["x/**"]), (True, []))

    def test_a_failed_diff_fails_instead_of_skipping(self) -> None:
        # Specification 22: a failed `changes` job fails the workflow; it never reads as "nothing changed".
        holder, repo = repository()
        with holder:
            with self.assertRaises(subprocess.CalledProcessError):
                ci_changes.decide("push", "1" * 40, "HEAD", ["x/**"], cwd=repo)

    def test_the_path_list_ignores_blank_lines_and_comments(self) -> None:
        self.assertEqual(
            ci_changes.read_patterns("# area\n\ndocs/**\n  scripts/*.py  \n"),
            ["docs/**", "scripts/*.py"],
        )


if __name__ == "__main__":
    unittest.main()
