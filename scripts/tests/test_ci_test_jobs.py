"""CI's native Test jobs build once (specification 22, "CI reliability and release-gate artifact
contract"). Like the release workflow's tests, these read the workflow's text."""

from __future__ import annotations

import tomllib
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
        ):
            self.assertIn(measuring, config)
        self.assertEqual(config.count('threads-required = "num-test-threads"'), 4)

    def test_the_mesh_tls_gate_is_the_last_cargo_step(self) -> None:
        # Catches the gate's own variant of trust-runtime built before the full suite, which then
        # compiled seven crates again: 14.5 minutes on Linux and 21 on macOS (main, 4 November 2026).
        names = list(steps(self.job))
        mesh = names.index("Mesh TLS stability gate")
        self.assertGreater(mesh, names.index("Run full test suite"))
        self.assertGreater(mesh, names.index("Doc tests with checked compile-fail codes"))
        for name in names[mesh + 1 :]:
            self.assertNotRegex(steps(self.job)[name], r"\bcargo\b", name)


class NativeTlsIdentityTests(unittest.TestCase):
    def test_on_windows_the_tests_that_build_a_native_tls_identity_run_one_at_a_time(self) -> None:
        # Catches the MQTT tests back in parallel on Windows: native-tls names the key container of each
        # identity by a count kept in its own process, so two test processes used `native-tls-0` at the
        # same time and tls_transport_test_builds_the_native_tls_client failed (#286's run, G59).
        config = tomllib.loads((ROOT / ".config" / "nextest.toml").read_text(encoding="utf-8"))
        self.assertEqual(config.get("test-groups", {}).get("native-tls-identity"), {"max-threads": 1})
        grouped = [
            override
            for override in config["profile"]["ci"]["overrides"]
            if override.get("test-group") == "native-tls-identity"
        ]
        self.assertEqual(len(grouped), 1)
        self.assertEqual(grouped[0]["platform"], "cfg(windows)")
        self.assertEqual(
            grouped[0]["filter"],
            "package(trust-runtime) & (test(/mqtt/) | binary(device_in_the_loop))",
        )


if __name__ == "__main__":
    unittest.main()
