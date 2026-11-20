"""The motion benchmark breakdown's summary (`scripts/runtime_motion_benchmark_breakdown.sh`) reads the reports from
the folder the benchmarks wrote them to, `OUT_DIR`, and summarises what a report of the native runtime holds
(specification 22, "Documentation that follows the product"; `examples/plcopen_motion_single_axis_benchmarks`).

Wrong behaviour caught: the summary reading a fixed `target/gate-artifacts/...` folder while the benchmarks wrote
their reports to another `OUT_DIR`, and a summary section for the bytecode engine's `vm_profile`, which no report has
carried since that engine was deleted on 24 October 2026."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "runtime_motion_benchmark_breakdown.sh"
PROFILES = (
    "runtime_floor",
    "constants_only",
    "status_only",
    "command_idle",
    "move_absolute_only",
    "full_demo_constants_once",
    "full_demo",
)


def summary_program() -> str:
    """The Python the script runs after the benchmarks, from its `python3 - <<'PY'` here-document."""
    match = re.search(r"python3 - <<'PY'\n(.*?)\nPY\n", SCRIPT.read_text(encoding="utf-8"), re.S)
    assert match, "the script's summary program"
    return match.group(1)


def report(p50_us: float, completed: int, demo: bool) -> dict:
    """A `bench project --output json` report with the fields the summary reads."""
    prefix = "g_motion_demo" if demo else "g_motion_bench"
    return {
        "report": {
            "samples": 128,
            "warmup_cycles": 32,
            "cycle_latency": {
                "p50_us": p50_us,
                "p95_us": 2 * p50_us,
                "p99_us": 3 * p50_us,
                "max_us": 4 * p50_us,
            },
            "budget_overruns": 0,
            "watched_globals": {
                f"{prefix}_completed_sequences": completed,
                f"{prefix}_last_error": 0,
            },
        }
    }


class MotionBenchmarkSummaryTests(unittest.TestCase):
    def test_the_summary_reads_out_dir_and_has_no_vm_section(self) -> None:
        with tempfile.TemporaryDirectory() as reports, tempfile.TemporaryDirectory() as elsewhere:
            out_dir = Path(reports)
            p50 = {"runtime_floor": 2.0, "status_only": 3.0, "full_demo": 5.0}
            for name in PROFILES:
                document = report(p50.get(name, 2.0), 22 if name == "full_demo" else 160, name == "full_demo")
                (out_dir / f"{name}.json").write_text(json.dumps(document), encoding="utf-8")
            # Run from a folder without target/, so a summary that reads anything but OUT_DIR finds nothing.
            result = subprocess.run(
                [sys.executable, "-c", summary_program()],
                cwd=elsewhere,
                env={**os.environ, "BUILD_MODE_ENV": "test", "OUT_DIR_ENV": str(out_dir)},
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            summary = (out_dir / "summary.md").read_text(encoding="utf-8")
            rows = json.loads((out_dir / "summary.json").read_text(encoding="utf-8"))["rows"]

        # status_only: p50 3.0 against the floor's 2.0 is 1.5x; p95, p99 and max are 2, 3 and 4 times p50.
        self.assertIn("| status_only | 3.000 | 6.000 | 9.000 | 12.000 | 0 | 160 | 0 | 1.5x |", summary)
        # full_demo reads the demo's own globals: 22 completed sequences; p50 5.0 is 2.5x the floor.
        self.assertIn("| full_demo | 5.000 | 10.000 | 15.000 | 20.000 | 0 | 22 | 0 | 2.5x |", summary)
        self.assertIn("- build mode: test", summary)
        self.assertEqual([row["name"] for row in rows], list(PROFILES))
        self.assertNotIn("VM Profile", summary)


if __name__ == "__main__":
    unittest.main()
