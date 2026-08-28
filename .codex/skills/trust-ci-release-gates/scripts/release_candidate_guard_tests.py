import io
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

    def test_vscode_remote_gate_runs_under_headless_display(self) -> None:
        commands = dict(
            candidate_prepare.remote_validation_commands(
                vscode_changed=True, remote_target="/tmp/trust-target"
            )
        )
        self.assertIn("npm ci &&", commands["remote_vscode"])
        self.assertIn("xvfb-run -a npm test", commands["remote_vscode"])

    def test_full_suite_is_builder_only_before_candidate_validation(self) -> None:
        self.assertNotIn("local_test_all", guard.BASE_REQUIRED_COMMANDS)
        self.assertEqual(candidate_prepare.local_candidate_validation_commands(), ())
        commands = dict(
            candidate_prepare.remote_validation_commands(
                vscode_changed=False, remote_target="/tmp/trust-target"
            )
        )
        self.assertIn("remote_test_all", commands)
        self.assertIn("just test-all", commands["remote_test_all"])

    def test_remote_validation_reclaims_exact_target_before_test_all(self) -> None:
        commands = candidate_prepare.remote_validation_commands(
            vscode_changed=True, remote_target="/tmp/trust target"
        )
        command_ids = [command_id for command_id, _command in commands]

        self.assertLess(
            command_ids.index("remote_prepare_target"),
            command_ids.index("remote_clippy"),
        )
        self.assertLess(
            command_ids.index("remote_clippy"),
            command_ids.index("remote_reclaim_before_test_all"),
        )
        self.assertLess(
            command_ids.index("remote_reclaim_before_test_all"),
            command_ids.index("remote_test_all"),
        )
        by_id = dict(commands)
        self.assertEqual(
            by_id["remote_prepare_target"],
            "mkdir -p -- '/tmp/trust target/tmp' '/tmp/trust target/bin' && "
            "install -m 755 "
            ".codex/skills/trust-ci-release-gates/scripts/compiler_passthrough.sh "
            "'/tmp/trust target/bin/sccache'",
        )
        self.assertEqual(
            by_id["remote_reclaim_before_test_all"],
            "rm -rf -- '/tmp/trust target' && "
            "mkdir -p -- '/tmp/trust target/tmp' '/tmp/trust target/bin' && "
            "install -m 755 "
            ".codex/skills/trust-ci-release-gates/scripts/compiler_passthrough.sh "
            "'/tmp/trust target/bin/sccache'",
        )
        for command_id in ("remote_vscode", "remote_clippy", "remote_test_all"):
            self.assertIn("CARGO_INCREMENTAL=0", by_id[command_id])
            self.assertIn("RUSTC_WRAPPER=/usr/bin/env", by_id[command_id])
            self.assertIn("CARGO_BUILD_RUSTC_WRAPPER=/usr/bin/env", by_id[command_id])
            self.assertIn("CC=cc", by_id[command_id])
            self.assertIn("CXX=c++", by_id[command_id])
            self.assertIn("TMPDIR='/tmp/trust target/tmp'", by_id[command_id])
            self.assertIn("PATH='/tmp/trust target/bin':$PATH", by_id[command_id])
        self.assertIn("CARGO_BUILD_JOBS=1", by_id["remote_test_all"])

    def test_compiler_passthrough_executes_argv_without_interpreting_its_name(self) -> None:
        passthrough = Path(__file__).with_name("compiler_passthrough.sh")

        result = subprocess.run(
            [passthrough, "sh", "-c", "printf passthrough-ok"],
            check=True,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.stdout, "passthrough-ok")

    def test_default_remote_target_is_an_absolute_validated_generated_path(self) -> None:
        args = guard.parser().parse_args(
            [
                "prepare",
                "--remote-worktree",
                "/home/johannes/projects/exact-validation",
            ]
        )

        commands = candidate_prepare.remote_validation_commands(
            vscode_changed=False,
            remote_target=args.remote_target,
        )

        self.assertIn(
            "CARGO_TARGET_DIR=/home/johannes/.cache/codex-targets/trust-platform-gate",
            dict(commands)["remote_test_all"],
        )

    def test_remote_target_rejects_repository_and_broad_paths(self) -> None:
        for unsafe in (
            "/",
            "/tmp",
            "/home/johannes",
            "/home/johannes/projects/trust-platform",
            "relative/target",
            "/home/johannes/.cache/codex-targets/../source",
        ):
            with self.subTest(unsafe=unsafe), self.assertRaises(ValueError):
                candidate_prepare.remote_validation_commands(
                    vscode_changed=False,
                    remote_target=unsafe,
                )

    def test_exact_candidate_uses_report_smoke_not_exhaustive_tooling(self) -> None:
        maintenance_ids = [
            command_id
            for command_id, _command in candidate_prepare.local_maintenance_commands()
        ]
        self.assertEqual(maintenance_ids, ["catalog_staleness"])

        command = candidate_prepare.strict_report_command(
            base_sha="1" * 40,
            head="2" * 40,
            intent="bugfix",
        )
        self.assertIn("--smoke", command)
        self.assertNotIn("scripts/check_verification_tooling_selftests.py", command)

    def test_metadata_maintenance_is_advisory_but_candidate_integrity_blocks(self) -> None:
        records = [
            {"id": "bootstrap", "exit_status": 0},
            {"id": "clean", "exit_status": 0},
            {"id": "base_ancestor", "exit_status": 0},
            {"id": "diff_check", "exit_status": 0},
            {"id": "planner", "exit_status": 4},
            {"id": "catalog_staleness", "exit_status": 1},
            {"id": "selftests", "exit_status": 1},
        ]
        self.assertTrue(
            candidate_prepare.stage_passed(
                records,
                (
                    "bootstrap",
                    "clean",
                    "base_ancestor",
                    "diff_check",
                    "planner",
                    "catalog_staleness",
                    "selftests",
                ),
            )
        )

        artifact = self.passing_artifact()
        by_id = {row["id"]: row for row in artifact["commands"]}
        for command_id in ("planner", "catalog_staleness", "selftests"):
            row = by_id.get(command_id)
            if row is None:
                row = {
                    "id": command_id,
                    "command": command_id,
                    "exit_status": 1,
                    "output_sha256": "0" * 64,
                    "duration_ms": 1,
                    "scope": "local",
                }
                artifact["commands"].append(row)
            row["exit_status"] = 1
        self.assertEqual(guard.validate_artifact(self.repo, artifact, self.head), [])

        records[0]["exit_status"] = 1
        self.assertFalse(
            candidate_prepare.stage_passed(records, ("bootstrap", "diff_check"))
        )

    def test_failed_artifact_labels_advisory_maintenance_separately(self) -> None:
        records = self.passing_artifact()["commands"]
        for command_id in ("planner", "catalog_staleness"):
            records.append(
                {
                    "id": command_id,
                    "command": command_id,
                    "exit_status": 1,
                    "output_sha256": "0" * 64,
                    "duration_ms": 1,
                    "scope": "local",
                }
            )
        next(row for row in records if row["id"] == "remote_clippy")[
            "exit_status"
        ] = 1

        stderr = io.StringIO()
        with mock.patch("sys.stderr", stderr), mock.patch("sys.stdout", io.StringIO()):
            result = candidate_prepare.finish_artifact(
                self.repo,
                head=self.head,
                base_ref="origin/main",
                base_sha=self.base,
                vscode_changed=False,
                records=records,
                log_dir=self.repo / "logs",
            )

        self.assertEqual(result, 1)
        output = stderr.getvalue()
        self.assertIn("FAILED remote_clippy:", output)
        self.assertIn("ADVISORY planner:", output)
        self.assertIn("ADVISORY catalog_staleness:", output)
        self.assertNotIn("FAILED planner:", output)
        self.assertNotIn("FAILED catalog_staleness:", output)

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
