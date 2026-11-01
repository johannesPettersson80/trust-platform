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

    def test_a_push_compares_with_its_previous_commit(self) -> None:
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
