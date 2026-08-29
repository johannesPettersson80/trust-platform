import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CONFORMANCE_SCRIPT = ROOT / "scripts" / "runtime_comms_conformance_gate.sh"


class RuntimeCommsConformanceGateContractTests(unittest.TestCase):
    def test_gate_and_ci_require_real_mosquitto_traffic_light_execution(self) -> None:
        gate = CONFORMANCE_SCRIPT.read_text(encoding="utf-8")
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")

        self.assertIn("scripts/mqtt_mosquitto_e2e.sh", gate)
        self.assertIn("mqtt-mosquitto-traffic-light", gate)
        self.assertIn("apt-get install -y mosquitto mosquitto-clients", workflow)

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
