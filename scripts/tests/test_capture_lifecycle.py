from pathlib import Path
import os
import signal
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
CAPTURE_RUNNER = ROOT / "scripts" / "captures" / "run-playwright-captures.sh"


def write_executable(path: Path, text: str) -> None:
    path.write_text(text)
    path.chmod(0o755)


class CaptureLifecycleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory(prefix="trust-capture-lifecycle-")
        self.root = Path(self.temp_dir.name)
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        self.docker_log = self.root / "docker.log"
        self.child_pid_file = self.root / "child.pid"

        write_executable(self.bin_dir / "cargo", "#!/usr/bin/env bash\nexit 0\n")
        write_executable(
            self.bin_dir / "docker",
            "#!/usr/bin/env bash\n"
            "printf '%s\\n' \"$*\" >>\"$CAPTURE_TEST_DOCKER_LOG\"\n"
            "exit 0\n",
        )

    def tearDown(self) -> None:
        if self.child_pid_file.exists():
            child_pid = int(self.child_pid_file.read_text().strip())
            try:
                os.kill(child_pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        self.temp_dir.cleanup()

    def environment(self) -> dict[str, str]:
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{self.bin_dir}:{env['PATH']}",
                "CAPTURE_TEST_DOCKER_LOG": str(self.docker_log),
                "CAPTURE_TEST_CHILD_PID": str(self.child_pid_file),
                "TRUST_CAPTURE_CODESERVER_CONTAINER": "trust-capture-lifecycle-test",
            }
        )
        return env

    def install_npm(self, capture_body: str) -> None:
        write_executable(
            self.bin_dir / "npm",
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            "if [[ \" $* \" == *\" capture:vscode \"* ]]; then\n"
            f"{capture_body}\n"
            "fi\n"
            "exit 0\n",
        )

    def test_success_removes_owned_code_server_container_after_capture(self) -> None:
        self.install_npm(":")

        result = subprocess.run(
            [str(CAPTURE_RUNNER), "vscode"],
            cwd=ROOT,
            env=self.environment(),
            text=True,
            capture_output=True,
            timeout=15,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        removals = [
            line
            for line in self.docker_log.read_text().splitlines()
            if line == "rm -f trust-capture-lifecycle-test"
        ]
        self.assertGreaterEqual(
            len(removals),
            2,
            "capture launcher must remove its code-server container before and after use",
        )

    def test_failure_preserves_status_and_removes_owned_container(self) -> None:
        self.install_npm("exit 23")

        result = subprocess.run(
            [str(CAPTURE_RUNNER), "vscode"],
            cwd=ROOT,
            env=self.environment(),
            text=True,
            capture_output=True,
            timeout=15,
            check=False,
        )

        self.assertEqual(result.returncode, 23, result.stderr)
        removals = [
            line
            for line in self.docker_log.read_text().splitlines()
            if line == "rm -f trust-capture-lifecycle-test"
        ]
        self.assertGreaterEqual(len(removals), 2)

    def test_termination_reaps_owned_capture_process_session(self) -> None:
        self.install_npm(
            "python3 -c 'import os, signal, time; "
            "os.setpgid(0, 0); "
            "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
            "open(os.environ[\"CAPTURE_TEST_CHILD_PID\"], \"w\").write(str(os.getpid())); "
            "time.sleep(300)' &\n"
            "child_pid=$!\n"
            "wait \"$child_pid\""
        )

        process = subprocess.Popen(
            [str(CAPTURE_RUNNER), "vscode"],
            cwd=ROOT,
            env=self.environment(),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 10
            while not self.child_pid_file.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            self.assertTrue(self.child_pid_file.exists(), "fake capture child did not start")
            child_pid = int(self.child_pid_file.read_text().strip())

            process.terminate()
            try:
                process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                self.fail("capture launcher did not terminate after SIGTERM")

            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                try:
                    os.kill(child_pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.05)
            else:
                self.fail("owned capture child survived launcher termination")
        finally:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate(timeout=5)
            else:
                if process.stdout is not None:
                    process.stdout.close()
                if process.stderr is not None:
                    process.stderr.close()


if __name__ == "__main__":
    unittest.main()
