import argparse
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import release_candidate_guard as guard
import release_candidate_release as candidate_release


class ReleaseCandidateGuardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp_dir.name)
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "config", "user.email", "guard@example.invalid"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "config", "user.name", "Guard Test"],
            check=True,
        )
        (self.repo / "Cargo.toml").write_text(
            '[workspace.package]\nversion = "1.2.3"\n', encoding="utf-8"
        )
        (self.repo / "tracked.txt").write_text("baseline\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(self.repo), "commit", "-qm", "baseline"], check=True
        )
        self.base = guard.git(self.repo, "rev-parse", "HEAD").strip()
        subprocess.run(
            ["git", "-C", str(self.repo), "branch", "-M", "main"], check=True
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "update-ref", "refs/remotes/origin/main", self.base],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "switch", "-qc", "integrate/example"], check=True
        )
        (self.repo / "tracked.txt").write_text("candidate\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "commit", "-qam", "candidate"], check=True)
        self.head = guard.git(self.repo, "rev-parse", "HEAD").strip()

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_a_push_needs_no_artifact_but_main_and_release_tags_are_refused(self) -> None:
        # Specification 22, "Change and release workflow" (owner, 31 October 2026): CI is the gate, so a work branch, an
        # integration branch and a version bump push without any artifact; main changes only through a merged pull
        # request and a release tag only through the Release workflow, neither pushed nor deleted from here. Catches the
        # retired guard still blocking a push, and a direct push to main or of a tag going through.
        for remote_ref in ("refs/heads/work/next", "refs/heads/integrate/example", "refs/heads/release/next"):
            self.assertEqual(guard.validate_push(remote_ref=remote_ref), [], remote_ref)
        for remote_ref, words in (
            ("refs/heads/main", "merged pull request"),
            ("refs/heads/master", "merged pull request"),
            ("refs/tags/v1.2.3", "Release workflow"),
        ):
            self.assertIn(words, "\n".join(guard.validate_push(remote_ref=remote_ref)), remote_ref)

    def test_the_workflow_keeps_its_tools_and_offers_no_retired_stage(self) -> None:
        # Specification 22, "Change and release workflow": CI runs every stage that the release guard's prepare and
        # preflight ran, and branch protection refuses what check-merge refused; the pre-push check, a pull request's
        # failure logs, release verification and the post-merge audit stay. Catches a kept tool removed with the retired
        # guard, and a retired stage still offered.
        head = "a" * 40
        for argv in (
            ["check-push"],
            ["collect-failures", "--pr", "1"],
            ["verify-release", "--candidate-head", head, "--branch", "release/next"],
            ["audit-post-merge", "--candidate-head", head, "--branch", "release/next"],
        ):
            self.assertEqual(guard.parser().parse_args(argv).command, argv[0])
        # Each with the arguments it took, so only an unknown command is refused.
        for retired in (
            ["prepare", "--remote-worktree", "/w"],
            ["preflight", "--remote-worktree", "/w"],
            ["check-merge", "--pr", "1"],
        ):
            with self.subTest(retired[0]), self.assertRaises(SystemExit):
                with mock.patch("sys.stderr", new_callable=io.StringIO):
                    guard.parser().parse_args(retired)

    def test_a_command_whose_output_splits_a_character_is_recorded(self) -> None:
        # Two processes writing to one pipe can split a UTF-8 character; the guard crashed reading
        # such output (27 October 2026). The broken sequence is replaced, the rest kept.
        result = guard.run(
            [sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'ok \\xe2 rest')"]
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "ok \ufffd rest")

    def test_incomplete_checks_cannot_create_failure_ledger(self) -> None:
        checks = [
            {"name": "Windows", "state": "FAILURE", "detailsUrl": "https://example.invalid/1"},
            {"name": "Verification", "state": "IN_PROGRESS", "detailsUrl": "https://example.invalid/2"},
        ]
        with self.assertRaisesRegex(ValueError, "checks are still pending"):
            guard.build_failure_ledger(self.head, checks, {})

    def collect_with_rollups(self, rollups: list[list[dict]], wait: bool) -> tuple[int, int]:
        """Run collect-failures against a fake `gh pr view` that returns each rollup in turn."""
        real_run = guard.run
        calls = {"gh": 0}

        def fake_run(command, **kwargs):
            if command[0] != "gh":
                return real_run(command, **kwargs)
            rollup = rollups[min(calls["gh"], len(rollups) - 1)]
            calls["gh"] += 1
            payload = {"headRefOid": self.head, "statusCheckRollup": rollup, "url": "https://example.invalid/pr/1"}
            return subprocess.CompletedProcess(command, 0, stdout=json.dumps(payload), stderr="")

        args = argparse.Namespace(repo=str(self.repo), pr=1, wait=wait, timeout=60, interval=0)
        with mock.patch.object(guard, "run", side_effect=fake_run), mock.patch.object(guard.time, "sleep"):
            with mock.patch("sys.stderr", new_callable=io.StringIO):
                status = guard.collect_failures(args)
        return status, calls["gh"]

    def test_collect_failures_waits_while_no_checks_are_registered(self) -> None:
        done = [{"name": "Windows", "conclusion": "SUCCESS", "status": "COMPLETED"}]
        status, polls = self.collect_with_rollups([[], [], done], wait=True)
        self.assertEqual(status, 0)
        self.assertEqual(polls, 3)
        ledger = json.loads(guard.failure_ledger_path(self.repo, self.head).read_text(encoding="utf-8"))
        self.assertEqual(ledger["check_count"], 1)

    def test_collect_failures_without_wait_reports_unregistered_checks_as_pending(self) -> None:
        status, polls = self.collect_with_rollups([[]], wait=False)
        self.assertEqual(status, 3)
        self.assertEqual(polls, 1)
        self.assertFalse(guard.failure_ledger_path(self.repo, self.head).exists())

    def test_failure_ledger_requires_logs_for_every_failed_check(self) -> None:
        checks = [
            {"name": "Windows", "state": "FAILURE", "detailsUrl": "https://example.invalid/1"}
        ]
        with self.assertRaisesRegex(ValueError, "missing failure log"):
            guard.build_failure_ledger(self.head, checks, {})

    def test_release_state_is_not_complete_without_latest_assets_and_marketplace(self) -> None:
        state = {
            "tag_on_main": True,
            "candidate_released": False,
            "annotated_tag_matches": True,
            "release_workflow_success": True,
            "github_release_published": True,
            "github_release_latest": False,
            "assets_verified": False,
            "marketplace_versions": {"linux-x64": "1.2.2"},
        }
        failures = guard.validate_release_state(state, "1.2.3", ["linux-x64"])
        joined = "\n".join(failures)
        self.assertIn("merged candidate", joined)
        self.assertIn("Latest", joined)
        self.assertIn("assets", joined)
        self.assertIn("linux-x64", joined)

    def test_the_release_run_is_the_one_its_provenance_names(self) -> None:
        # The Release runs after main's CI on main's ref, so no tag ref finds it: the release's own
        # provenance names its run. A provenance without one names no run.
        calls = []

        def fake_run(command, cwd=None):
            calls.append(list(command))
            return subprocess.CompletedProcess(
                command, 0, stdout='{"status": "completed", "conclusion": "success", "workflowName": "Release"}', stderr=""
            )

        with mock.patch.object(candidate_release.guard, "run", side_effect=fake_run):
            self.assertEqual(candidate_release.release_run_of({}, self.repo), {})
            self.assertEqual(candidate_release.release_run_of({"workflow_run_id": "abc"}, self.repo), {})
            self.assertEqual(calls, [])
            found = candidate_release.release_run_of({"workflow_run_id": "4242"}, self.repo)
        self.assertEqual(found["conclusion"], "success")
        self.assertEqual(calls[0][:4], ["gh", "run", "view", "4242"])

    def test_release_asset_check_recomputes_published_checksum(self) -> None:
        asset_dir = self.repo / "assets"
        asset_dir.mkdir()
        payload = asset_dir / "trust.vsix"
        payload.write_bytes(b"extension")
        digest = guard.sha256_bytes(payload.read_bytes())
        (asset_dir / "SHA256SUMS").write_text(f"{digest}  trust.vsix\n", encoding="utf-8")
        assets = [{"name": "trust.vsix"}, {"name": "SHA256SUMS"}]
        self.assertTrue(candidate_release.verify_downloaded_assets(asset_dir, assets))
        payload.write_bytes(b"tampered")
        self.assertFalse(candidate_release.verify_downloaded_assets(asset_dir, assets))

    def test_release_asset_check_resolves_flattened_github_asset_name(self) -> None:
        asset_dir = self.repo / "flattened-assets"
        asset_dir.mkdir()
        payload = asset_dir / "trust.vsix"
        payload.write_bytes(b"extension")
        digest = guard.sha256_bytes(payload.read_bytes())
        (asset_dir / "SHA256SUMS").write_text(
            f"{digest}  vsix-artifacts/trust.vsix\n", encoding="utf-8"
        )
        assets = [{"name": "trust.vsix"}, {"name": "SHA256SUMS"}]
        self.assertTrue(candidate_release.verify_downloaded_assets(asset_dir, assets))

    def test_release_asset_check_rejects_ambiguous_flattened_names(self) -> None:
        asset_dir = self.repo / "ambiguous-assets"
        asset_dir.mkdir()
        payload = asset_dir / "trust.vsix"
        payload.write_bytes(b"extension")
        digest = guard.sha256_bytes(payload.read_bytes())
        (asset_dir / "SHA256SUMS").write_text(
            "\n".join(
                [
                    f"{digest}  first/trust.vsix",
                    f"{digest}  second/trust.vsix",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        assets = [{"name": "trust.vsix"}, {"name": "SHA256SUMS"}]
        self.assertFalse(candidate_release.verify_downloaded_assets(asset_dir, assets))

    def test_pre_push_input_passes_a_branch_and_blocks_main(self) -> None:
        # The hook's own input (git's pre-push lines): a branch goes through, a push to main is blocked, and a line that
        # is not four fields is reported.
        branch = f"refs/heads/integrate/example {self.head} refs/heads/integrate/example {'0' * 40}"
        self.assertEqual(guard.check_push_lines([branch]), [])
        main = f"refs/heads/work/next {self.head} refs/heads/main {'0' * 40}"
        self.assertIn("merged pull request", "\n".join(guard.check_push_lines([main])))
        self.assertIn("invalid pre-push input line", "\n".join(guard.check_push_lines(["broken"])))


if __name__ == "__main__":
    unittest.main()
