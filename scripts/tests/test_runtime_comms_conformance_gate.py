import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CONFORMANCE_SCRIPT = ROOT / "scripts" / "runtime_comms_conformance_gate.sh"


class RuntimeCommsConformanceGateContractTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
