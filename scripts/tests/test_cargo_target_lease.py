import os
import signal
import subprocess
import tempfile
import time
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WITH_LEASE = ROOT / "scripts" / "with_cargo_target_lease.sh"
REMOVE_IDLE = ROOT / "scripts" / "remove_cargo_target_if_idle.sh"


class CargoTargetLeaseTests(unittest.TestCase):
    def test_detached_child_does_not_inherit_command_lease(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-target-lease-", dir="/tmp") as root:
            target = Path(root) / "target"
            pid_file = Path(root) / "child.pid"
            target.mkdir()
            child_pid: int | None = None
            try:
                completed = subprocess.run(
                    [
                        str(WITH_LEASE),
                        str(target),
                        "sh",
                        "-c",
                        'sleep 30 >/dev/null 2>&1 & echo "$!" > "$1"',
                        "sh",
                        str(pid_file),
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=5,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)
                child_pid = int(pid_file.read_text(encoding="utf-8").strip())
                os.kill(child_pid, 0)

                removed = subprocess.run(
                    [str(REMOVE_IDLE), str(target)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(removed.returncode, 0, removed.stderr)
                self.assertFalse(target.exists())
            finally:
                if child_pid is not None:
                    try:
                        os.kill(child_pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass

    def test_cleanup_skips_live_target_then_removes_it_after_command_exits(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-target-lease-", dir="/tmp") as root:
            target = Path(root) / "target"
            target.mkdir()
            sentinel = target / "sentinel"
            sentinel.write_text("live", encoding="utf-8")
            holder = subprocess.Popen(
                [str(WITH_LEASE), str(target), "sleep", "2"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            try:
                time.sleep(0.2)
                blocked = subprocess.run(
                    [str(REMOVE_IDLE), str(target)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(blocked.returncode, 75, blocked.stderr)
                self.assertTrue(sentinel.is_file())
                self.assertEqual(holder.wait(timeout=5), 0)
                removed = subprocess.run(
                    [str(REMOVE_IDLE), str(target)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(removed.returncode, 0, removed.stderr)
                self.assertFalse(target.exists())
            finally:
                if holder.poll() is None:
                    holder.terminate()
                    holder.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
