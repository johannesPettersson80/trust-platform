"""CI's native Test jobs build once (specification 22, "CI reliability and release-gate artifact
contract"). Like the release workflow's tests, these read the workflow's text."""

from __future__ import annotations

import unittest

from scripts.tests.test_release_workflow import CI, ROOT, jobs, step_texts, steps


class CiTestJobTests(unittest.TestCase):
    def setUp(self) -> None:
        self.job = jobs(CI.read_text(encoding="utf-8"))["test"]

    def test_every_crate_builds_once_with_warnings_denied(self) -> None:
        # Catches a Test job that lets a warning through on macOS or Windows, and a step that builds
        # with other compiler flags: the separate trust-runtime check compiled the crate and its
        # dependencies a second time, 1.5, 3.5 and 14 minutes on Linux, macOS and Windows.
        self.assertRegex(self.job, r"(?m)^    env:\n      RUSTFLAGS: -Dwarnings$")
        for text in step_texts(self.job):
            self.assertNotIn("RUSTFLAGS", text)

    def test_every_system_runs_each_programs_tests_in_parallel(self) -> None:
        # Catches a system's whole suite made serial again: Windows ran one test at a time from
        # February 2026, with no recorded reason, which was 56 minutes of #268's 98-minute Windows
        # job (5 November 2026); its trust-runtime unit tests alone took 610 s, against 171 s in
        # parallel on Linux. A test that measures time takes its program's measurement lock instead.
        self.assertNotIn("--test-threads", steps(self.job)["Run full test suite"])

    def test_the_suite_runs_under_nextest_and_each_measuring_test_runs_alone(self) -> None:
        # Catches the suite back on `cargo test`, which runs the programs one after another (#271's
        # suites took 23 to 43 minutes, most of it waiting on single programs), and a test that measures
        # time without its override, which would run beside the tests of every other program
        # (specification 22).
        suite = steps(self.job)["Run full test suite"]
        self.assertIn("cargo nextest run --all-targets --no-fail-fast --profile ci", suite)
        self.assertNotIn("cargo test", suite)
        self.assertIn("cargo install cargo-nextest --version", steps(self.job)["Install cargo-nextest"])
        config = (ROOT / ".config" / "nextest.toml").read_text(encoding="utf-8")
        for measuring in (
            "ci_clean_setup_first_passing_test_is_under_ten_minutes",
            "single_test_feedback_is_under_two_seconds_for_small_project",
            "run_all_tests_scales_roughly_linearly_for_small_projects",
            "binary(web_ide_performance_gate)",
            # The scan supervisor's watchdog tests time real cycles (G56).
            "binary(concurrency_supervisor)",
        ):
            self.assertIn(measuring, config)
        self.assertEqual(config.count('threads-required = "num-test-threads"'), 3)

    def test_the_build_fits_its_budget_on_every_system(self) -> None:
        # Catches the Ubuntu build back on one job: it took 1,339 s against 862 s with two jobs (probe run
        # 42251508042) and ran out of its 1,800 s in a third of main's runs of 6 November 2026; and a macOS
        # budget of 30 minutes, which a macOS build exceeded on main the same day (run 42202513941).
        build = steps(self.job)["Run cargo build --all-targets (retry once on transient failure)"]
        self.assertIn("build_command+=(--jobs 2)", build)
        self.assertNotIn("--jobs 1", build)
        self.assertRegex(build, r'"macos-latest" \]\]; then\n(?:\s*#[^\n]*\n)*\s*build_budget=2700')
        self.assertIn("--build-jobs 2", steps(self.job)["Run full test suite"])

    def test_the_mesh_tls_gate_is_the_last_cargo_step(self) -> None:
        # Catches the gate's own variant of trust-runtime built before the full suite, which then
        # compiled seven crates again: 14.5 minutes on Linux and 21 on macOS (main, 4 November 2026).
        names = list(steps(self.job))
        mesh = names.index("Mesh TLS stability gate")
        self.assertGreater(mesh, names.index("Run full test suite"))
        self.assertGreater(mesh, names.index("Doc tests with checked compile-fail codes"))
        for name in names[mesh + 1 :]:
            self.assertNotRegex(steps(self.job)[name], r"\bcargo\b", name)


if __name__ == "__main__":
    unittest.main()
