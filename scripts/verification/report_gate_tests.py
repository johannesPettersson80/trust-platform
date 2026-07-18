"""Tests for the changed-file verification gate."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.report_gate import (
    CommandResult,
    DEFAULT_OUTPUT_DIR,
    build_report,
    changed_files_from_git,
    find_uncataloged_tests,
    parse_name_status_z,
    report_exit_code,
    render_markdown,
)


class VerificationReportGateTests(unittest.TestCase):
    def test_default_output_dir_lives_under_target(self) -> None:
        self.assertEqual(DEFAULT_OUTPUT_DIR, Path("target/gate-artifacts/verification"))

    def test_changed_files_from_git_uses_merge_base(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            run_git(repo, "init")
            run_git(repo, "config", "user.email", "verification@example.invalid")
            run_git(repo, "config", "user.name", "Verification Test")
            (repo / "shared.txt").write_text("base\n")
            run_git(repo, "add", ".")
            run_git(repo, "commit", "-m", "base")

            run_git(repo, "checkout", "-b", "feature")
            (repo / "feature.txt").write_text("feature\n")
            run_git(repo, "add", ".")
            run_git(repo, "commit", "-m", "feature change")
            feature_head = run_git(repo, "rev-parse", "HEAD").stdout.strip()

            run_git(repo, "checkout", "master")
            (repo / "shared.txt").write_text("main\n")
            run_git(repo, "add", ".")
            run_git(repo, "commit", "-m", "main change")
            base_tip = run_git(repo, "rev-parse", "HEAD").stdout.strip()

            changed = changed_files_from_git(repo, base_tip, feature_head)

        self.assertEqual(changed, ["feature.txt"])

    def test_changed_files_from_git_includes_deleted_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            run_git(repo, "init")
            run_git(repo, "config", "user.email", "verification@example.invalid")
            run_git(repo, "config", "user.name", "Verification Test")
            deleted = repo / "crates/trust-runtime/src/runtime/deleted.rs"
            deleted.parent.mkdir(parents=True)
            deleted.write_text("pub fn removed() {}\n")
            run_git(repo, "add", ".")
            run_git(repo, "commit", "-m", "base")
            base = run_git(repo, "rev-parse", "HEAD").stdout.strip()

            deleted.unlink()
            run_git(repo, "add", "-u")
            run_git(repo, "commit", "-m", "delete runtime source")
            head = run_git(repo, "rev-parse", "HEAD").stdout.strip()

            changed = changed_files_from_git(repo, base, head)

        self.assertEqual(changed, ["crates/trust-runtime/src/runtime/deleted.rs"])

    def test_changed_files_from_git_includes_both_rename_endpoints(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            run_git(repo, "init")
            run_git(repo, "config", "user.email", "verification@example.invalid")
            run_git(repo, "config", "user.name", "Verification Test")
            old_path = "crates/trust-hir/src/old_owner.rs"
            new_path = "crates/trust-runtime/src/runtime/new_owner.rs"
            source = repo / old_path
            source.parent.mkdir(parents=True)
            source.write_text("pub fn moved() {}\n")
            run_git(repo, "add", ".")
            run_git(repo, "commit", "-m", "base")
            base = run_git(repo, "rev-parse", "HEAD").stdout.strip()

            (repo / new_path).parent.mkdir(parents=True)
            run_git(repo, "mv", old_path, new_path)
            run_git(repo, "commit", "-m", "move ownership")
            head = run_git(repo, "rev-parse", "HEAD").stdout.strip()

            changed = changed_files_from_git(repo, base, head)

        self.assertEqual(changed, sorted([old_path, new_path]))

    def test_name_status_parser_preserves_unusual_paths_for_default_deny(self) -> None:
        paths = parse_name_status_z(
            "M\0path with spaces.rs\0D\0path-with-tab\t.rs\0"
            "R100\0old\nname.rs\0new\nname.rs\0"
        )

        self.assertEqual(
            paths,
            sorted(
                [
                    "path with spaces.rs",
                    "path-with-tab\t.rs",
                    "old\nname.rs",
                    "new\nname.rs",
                ]
            ),
        )

    def test_name_status_parser_rejects_truncated_rename(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "malformed NUL-delimited"):
            parse_name_status_z("R100\0old.rs\0")

    def test_uncataloged_test_report_uses_catalog_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text(
                """
