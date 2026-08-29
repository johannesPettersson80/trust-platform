import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import release_candidate_cleanup as candidate_cleanup
import release_candidate_guard as guard
import release_candidate_release as candidate_release


SCRIPT = Path(__file__).with_name("release_candidate_guard.py")
REPO_ROOT = Path(__file__).resolve().parents[4]


class PostMergeCleanupCommandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "config", "user.email", "cleanup@example.invalid"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "config", "user.name", "Cleanup Test"],
            check=True,
        )
        (self.repo / "tracked.txt").write_text("base\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "add", "tracked.txt"], check=True)
        subprocess.run(["git", "-C", str(self.repo), "commit", "-qm", "base"], check=True)
        subprocess.run(["git", "-C", str(self.repo), "branch", "-M", "main"], check=True)
        subprocess.run(["git", "-C", str(self.repo), "switch", "-qc", "fix/example"], check=True)
        (self.repo / "tracked.txt").write_text("candidate\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "commit", "-qam", "candidate"], check=True)
        self.candidate = subprocess.run(
            ["git", "-C", str(self.repo), "rev-parse", "HEAD"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.strip()
        subprocess.run(["git", "-C", str(self.repo), "switch", "main"], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "merge", "--no-ff", "-qm", "merge", "fix/example"],
            check=True,
        )
        subprocess.run(["git", "-C", str(self.repo), "branch", "-D", "fix/example"], check=True)
        main = subprocess.run(
            ["git", "-C", str(self.repo), "rev-parse", "HEAD"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.strip()
        self.remote = self.root / "origin.git"
        subprocess.run(["git", "init", "--bare", "-q", str(self.remote)], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "remote", "add", "origin", str(self.remote)],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "push", "-q", "origin", "main"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "update-ref", "refs/remotes/origin/main", main],
            check=True,
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def restore_candidate_state(self) -> Path:
        candidate_worktree = self.root / "candidate-worktree"
        subprocess.run(
            ["git", "-C", str(self.repo), "branch", "fix/example", self.candidate],
            check=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "push",
                "-q",
                "origin",
                f"{self.candidate}:refs/heads/fix/example",
            ],
            check=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "worktree",
                "add",
                "--detach",
                str(candidate_worktree),
                self.candidate,
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        return candidate_worktree

    def test_clean_merged_candidate_audit_passes_after_cleanup(self) -> None:
        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                self.candidate,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )

        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn('"status": "clean"', result.stdout)

    def test_clean_candidate_state_is_reported_as_explicit_cleanup_targets(self) -> None:
        candidate_worktree = self.restore_candidate_state()

        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                self.candidate,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        payload = json.loads(result.stdout)

        self.assertEqual(result.returncode, 2, result.stdout)
        self.assertEqual(payload["status"], "cleanup_required")
        self.assertEqual(
            {(row["kind"], row["target"]) for row in payload["cleanup_targets"]},
            {
                ("local_branch", "refs/heads/fix/example"),
                ("remote_branch", "refs/remotes/origin/fix/example"),
                ("worktree", str(candidate_worktree.resolve())),
            },
        )

    def test_dirty_candidate_worktree_blocks_all_cleanup_targets(self) -> None:
        candidate_worktree = self.restore_candidate_state()
        (candidate_worktree / "tracked.txt").write_text("uncommitted\n", encoding="utf-8")

        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                self.candidate,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        payload = json.loads(result.stdout)

        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertEqual(payload["status"], "blocked")
        self.assertIn("is dirty", "\n".join(payload["failures"]))
        self.assertEqual(payload["cleanup_targets"], [])

    def test_unmerged_candidate_is_blocked(self) -> None:
        subprocess.run(
            ["git", "-C", str(self.repo), "switch", "--detach", self.candidate],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        (self.repo / "tracked.txt").write_text("later candidate\n", encoding="utf-8")
        subprocess.run(
            ["git", "-C", str(self.repo), "commit", "-qam", "not merged"],
            check=True,
        )
        unmerged = subprocess.run(
            ["git", "-C", str(self.repo), "rev-parse", "HEAD"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.strip()

        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                unmerged,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        payload = json.loads(result.stdout)

        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("not contained", "\n".join(payload["failures"]))
        self.assertEqual(payload["cleanup_targets"], [])

    def test_missing_candidate_is_blocked_as_unavailable(self) -> None:
        missing = "f" * 40
        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                missing,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        payload = json.loads(result.stdout)

        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("is not available", "\n".join(payload["failures"]))
        self.assertEqual(payload["cleanup_targets"], [])

    def test_branch_moved_from_candidate_is_blocked(self) -> None:
        main = subprocess.run(
            ["git", "-C", str(self.repo), "rev-parse", "origin/main"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.strip()
        subprocess.run(
            ["git", "-C", str(self.repo), "branch", "fix/example", main],
            check=True,
        )

        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                self.candidate,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        payload = json.loads(result.stdout)

        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("not candidate head", "\n".join(payload["failures"]))
        self.assertEqual(payload["cleanup_targets"], [])

    def test_audit_fetches_remote_candidate_branch_before_reporting_clean(self) -> None:
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "push",
                "-q",
                "origin",
                f"{self.candidate}:refs/heads/fix/example",
            ],
            check=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "update-ref",
                "-d",
                "refs/remotes/origin/fix/example",
            ],
            check=True,
        )

        result = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--repo",
                str(self.repo),
                "audit-post-merge",
                "--candidate-head",
                self.candidate,
                "--branch",
                "fix/example",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        payload = json.loads(result.stdout)

        self.assertEqual(result.returncode, 2, result.stdout)
        self.assertEqual(payload["status"], "cleanup_required")
        self.assertIn(
            {"kind": "remote_branch", "target": "refs/remotes/origin/fix/example"},
            payload["cleanup_targets"],
        )


class PostMergeCleanupRegistrationTests(unittest.TestCase):
    def test_ci_runs_all_release_candidate_guard_selftests(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        normalized = " ".join(workflow.split())

        self.assertIn(
            "python3 -m unittest discover -s "
            ".codex/skills/trust-ci-release-gates/scripts "
            "-p 'release_candidate_*_tests.py'",
            normalized,
        )

    def test_verify_release_requires_exact_candidate_identity(self) -> None:
        with self.assertRaises(SystemExit):
            guard.parser().parse_args(["verify-release"])

        args = guard.parser().parse_args(
            [
                "verify-release",
                "--candidate-head",
                "a" * 40,
                "--branch",
                "fix/example",
            ]
        )
        self.assertEqual(args.candidate_head, "a" * 40)
        self.assertEqual(args.branch, "fix/example")

    def test_verify_release_runs_post_merge_audit_after_public_proof(self) -> None:
        args = guard.parser().parse_args(
            [
                "verify-release",
                "--candidate-head",
                "a" * 40,
                "--branch",
                "fix/example",
            ]
        )
        with (
            mock.patch.object(candidate_release, "verify_release", return_value=0) as release,
            mock.patch.object(candidate_cleanup, "audit_post_merge", return_value=2) as cleanup,
        ):
            result = guard.verify_release_command(args)

        self.assertEqual(result, 2)
        release.assert_called_once_with(args)
        cleanup.assert_called_once_with(args)


if __name__ == "__main__":
    unittest.main()
