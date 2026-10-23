import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SMOKE_SCRIPT = ROOT / "scripts" / "check_editor_integration_smoke.sh"


class EditorIntegrationSmokeContractTests(unittest.TestCase):
    def test_smoke_fails_when_an_exact_filter_selects_zero_tests(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cargo = Path(temp_dir) / "cargo"
            cargo.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "echo '0 tests, 0 benchmarks'\n",
                encoding="utf-8",
            )
            cargo.chmod(cargo.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env["PATH"] = f"{temp_dir}:{env['PATH']}"

            result = subprocess.run(
                ["bash", str(SMOKE_SCRIPT)],
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
