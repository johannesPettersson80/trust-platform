import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import textwrap
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
CONFORMANCE_SCRIPT = ROOT / "scripts" / "runtime_comms_conformance_gate.sh"
MQTT_SCRIPT = ROOT / "scripts" / "mqtt_mosquitto_e2e.sh"

# Stand-ins for the programs the MQTT case runs, so that it runs here without a broker or a build.
# The runtime reports `ready` for its first FAKE_READY_READS status reads, as a runtime does between
# opening its control endpoint and opening its scan start gate, then `running`.
MQTT_STAND_INS = {
    "cargo": "#!/usr/bin/env bash\nexit 0\n",
    "mosquitto": textwrap.dedent(
        """\
        #!/usr/bin/env python3
        import socket
        import sys

        if "-h" in sys.argv:
            print("mosquitto version 2.0.18", file=sys.stderr)
            sys.exit(3)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", int(sys.argv[sys.argv.index("-p") + 1])))
            listener.listen()
            while True:
                listener.accept()[0].close()
        """
    ),
    "mosquitto_sub": textwrap.dedent(
        """\
        #!/usr/bin/env bash
        for light in green yellow red; do
          for _ in $(seq 1 10); do
            echo "traffic/north/${light} true"
            echo "traffic/north/${light} false"
          done
        done
        """
    ),
    "target/debug/trust-runtime": textwrap.dedent(
        """\
        #!/usr/bin/env python3
        import os
        from pathlib import Path
        import re
        import socket
        import sys
        import time

        args = sys.argv[1:]
        if args[0] == "build":
            sys.exit(0)
        if args[0] == "run":
            project = Path(args[args.index("--project") + 1])
            socket_path = project / ".trust-runtime" / "control.sock"
            socket_path.parent.mkdir(exist_ok=True)
            path = str(socket_path)
            with socket.socket(socket.AF_UNIX) as server:
                server.bind(path)
                while not Path(path + ".stop").exists():
                    time.sleep(0.05)
            sys.exit(0)
        path = args[args.index("--endpoint") + 1].removeprefix("unix://")
        if "shutdown" in args:
            Path(path + ".stop").touch()
            sys.exit(0)
        reads = Path(path + ".reads")
        count = int(reads.read_text()) if reads.exists() else 0
        reads.write_text(str(count + 1))
        state = "ready" if count < int(os.environ["FAKE_READY_READS"]) else "running"
        print(f"state={state} fault=none rt_profile=disabled rt_active=false")
        """
    ),
}


class RuntimeCommsConformanceGateContractTests(unittest.TestCase):
    def run_mqtt_case(self, ready_reads: int) -> tuple[subprocess.CompletedProcess[str], str]:
        with tempfile.TemporaryDirectory() as temp_dir:
            for name, source in MQTT_STAND_INS.items():
                program = Path(temp_dir) / name
                program.parent.mkdir(parents=True, exist_ok=True)
                program.write_text(source, encoding="utf-8")
                program.chmod(program.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env["PATH"] = f"{temp_dir}:{env['PATH']}"
            env["CARGO_TARGET_DIR"] = str(Path(temp_dir) / "target")
            env["OUT_DIR"] = str(Path(temp_dir) / "evidence")
            env["FAKE_READY_READS"] = str(ready_reads)
            result = subprocess.run(
                ["bash", str(MQTT_SCRIPT)],
                cwd=ROOT,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
                timeout=60,
            )
            status = (Path(temp_dir) / "evidence" / "status.log").read_text(encoding="utf-8")
        return result, status

    def test_mqtt_case_waits_while_the_runtime_is_ready(self) -> None:
        # Wrong behaviour caught: the case read the status once, just after the control socket
        # appeared, and failed a runtime that was still `ready` because it had not opened its scan
        # start gate yet (G185: CI run 43716028760 kept `state=ready fault=none` and failed).
        result, status = self.run_mqtt_case(ready_reads=3)

        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("RESULT=PASS", result.stdout)
        self.assertTrue(status.startswith("state=running fault=none "), status)

    def test_mqtt_case_fails_a_runtime_that_never_runs(self) -> None:
        # Wrong behaviour caught: a wait without an end, or one that passes a runtime that stays
        # `ready`; the failure names the state the runtime reported.
        result, status = self.run_mqtt_case(ready_reads=1000)

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("runtime is not running: state=ready fault=none", result.stdout)
        self.assertTrue(status.startswith("state=ready "), status)

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
