"""The runtime mesh TLS stability gate fails a run that passed without running its tests (checklist W38).

The gate repeats the mesh TLS tests by filter. Its filter once named a test that had been deleted, so every run reported
"0 passed ... 4423 filtered out" and the gate passed without testing anything. The gate now checks each run's log for a
passing test of every filter; `--check-log` runs that check alone, on a log the test writes.
"""

import subprocess
import tempfile
import unittest
from pathlib import Path

GATE = Path(__file__).resolve().parents[1] / "runtime_mesh_tls_stability_gate.sh"

PASSING = [
    "test mesh::tests::mesh_tls_requires_certificate_materials_instead_of_starting_plaintext ... ok",
    "test mesh::security_tests::mesh_tls_socket_presents_the_configured_certificate ... ok",
    "test mesh::hmi_remote_tests::two_tls_peers_read_and_write_only_published_owner_authorized_variables ... ok",
]


def check(lines):
    """The gate's `--check-log` exit status for a run log of `lines`."""
    with tempfile.TemporaryDirectory() as folder:
        log = Path(folder) / "run.log"
        log.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return subprocess.run(
            ["bash", str(GATE), "--check-log", str(log)],
            capture_output=True,
            text=True,
            check=False,
        ).returncode


class RuntimeMeshTlsGateTests(unittest.TestCase):
    def test_a_run_that_ran_no_test_fails(self):
        # Catches W38: main's macOS run 41990796998 logged exactly this and the gate passed.
        log = [
            "running 0 tests",
            "",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4423 filtered out; finished in 0.00s",
        ]
        self.assertEqual(check(log), 1)

    def test_a_run_that_passed_a_test_of_every_filter_passes(self):
        log = ["running 3 tests", *PASSING, "", "test result: ok. 3 passed; 0 failed; 0 ignored"]
        self.assertEqual(check(log), 0)

    def test_a_run_missing_one_filter_fails(self):
        # Catches a gate that checks only that something ran: one renamed test leaves its filter empty.
        log = ["running 2 tests", *PASSING[:2], "", "test result: ok. 2 passed; 0 failed; 0 ignored"]
        self.assertEqual(check(log), 1)


if __name__ == "__main__":
    unittest.main()
