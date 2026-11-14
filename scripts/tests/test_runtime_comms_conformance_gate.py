import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
CONFORMANCE_SCRIPT = ROOT / "scripts" / "runtime_comms_conformance_gate.sh"


class RuntimeCommsConformanceGateContractTests(unittest.TestCase):
    def test_mosquitto_version_capture_accepts_help_exit_after_output(self) -> None:
        script = (ROOT / "scripts" / "mqtt_mosquitto_e2e.sh").read_text(
            encoding="utf-8"
        )
        assignment = re.search(r"^mosquitto_version=.*$", script, re.MULTILINE)
        self.assertIsNotNone(assignment, "MQTT harness must capture the broker version")

        with tempfile.TemporaryDirectory() as temp_dir:
            mosquitto = Path(temp_dir) / "mosquitto"
            mosquitto.write_text(
                "#!/usr/bin/env bash\n"
                "echo 'mosquitto version 2.0.18' >&2\n"
                "exit 3\n",
                encoding="utf-8",
            )
            mosquitto.chmod(mosquitto.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env["PATH"] = f"{temp_dir}:{env['PATH']}"
            result = subprocess.run(
                [
                    "bash",
                    "-c",
                    "set -euo pipefail\n"
                    f"{assignment.group(0)}\n"
                    "printf '%s\\n' \"${mosquitto_version}\"\n",
                ],
                cwd=ROOT,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )

        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(result.stdout.strip(), "mosquitto version 2.0.18")

    def test_gate_and_ci_require_real_mosquitto_traffic_light_execution(self) -> None:
        gate = CONFORMANCE_SCRIPT.read_text(encoding="utf-8")
        self.assertIn("scripts/mqtt_mosquitto_e2e.sh", gate)
        self.assertIn("mqtt-mosquitto-traffic-light", gate)

        # The CI job that runs the gate installs the real broker and its clients before it, through
        # `scripts/ci_apt_install.sh` like every Ubuntu package (specification 22). Wrong behaviour caught: a
        # gate job without the broker, or one that installs it only after the gate ran.
        workflow = yaml.load(
            (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8"),
            Loader=yaml.BaseLoader,
        )
        jobs = [
            [step.get("run", "") for step in job.get("steps") or []]
            for job in workflow["jobs"].values()
        ]
        gate_jobs = [
            runs for runs in jobs if any(CONFORMANCE_SCRIPT.name in run for run in runs)
        ]
        self.assertEqual(len(gate_jobs), 1)
        runs = gate_jobs[0]
        gate_step = next(i for i, run in enumerate(runs) if CONFORMANCE_SCRIPT.name in run)
        installs = [
            set(run.split()) for run in runs[:gate_step] if "scripts/ci_apt_install.sh" in run
        ]
        self.assertTrue(
            any({"mosquitto", "mosquitto-clients"} <= words for words in installs), installs
        )

    def test_gate_fails_when_a_case_filter_selects_zero_tests(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cargo = Path(temp_dir) / "cargo"
            cargo.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "echo 'running 0 tests'\n"
                "echo 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out'\n",
                encoding="utf-8",
            )
            cargo.chmod(cargo.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env["PATH"] = f"{temp_dir}:{env['PATH']}"
            env["OUT_DIR"] = str(Path(temp_dir) / "evidence")

            result = subprocess.run(
                ["bash", str(CONFORMANCE_SCRIPT)],
                cwd=ROOT,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("selected zero tests", result.stdout)

    def test_gate_does_not_reuse_passing_logs_from_a_previous_run(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cargo = Path(temp_dir) / "cargo"
            cargo.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "if [[ ${FAKE_CARGO_RESULT:-pass} == pass ]]; then\n"
                "  echo 'running 1 test'\n"
                "  echo 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'\n"
                "else\n"
                "  echo 'running 0 tests'\n"
                "  echo 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out'\n"
                "fi\n",
                encoding="utf-8",
            )
            cargo.chmod(cargo.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env["PATH"] = f"{temp_dir}:{env['PATH']}"
            env["OUT_DIR"] = str(Path(temp_dir) / "evidence")
            passing_external = Path(temp_dir) / "mqtt-e2e"
            passing_external.write_text(
                "#!/usr/bin/env bash\nset -euo pipefail\necho RESULT=PASS\n",
                encoding="utf-8",
            )
            passing_external.chmod(passing_external.stat().st_mode | stat.S_IXUSR)
            env["TRUST_TEST_MQTT_E2E_SCRIPT"] = str(passing_external)
            env["FAKE_CARGO_RESULT"] = "pass"

            first = subprocess.run(
                ["bash", str(CONFORMANCE_SCRIPT)],
                cwd=ROOT,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )
            self.assertEqual(first.returncode, 0, first.stdout)

            env["FAKE_CARGO_RESULT"] = "zero"
            second = subprocess.run(
                ["bash", str(CONFORMANCE_SCRIPT)],
                cwd=ROOT,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )

        self.assertNotEqual(second.returncode, 0, second.stdout)
        self.assertIn("t0_shm_bind_contract_mismatch selected zero tests", second.stdout)


if __name__ == "__main__":
    unittest.main()
