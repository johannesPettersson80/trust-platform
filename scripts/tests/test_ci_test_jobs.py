"""CI's native Test jobs build once, and the Windows job keeps a dump of every crashing process
(specification 22, "CI reliability and release-gate artifact contract"). Like the release workflow's
tests, these read the workflow's text."""

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
            # So do the debugger's pause tests, against a 20 ms watchdog (G58).
            "binary(debug_pause_watchdog) | binary(debug_pause_watchdog_trace_cases)",
            # The hung-driver tests time the supervisor's record of a hang (G74).
            "binary(failure_injection_hung_driver_watchdog)",
            # Accelerated scans against a 200 ms real-time watchdog (G74).
            "binary(simulation_workflow) & test(=accelerated_clock_keeps_watchdog_semantics)",
        ):
            self.assertIn(measuring, config)
        self.assertEqual(config.count('threads-required = "num-test-threads"'), 6)

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

    def test_windows_keeps_a_minidump_of_every_crashing_process_from_before_the_build(self) -> None:
        # Catches a Windows job that keeps no dump: #313's test process crashed with 0xc0000005, the
        # suite's second attempt passed, and nothing named the faulting code (G81); and dumps switched
        # on after the suite, which keep none of its crashes; and a Windows Error Reporting dialog, which
        # would hold a crashed test until the suite's two-hour limit, since nextest's ci profile gives a
        # test no timeout of its own. Specification 22: a minidump (`DumpType` 1) of every process that
        # crashes during the job, the 20 newest (`DumpCount`), without a dialog (`DontShowUI`).
        names = list(steps(self.job))
        keep = steps(self.job)["Keep a dump of every crashing process (Windows)"]
        self.assertIn("if: ${{ matrix.os == 'windows-latest' }}", keep)
        self.assertIn(r"HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps", keep)
        self.assertIn(r'-Name DumpFolder -Value "$env:RUNNER_TEMP\crash-dumps"', keep)
        self.assertIn("-Name DumpType -Value 1 -PropertyType DWord", keep)
        self.assertIn("-Name DumpCount -Value 20 -PropertyType DWord", keep)
        self.assertIn(
            r'"HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting" -Name DontShowUI -Value 1 -PropertyType DWord',
            keep,
        )
        self.assertLess(
            names.index("Keep a dump of every crashing process (Windows)"),
            names.index("Run cargo build --all-targets (retry once on transient failure)"),
        )

    def test_the_build_and_the_suite_start_where_their_crashes_reach_windows_error_reporting(self) -> None:
        # Catches the build or the suite started straight from the step's shell: their processes inherit
        # the runner's SEM_NOGPFAULTERRORBOX, Windows then never hands a crash to Windows Error Reporting,
        # and no dump is kept (G81b, probe run 42747857506: error mode 0x3 in every step and no dump, but
        # a dump once the command's error mode lost the flag). And a Windows job that does not run
        # run_with_progress.py's self-test, which fails when its command keeps the flag, before the
        # build, or lets that test fail without failing the job: the dumps would stop without a sign.
        names = list(steps(self.job))
        build = "Run cargo build --all-targets (retry once on transient failure)"
        for name in (build, "Run full test suite"):
            self.assertIn("scripts/run_with_progress.py", steps(self.job)[name], name)
        check_name = "Check that the build's and the suite's crashes reach Windows Error Reporting (Windows)"
        check = steps(self.job)[check_name]
        self.assertIn("if: ${{ matrix.os == 'windows-latest' }}", check)
        self.assertIn("run: python scripts/run_with_progress.py --self-test", check)
        self.assertNotIn("continue-on-error", check)
        self.assertLess(names.index(check_name), names.index(build))

    def test_after_the_tests_windows_names_each_crash_and_keeps_the_dumps_even_in_a_green_job(self) -> None:
        # Catches the report or the upload skipped in a job that passed (`always()` lost), when the
        # suite's second attempt hid the crash, as in #313; a report that names no faulting frame; a
        # run without a warning, where nobody would open the artifact of a green job; and an upload of
        # another folder than the one Windows Error Reporting writes to. Specification 22: `cdb` names
        # the exception, faulting module and stack of the first five dumps, and the dumps with those
        # reports are the artifact `crash-dumps-windows-latest`.
        names = list(steps(self.job))
        suite = names.index("Run full test suite")
        report = steps(self.job)["Name the faulting frame of each crash dump (Windows)"]
        self.assertGreater(names.index("Name the faulting frame of each crash dump (Windows)"), suite)
        self.assertIn("if: ${{ always() && matrix.os == 'windows-latest' }}", report)
        self.assertIn(r'Get-ChildItem "$env:RUNNER_TEMP\crash-dumps" -Filter *.dmp', report)
        self.assertIn("Select-Object -First 5", report)
        self.assertIn('-logo $report -c "!analyze -v; .ecxr; kb 40; q"', report)
        self.assertIn("Join-Path $dump.DirectoryName", report)
        self.assertIn("::warning title=Crash dumps::", report)
        upload = steps(self.job)["Upload crash dumps (Windows)"]
        self.assertGreater(
            names.index("Upload crash dumps (Windows)"),
            names.index("Name the faulting frame of each crash dump (Windows)"),
        )
        self.assertIn("if: ${{ always() && matrix.os == 'windows-latest' }}", upload)
        self.assertIn("name: crash-dumps-${{ matrix.os }}", upload)
        self.assertIn("path: ${{ runner.temp }}/crash-dumps/", upload)
        self.assertIn("if-no-files-found: ignore", upload)

    def test_no_crash_dump_step_fails_the_job_and_a_failed_upload_is_tried_once_more(self) -> None:
        # Catches a dump step that turns a job whose tests passed red, for a failed upload or a runner
        # without cdb (specification 22: the dumps are evidence, and the job's verdict is its tests'); a
        # symbol download that hangs and holds the job until GitHub's six-hour limit (cdb gets 15
        # minutes); and dumps lost to one failed upload, which the test log's upload already survives
        # (G107).
        self.assertIn("timeout-minutes: 15", steps(self.job)["Name the faulting frame of each crash dump (Windows)"])
        for name in (
            "Keep a dump of every crashing process (Windows)",
            "Name the faulting frame of each crash dump (Windows)",
            "Upload crash dumps (Windows)",
            "Upload crash dumps (Windows, second attempt)",
        ):
            self.assertIn("continue-on-error: true", steps(self.job)[name], name)
        self.assertIn("id: crash-dumps", steps(self.job)["Upload crash dumps (Windows)"])
        second = steps(self.job)["Upload crash dumps (Windows, second attempt)"]
        self.assertIn(
            "if: ${{ always() && matrix.os == 'windows-latest' && steps.crash-dumps.outcome == 'failure' }}", second
        )
        self.assertIn("name: crash-dumps-${{ matrix.os }}", second)
        self.assertIn("overwrite: true", second)
        self.assertIn("path: ${{ runner.temp }}/crash-dumps/", second)


if __name__ == "__main__":
    unittest.main()
