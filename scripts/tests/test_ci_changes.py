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

    def test_with_pull_requests_only_a_push_checks_everything(self) -> None:
        # Catches CI's own jobs skipped on a push to main: main's runs go one at a time and a newer
        # push replaces a waiting run, so a skip against the previous commit could leave a merged
        # change unchecked (specification 22, change detection).
        holder, repo = repository()
        with holder:
            before = git(repo, "rev-parse", "HEAD")
            (repo / "docs").mkdir()
            (repo / "docs" / "a.md").write_text("a\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "-m", "docs")
            after = git(repo, "rev-parse", "HEAD")
            only = {"cwd": repo, "pull_requests_only": True}
            self.assertEqual(ci_changes.decide("push", before, after, ["*.rs"], **only), (True, []))
            self.assertEqual(ci_changes.decide("merge_group", before, after, ["*.rs"], **only), (True, []))
            self.assertEqual(ci_changes.decide("pull_request", before, after, ["*.rs"], **only), (False, []))

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


    def test_the_last_matching_line_decides_and_an_exclamation_line_excludes(self) -> None:
        # Catches an exclusion that ignores the order of the lines, which would either run the
        # checks on an excluded path or skip them on a path a later line puts back on the list.
        lines = ["**", "!editors/vscode/**", "editors/vscode/package.json"]
        self.assertTrue(ci_changes.on_list("crates/trust-lsp/src/main.rs", lines))
        self.assertFalse(ci_changes.on_list("editors/vscode/src/extension.ts", lines))
        self.assertTrue(ci_changes.on_list("editors/vscode/package.json", lines))
        self.assertFalse(ci_changes.on_list("editors/vscode/package.json", lines + ["!**"]))
        self.assertFalse(ci_changes.on_list("README.md", ["!README.md"]))

    def test_the_rust_test_list_skips_only_what_no_rust_test_reads(self) -> None:
        # Catches a TypeScript-only change running the three Test jobs and Conformance, and an
        # extension file that Rust reads (a fixture project, the manifest that trust-lsp's tests
        # include) skipping them (specification 22, change detection).
        lines = ci_changes.read_patterns(
            (ROOT / ".github/paths/ci-rust-tests.txt").read_text(encoding="utf-8")
        )
        for path in [
            "crates/trust-lsp/src/main.rs",
            "Cargo.lock",
            "docs/specs/47-vscode-plant-simulation-ui.md",
            "examples/tank/src/main.st",
            ".github/workflows/ci.yml",
            ".github/paths/ci-rust-tests.txt",
            "scripts/ci_changes.py",
            "editors/vscode/src/test/fixtures/control-studio-engineer/control-loop.json",
            "editors/vscode/package.json",
            "editors/vscode/src/extension.ts",
        ]:
            self.assertTrue(ci_changes.on_list(path, lines), path)
        for path in [
            "editors/vscode/src/networkCanvas/plant/projection.ts",
            "editors/vscode/src/test/suite/plant-service.test.ts",
            "editors/vscode/package-lock.json",
            "AGENTS.md",
            "CLAUDE.md",
            ".codex/skills/trust-test-authoring/SKILL.md",
            "CHANGELOG.md",
            "changelog.d/plant-topology-in-rust.md",
        ]:
            self.assertFalse(ci_changes.on_list(path, lines), path)


if __name__ == "__main__":
    unittest.main()
