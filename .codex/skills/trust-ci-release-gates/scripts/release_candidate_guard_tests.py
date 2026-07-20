import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import release_candidate_guard as guard
import release_candidate_prepare as candidate_prepare
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

    def passing_artifact(self) -> dict:
        return {
            "schema_version": 1,
            "status": "pass",
            "head": self.head,
            "base_ref": "origin/main",
            "base_sha": self.base,
            "changed_paths_sha256": guard.changed_paths_sha256(
                self.repo, self.base, self.head
            ),
            "created_at": "2026-07-19T12:00:00+00:00",
            "commands": [
                {
                    "id": command_id,
                    "command": command_id,
                    "exit_status": 0,
                    "output_sha256": "0" * 64,
                    "duration_ms": 1,
                    "scope": "local" if not command_id.startswith("remote_") else "trust-builder",
                }
                for command_id in guard.required_command_ids(vscode_changed=False)
            ],
        }

    def test_release_sensitive_branch_requires_exact_sha_artifact(self) -> None:
        failures = guard.validate_push_candidate(
            self.repo,
            local_ref="refs/heads/integrate/example",
            local_oid=self.head,
            remote_ref="refs/heads/integrate/example",
            artifact=None,
        )
        self.assertIn("missing exact-SHA release-candidate artifact", "\n".join(failures))

    def test_bootstrap_detects_any_agent_or_skill_mismatch(self) -> None:
        canonical = self.repo / "canonical"
        destination = self.repo / "destination"
        for root in (canonical, destination):
            (root / ".codex/skills/example").mkdir(parents=True)
            (root / "AGENTS.md").write_text("rules\n", encoding="utf-8")
            (root / ".codex/skills/example/SKILL.md").write_text("skill\n", encoding="utf-8")
        self.assertEqual(candidate_prepare.bootstrap_failures(destination, canonical), [])
        (destination / ".codex/skills/example/SKILL.md").write_text("stale\n", encoding="utf-8")
        self.assertIn(
            ".codex/skills/example/SKILL.md",
            "\n".join(candidate_prepare.bootstrap_failures(destination, canonical)),
        )

    def test_failed_cheap_preflight_blocks_the_next_stage(self) -> None:
        records = [
            {"id": "bootstrap", "exit_status": 0},
            {"id": "planner", "exit_status": 4},
        ]
        self.assertFalse(candidate_prepare.stage_passed(records, ("bootstrap", "planner")))

    def test_planner_advisory_exit_is_accepted_without_hiding_raw_status(self) -> None:
        payload = {
            "areas": ["bytecode_vm", "runtime_safety"],
            "missing_test_classes": ["runtime_vertical"],
            "missing_test_classes_by_area": {
                "runtime_safety": ["runtime_vertical"],
            },
            "spec_gaps": [],
            "unmapped_files": [],
            "unknown_areas": [],
            "uninventoried_areas": [],
        }
        accepted = candidate_prepare.planner_exit_is_advisory(2, json.dumps(payload))
        blocked = candidate_prepare.planner_exit_is_advisory(
            4,
            json.dumps({**payload, "unmapped_files": ["unknown/path"]}),
        )

        self.assertTrue(accepted)
        self.assertFalse(blocked)

    def test_planner_command_requests_json_for_advisory_parser(self) -> None:
        command = candidate_prepare.planner_command(
            python="python3",
            intent="bugfix",
            baseline="a" * 40,
            paths=["crates/trust-runtime/src/lib.rs"],
        )

        self.assertEqual(command[-2:], ["--format", "json"])

    def test_remote_vscode_command_runs_rendered_tests_headlessly(self) -> None:
        self.assertEqual(
            candidate_prepare.remote_vscode_command(),
            "cd editors/vscode && npm run lint && npm run compile && "
            "TRUST_UI_TEST_BROWSER=/usr/bin/google-chrome "
            'xvfb-run -a -s "-screen 0 1920x1080x24" npm test',
        )

    def test_stale_head_and_base_are_rejected(self) -> None:
        artifact = self.passing_artifact()
        artifact["head"] = "1" * 40
        artifact["base_sha"] = "2" * 40
        failures = guard.validate_artifact(self.repo, artifact, self.head)
        joined = "\n".join(failures)
        self.assertIn("artifact head", joined)
        self.assertIn("artifact base", joined)

    def test_missing_required_command_is_rejected(self) -> None:
        artifact = self.passing_artifact()
        artifact["commands"] = artifact["commands"][1:]
        failures = guard.validate_artifact(self.repo, artifact, self.head)
        self.assertIn("missing required command", "\n".join(failures))

    def test_incomplete_checks_cannot_create_failure_ledger(self) -> None:
        checks = [
            {"name": "Windows", "state": "FAILURE", "detailsUrl": "https://example.invalid/1"},
            {"name": "Verification", "state": "IN_PROGRESS", "detailsUrl": "https://example.invalid/2"},
        ]
        with self.assertRaisesRegex(ValueError, "checks are still pending"):
            guard.build_failure_ledger(self.head, checks, {})

    def test_failure_ledger_requires_logs_for_every_failed_check(self) -> None:
        checks = [
            {"name": "Windows", "state": "FAILURE", "detailsUrl": "https://example.invalid/1"}
        ]
        with self.assertRaisesRegex(ValueError, "missing failure log"):
            guard.build_failure_ledger(self.head, checks, {})

    def test_merge_requires_exact_head_and_all_green_checks(self) -> None:
        pr = {
            "headRefOid": self.head,
            "mergeStateStatus": "BLOCKED",
            "statusCheckRollup": [
                {"name": "Windows", "conclusion": "FAILURE", "status": "COMPLETED"}
            ],
        }
        failures = guard.validate_merge_state(pr, self.head)
        joined = "\n".join(failures)
        self.assertIn("merge state", joined)
        self.assertIn("Windows", joined)

    def test_release_state_is_not_complete_without_latest_assets_and_marketplace(self) -> None:
        state = {
            "main_sha_matches": True,
            "annotated_tag_matches": True,
            "release_workflow_success": True,
            "github_release_published": True,
            "github_release_latest": False,
            "assets_verified": False,
            "marketplace_versions": {"linux-x64": "1.2.2"},
        }
        failures = guard.validate_release_state(state, "1.2.3", ["linux-x64"])
        joined = "\n".join(failures)
        self.assertIn("Latest", joined)
        self.assertIn("assets", joined)
        self.assertIn("linux-x64", joined)

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

    @mock.patch.object(guard, "load_artifact", return_value=None)
    def test_pre_push_input_blocks_release_branch(self, _load: mock.Mock) -> None:
        line = (
            f"refs/heads/integrate/example {self.head} "
            f"refs/heads/integrate/example {'0' * 40}\n"
        )
        failures = guard.check_push_lines(self.repo, line.splitlines())
        self.assertIn("missing exact-SHA", "\n".join(failures))


if __name__ == "__main__":
    unittest.main()