[[tests]]
id = "TEST_KNOWN"
path = "crates/trust-runtime/tests/known.rs"
"""
            )

            missing = find_uncataloged_tests(
                root=root,
                changed_files=[
                    "crates/trust-runtime/tests/known.rs",
                    "crates/trust-runtime/tests/new_case.rs",
                    "crates/trust-runtime/src/lib.rs",
                    "scripts/verification/new_contract_tests.py",
                ],
            )

        self.assertEqual(missing, ["crates/trust-runtime/tests/new_case.rs"])

    def test_build_report_is_report_only_when_gate_and_planner_fail(self) -> None:
        results = [
            CommandResult("verification_focused_tests", ["focused"], 1, "tests failed", ""),
            CommandResult("verification_metadata_gate", ["gate"], 1, "metadata failed", ""),
            CommandResult("phase16_readiness", ["readiness"], 1, "product change blocked", ""),
            CommandResult("verification_governance", ["governance"], 1, "governance failed", ""),
            CommandResult("plan_tests", ["plan"], 3, '{"verdict":"spec_gap"}\n', ""),
        ]

        def fake_runner(command: list[str]) -> CommandResult:
            return results.pop(0)

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("")
            report = build_report(
                root=root,
                changed_files=["crates/trust-runtime/tests/new_case.rs"],
                intent="bugfix",
                baseline="HEAD~1",
                command_runner=fake_runner,
            )

        self.assertEqual(report_exit_code(report, strict=False), 0)
        self.assertEqual(report_exit_code(report, strict=True), 1)
        self.assertEqual(report.planner_exit_code, 3)
        self.assertEqual(report.uncataloged_tests, ["crates/trust-runtime/tests/new_case.rs"])
        self.assertEqual(report.commands[2].name, "phase16_readiness")
        self.assertIn("report-only", render_markdown(report))

    def test_enforcing_report_is_labeled_and_fails_on_red_commands(self) -> None:
        results = [
            CommandResult("verification_focused_tests", ["focused"], 0, "ok", ""),
            CommandResult("verification_metadata_gate", ["gate"], 0, "ok", ""),
            CommandResult("phase16_readiness", ["readiness"], 0, "ok", ""),
            CommandResult("verification_governance", ["governance"], 0, "ok", ""),
            CommandResult("plan_tests", ["plan"], 2, '{"verdict":"missing_tests"}\n', ""),
        ]

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("")
            report = build_report(
                root=root,
                changed_files=["crates/trust-runtime/src/runtime/cycle.rs"],
                intent="bugfix",
                baseline="HEAD~1",
                command_runner=lambda _command: results.pop(0),
                enforcing=True,
            )

        self.assertEqual(report.mode, "enforcing")
        self.assertEqual(report_exit_code(report, strict=True), 1)
        rendered = render_markdown(report)
        self.assertIn("Mode: `enforcing`", rendered)
        self.assertIn("blocks merge", rendered)

    def test_non_bytecode_test_class_debt_is_visible_but_not_a_false_block(self) -> None:
        results = [
            CommandResult("verification_focused_tests", ["focused"], 0, "ok", ""),
            CommandResult("verification_metadata_gate", ["gate"], 0, "ok", ""),
            CommandResult("phase16_readiness", ["readiness"], 0, "ok", ""),
            CommandResult("verification_governance", ["governance"], 0, "ok", ""),
            CommandResult(
                "plan_tests",
                ["plan"],
                2,
                '{"areas":["runtime_safety"],"verdict":"missing_tests",'
                '"missing_test_classes":["runtime_vertical"],"spec_gaps":[],'
                '"unmapped_files":[],"unknown_areas":[],"uninventoried_areas":[]}\n',
                "",
            ),
        ]

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("")
            report = build_report(
                root=root,
                changed_files=["crates/trust-runtime/src/runtime/cycle.rs"],
                intent="bugfix",
                baseline=None,
                command_runner=lambda _command: results.pop(0),
                enforcing=True,
            )

        self.assertEqual(report_exit_code(report, strict=True), 0)
        self.assertIn("advisory outside the bytecode/VM test-class ratchet", render_markdown(report))

    def test_bytecode_test_class_debt_and_global_planner_findings_block(self) -> None:
        base = {
            "areas": ["bytecode_vm"],
            "verdict": "missing_tests",
            "missing_test_classes": ["metadata_validation"],
            "spec_gaps": [],
            "unmapped_files": [],
            "unknown_areas": [],
            "uninventoried_areas": [],
        }
        report = _report_with_planner(base, planner_exit=2)
        self.assertEqual(report_exit_code(report, strict=True), 1)

        for field, value in (
            ("spec_gaps", ["SPEC_GAP_X"]),
            ("unmapped_files", ["unknown/path"]),
            ("unknown_areas", ["unknown"]),
            ("uninventoried_areas", ["runtime_safety"]),
        ):
            with self.subTest(field=field):
                payload = {**base, "areas": ["runtime_safety"], "missing_test_classes": []}
                payload[field] = value
                self.assertEqual(
                    report_exit_code(_report_with_planner(payload, planner_exit=3), strict=True),
                    1,
                )

    def test_build_report_routes_product_paths_through_phase16_readiness(self) -> None:
        commands: list[list[str]] = []

        def fake_runner(command: list[str]) -> CommandResult:
            commands.append(command)
            return CommandResult("fixture", command, 0, "", "")

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("")
            report = build_report(
                root=root,
                changed_files=["crates/trust-runtime/src/stdlib/timers.rs"],
                intent="bugfix",
                baseline=None,
                command_runner=fake_runner,
                run_planner=False,
            )

        self.assertEqual(len(report.commands), 4)
        self.assertEqual(
            commands[2],
            [
                "python3",
                "-m",
                "scripts.verification.phase16_readiness",
                "--changed-file=crates/trust-runtime/src/stdlib/timers.rs",
            ],
        )
        self.assertEqual(commands[3][:3], ["python3", "-m", "scripts.verification.governance"])
        self.assertIn(
            "--changed-file=crates/trust-runtime/src/stdlib/timers.rs",
            commands[3],
        )
        self.assertEqual(report_exit_code(report, strict=False), 0)

    def test_report_json_is_stable_and_includes_uncataloged_tests(self) -> None:
        result = CommandResult("verification_metadata_gate", ["gate"], 0, "ok", "")

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("")
            report = build_report(
                root=root,
                changed_files=["editors/vscode/src/test/suite/new.test.ts"],
                intent="docs",
                baseline=None,
                command_runner=lambda _command: result,
                run_planner=False,
            )

        payload = json.loads(report.to_json())
        self.assertEqual(payload["intent"], "docs")
        self.assertEqual(
            payload["uncataloged_tests"],
            ["editors/vscode/src/test/suite/new.test.ts"],
        )
        self.assertEqual(payload["commands"][0]["exit_code"], 0)


def run_git(repo: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )


def _report_with_planner(payload: dict[str, object], *, planner_exit: int):
    from scripts.verification.report_gate import VerificationReport

    return VerificationReport(
        mode="enforcing",
        intent="bugfix",
        baseline=None,
        changed_files=["fixture"],
        commands=[
            CommandResult("verification_focused_tests", ["focused"], 0, "ok", ""),
            CommandResult("verification_metadata_gate", ["gate"], 0, "ok", ""),
            CommandResult("phase16_readiness", ["readiness"], 0, "ok", ""),
            CommandResult("verification_governance", ["governance"], 0, "ok", ""),
            CommandResult("plan_tests", ["plan"], planner_exit, json.dumps(payload), ""),
        ],
        planner_exit_code=planner_exit,
        planner_json=payload,
        uncataloged_tests=[],
    )


if __name__ == "__main__":
    unittest.main()
