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
PATH_POLICY = ROOT / "scripts" / "cargo_target_path.sh"
VOLUME_ROOT = Path("/mnt/HC_Volume_107089260/builder-storage/cargo-targets")


class CargoTargetLeaseTests(unittest.TestCase):
    def test_alias_and_traversal_paths_cannot_reclaim_another_target(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-target-path-", dir="/tmp") as temporary:
            root = Path(temporary)
            kept = root / "kept"
            kept.mkdir()
            (kept / "sentinel").write_text("keep")
            (root / "child").mkdir()
            alias = root / "alias"
            alias.symlink_to(kept, target_is_directory=True)
            for candidate in (str(alias), str(root / "child") + "/../kept"):
                for script in (WITH_LEASE, REMOVE_IDLE):
                    args = [str(script), candidate]
                    if script == WITH_LEASE:
                        args.append("true")
                    result = subprocess.run(args, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 2, result.stderr)
                    self.assertEqual((kept / "sentinel").read_text(), "keep")
            self.assertTrue(alias.is_symlink())

    def test_unmounted_volume_is_rejected_before_target_creation(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-target-mount-", dir="/tmp") as temporary:
            mountpoint = Path(temporary) / "mountpoint"
            mountpoint.write_text("#!/bin/sh\nexit 1\n")
            mountpoint.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = temporary + os.pathsep + environment["PATH"]
            result = subprocess.run([str(PATH_POLICY), str(VOLUME_ROOT / "not-created")],
                                    env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2)
            self.assertIn("not mounted", result.stderr)

    def test_mounted_volume_uses_the_same_lease_and_idle_cleanup(self) -> None:
        if not VOLUME_ROOT.is_dir():
            self.skipTest("builder volume is not available on this host")
        with tempfile.TemporaryDirectory(prefix="trust-target-volume-", dir=VOLUME_ROOT) as temporary:
            target = Path(temporary) / "target"
            created = subprocess.run([str(WITH_LEASE), str(target), "touch", str(target / "sentinel")],
                                     capture_output=True, text=True, check=False)
            self.assertEqual(created.returncode, 0, created.stderr)
            self.assertTrue((target / "sentinel").exists())
            removed = subprocess.run([str(REMOVE_IDLE), str(target)],
                                     capture_output=True, text=True, check=False)
            self.assertEqual(removed.returncode, 0, removed.stderr)
            self.assertFalse(target.exists())

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
