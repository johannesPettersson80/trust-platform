"""CI's native Test jobs build once (specification 22, "CI reliability and release-gate artifact
contract"). Like the release workflow's tests, these read the workflow's text."""

from __future__ import annotations

import unittest

from scripts.tests.test_release_workflow import CI, jobs, step_texts, steps


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
