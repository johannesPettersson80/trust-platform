import subprocess
import tempfile
import time
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WITH_LEASE = ROOT / "scripts" / "with_cargo_target_lease.sh"
REMOVE_IDLE = ROOT / "scripts" / "remove_cargo_target_if_idle.sh"


class CargoTargetLeaseTests(unittest.TestCase):
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
