"""Tests for the changed-file verification gate."""

from __future__ import annotations

import json
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

from scripts.verification.report_gate import (
    CommandResult,
    DEFAULT_OUTPUT_DIR,
    VerificationReport,
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

    def test_smoke_report_runs_only_report_boundary_tests(self) -> None:
        commands: list[list[str]] = []

        def fake_runner(command: list[str]) -> CommandResult:
            commands.append(command)
            return CommandResult("verification_report_smoke", command, 0, "ok", "")

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "verification").mkdir()
            (root / "verification/test-catalog.toml").write_text("")
            report = build_report(
                root=root,
                changed_files=[],
                intent="bugfix",
                baseline=None,
                command_runner=fake_runner,
                smoke=True,
            )

        self.assertEqual(
            commands,
            [
                [
                    "python3",
                    "-m",
                    "unittest",
                    "scripts.verification.report_gate_tests",
                    "scripts.verification.focused_test_suite_tests",
                ]
            ],
        )
        self.assertEqual([command.name for command in report.commands], ["verification_report_smoke"])
        self.assertEqual(report_exit_code(report, strict=True), 0)

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
            crate = root / "crates/trust-runtime"
            tests = crate / "tests"
            tests.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            )
            known = tests / "known.rs"
            known.write_text("#[test]\nfn known() {}\n")
            new_case = tests / "new_case.rs"
            new_case.write_text("#[test]\nfn new_case() {}\n")
            from scripts.verification.test_catalog_rust import scan_rust_tests

            known_fact = next(
                fact for fact in scan_rust_tests(root).facts if fact.name == "known"
            )
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text(textwrap.dedent(f"""
                [[tests]]
                id = "TEST_KNOWN"
                path = "crates/trust-runtime/tests/known.rs"
                discovery_id = "{known_fact.stable_id}"
            """)
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

    def test_reviewed_nonmapping_fact_is_not_reported_as_uncataloged(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            crate = root / "crates/trust-runtime"
            tests = crate / "tests"
            tests.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            )
            source = tests / "reviewed.rs"
            source.write_text("#[test]\nfn reviewed() {}\n")
            from scripts.verification.test_catalog_rust import scan_rust_tests

            fact = scan_rust_tests(root).facts[0]
            verification = root / "verification"
            verification.mkdir()
            (verification / "test-catalog.toml").write_text("")
            (verification / "test-catalog-denominator.toml").write_text(
                textwrap.dedent(f"""
                    [[reviews]]
                    discovery_id = "{fact.stable_id}"
                    disposition = "reviewed_nonmapping"
                """)
            )

            missing = find_uncataloged_tests(
                root=root,
                changed_files=["crates/trust-runtime/tests/reviewed.rs"],
            )

        self.assertEqual(missing, [])

    def test_unreviewed_fact_in_an_already_reviewed_file_is_uncataloged(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            crate = root / "crates/trust-runtime"
            tests = crate / "tests"
            tests.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            )
            source = tests / "mixed.rs"
            source.write_text(
                "#[test]\nfn reviewed() {}\n\n#[test]\nfn newly_added() {}\n"
            )
            from scripts.verification.test_catalog_rust import scan_rust_tests

            facts = scan_rust_tests(root).facts
            reviewed = next(fact for fact in facts if fact.name == "reviewed")
            verification = root / "verification"
            verification.mkdir()
            (verification / "test-catalog.toml").write_text(
                textwrap.dedent(f"""
                    [[tests]]
                    id = "TEST_REVIEWED"
                    path = "crates/trust-runtime/tests/mixed.rs"
                    discovery_id = "{reviewed.stable_id}"
                """)
            )

            missing = find_uncataloged_tests(
                root=root,
                changed_files=["crates/trust-runtime/tests/mixed.rs"],
            )

        self.assertEqual(missing, ["crates/trust-runtime/tests/mixed.rs"])

    def test_test_support_file_without_a_scanner_fact_is_not_uncataloged(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            crate = root / "crates/trust-runtime"
            tests = crate / "tests"
            tests.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            )
            (tests / "support.rs").write_text("pub fn fixture() {}\n")
            verification = root / "verification"
            verification.mkdir()
            (verification / "test-catalog.toml").write_text("")

            missing = find_uncataloged_tests(
                root=root,
                changed_files=["crates/trust-runtime/tests/support.rs"],
            )

        self.assertEqual(missing, [])

    def test_malformed_catalog_remains_advisory_in_strict_report(self) -> None:
        results = [
            CommandResult("verification_focused_tests", ["focused"], 1, "", "metadata red"),
            CommandResult("verification_metadata_gate", ["metadata"], 1, "", "catalog red"),
            CommandResult("phase16_readiness", ["readiness"], 0, "", ""),
            CommandResult("verification_governance", ["governance"], 0, "", ""),
            CommandResult("plan_tests", ["plan"], 4, '{"verdict":"unmapped"}\n', ""),
        ]

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            crate = root / "crates/trust-runtime"
            tests = crate / "tests"
            tests.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            )
            (tests / "new_case.rs").write_text("#[test]\nfn new_case() {}\n")
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("[[tests]\n", encoding="utf-8")
            try:
                report = build_report(
                    root=root,
                    changed_files=["crates/trust-runtime/tests/new_case.rs"],
                    intent="bugfix",
                    baseline="HEAD~1",
                    command_runner=lambda _command: results.pop(0),
                    enforcing=True,
                )
            except Exception as exc:  # The production boundary must not escape metadata errors.
                self.fail(f"strict report let advisory catalog parsing escape: {exc}")

        self.assertEqual(report.uncataloged_tests, ["crates/trust-runtime/tests/new_case.rs"])
        self.assertEqual(report_exit_code(report, strict=True), 0)

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
            crate = root / "crates/trust-runtime"
            (crate / "tests").mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            )
            (crate / "tests/new_case.rs").write_text(
                "#[test]\nfn new_case() {}\n"
            )
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
        self.assertEqual(report_exit_code(report, strict=True), 0)
        self.assertEqual(report.planner_exit_code, 3)
        self.assertEqual(report.uncataloged_tests, ["crates/trust-runtime/tests/new_case.rs"])
        self.assertEqual(report.commands[2].name, "phase16_readiness")
        self.assertIn("report-only", render_markdown(report))

    def test_enforcing_report_keeps_planner_findings_advisory(self) -> None:
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
        self.assertEqual(report_exit_code(report, strict=True), 0)
        rendered = render_markdown(report)
        self.assertIn("Mode: `enforcing`", rendered)
        self.assertIn("Planner and catalog observations are advisory", rendered)

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
        self.assertIn("Planner finding: advisory maintenance information", render_markdown(report))

    def test_each_verification_maintenance_command_failure_remains_nonblocking(self) -> None:
        for command_name in (
            "verification_focused_tests",
            "verification_tooling_exhaustive",
            "verification_metadata_gate",
        ):
            with self.subTest(command_name=command_name):
                report = VerificationReport(
                    mode="enforcing",
                    intent="bugfix",
                    baseline=None,
                    changed_files=[],
                    commands=[CommandResult(command_name, [command_name], 1, "", "red")],
                    planner_exit_code=None,
                    planner_json=None,
                    uncataloged_tests=[],
                )

                self.assertEqual(report_exit_code(report, strict=True), 0)

    def test_each_advisory_command_failure_remains_nonblocking(self) -> None:
        for command_name in ("phase16_readiness", "verification_governance", "plan_tests"):
            with self.subTest(command_name=command_name):
                report = VerificationReport(
                    mode="enforcing",
                    intent="bugfix",
                    baseline=None,
                    changed_files=[],
                    commands=[CommandResult(command_name, [command_name], 1, "", "red")],
                    planner_exit_code=1 if command_name == "plan_tests" else None,
                    planner_json=None,
                    uncataloged_tests=[],
                )

                self.assertEqual(report_exit_code(report, strict=True), 0)

    def test_unknown_red_command_fails_closed(self) -> None:
        report = VerificationReport(
            mode="enforcing",
            intent="bugfix",
            baseline=None,
            changed_files=[],
            commands=[CommandResult("unexpected_gate", ["unexpected"], 1, "", "red")],
            planner_exit_code=None,
            planner_json=None,
            uncataloged_tests=[],
        )

        self.assertEqual(report_exit_code(report, strict=True), 1)

    def test_enforcing_report_describes_python_verification_as_advisory_not_native(self) -> None:
        report = VerificationReport(
            mode="enforcing",
            intent="bugfix",
            baseline=None,
            changed_files=[],
            commands=[
                CommandResult("verification_focused_tests", ["focused"], 1, "", "red"),
                CommandResult("verification_metadata_gate", ["metadata"], 1, "", "red"),
            ],
            planner_exit_code=None,
            planner_json=None,
            uncataloged_tests=[],
        )

        rendered = render_markdown(report)
        self.assertIn("verification-tooling and metadata commands are advisory", rendered)
        self.assertNotIn("focused native", rendered)

    def test_all_planner_findings_are_visible_but_advisory(self) -> None:
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
        self.assertEqual(report_exit_code(report, strict=True), 0)

        broad_payload = {
            **base,
            "areas": ["bytecode_vm", "runtime_safety"],
            "missing_test_classes": ["runtime_vertical"],
            "missing_test_classes_by_area": {
                "runtime_safety": ["runtime_vertical"],
            },
        }
        self.assertEqual(
            report_exit_code(
                _report_with_planner(broad_payload, planner_exit=2), strict=True
            ),
            0,
        )

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
                    0,
                )

    def test_uncataloged_changed_test_is_visible_but_advisory(self) -> None:
        report = VerificationReport(
            mode="enforcing",
            intent="bugfix",
            baseline=None,
            changed_files=["crates/trust-runtime/tests/new_case.rs"],
            commands=[CommandResult("verification_focused_tests", ["focused"], 0, "ok", "")],
            planner_exit_code=None,
            planner_json=None,
            uncataloged_tests=["crates/trust-runtime/tests/new_case.rs"],
        )

        self.assertEqual(report_exit_code(report, strict=True), 0)
        self.assertIn("Planner and catalog observations are advisory", render_markdown(report))

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
            vscode = root / "editors/vscode"
            suite = vscode / "src/test/suite"
            suite.mkdir(parents=True)
            (vscode / "package.json").write_text('{"name":"trust-vscode"}\n')
            (suite / "new.test.ts").write_text(
                'test("new test", () => {});\n'
            )
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
