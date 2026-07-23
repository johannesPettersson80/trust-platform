"""Focused tests for LLVM merge/export orchestration."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.coverage_trace_tools import (
    export_attempt_coverage,
    llvm_cov_export_command,
)


class CoverageTraceToolsTests(unittest.TestCase):
    def test_each_attempt_is_merged_and_exported_without_shell_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            attempt = root / "attempts" / "one"
            profiles = attempt / "profiles"
            profiles.mkdir(parents=True)
            (profiles / "one.profraw").write_bytes(b"profile")
            executable = root / "test-bin"
            executable.write_bytes(b"binary")
            source = root / "src" / "lib.rs"
            source.parent.mkdir()
            source.write_text("pub fn covered() {}\n")
            (attempt / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_00000000000000000001",
                        "status": "completed",
                        "exit_code": 0,
                        "test_executable": str(executable),
                    }
                )
            )
            tools = root / "llvm"
            tools.mkdir()
            (tools / "llvm-profdata").write_bytes(b"tool")
            (tools / "llvm-cov").write_bytes(b"tool")
            export = {
                "type": "llvm.coverage.json.export",
                "version": "3.1.0",
                "data": [],
            }

            def completed_run(command, **kwargs):
                if command[1] == "export":
                    kwargs["stdout"].write(json.dumps(export))
                return subprocess.CompletedProcess(command, 0, stdout="", stderr="")

            with patch("subprocess.run", side_effect=completed_run) as run:
                exports = export_attempt_coverage(
                    root=root,
                    trace_root=root,
                    llvm_tools_dir=tools,
                    source_paths=("src/lib.rs",),
                )
                self.assertEqual(0, run.call_count)
                self.assertEqual(export, exports["DISC_00000000000000000001"])
                self.assertFalse((attempt / "coverage.json").exists())
                self.assertFalse((attempt / "merged.profdata").exists())
                merge_command = run.call_args_list[0].args[0]
                export_command = run.call_args_list[1].args[0]
                self.assertEqual(str(tools / "llvm-profdata"), merge_command[0])
                self.assertIn(str(profiles / "one.profraw"), merge_command)
                self.assertEqual(str(tools / "llvm-cov"), export_command[0])
                self.assertIn("-skip-functions", export_command)
                self.assertEqual(
                    export_command[-2:],
                    ["--sources", str(source.resolve())],
                )
                self.assertNotIn("capture_output", run.call_args_list[1].kwargs)
                self.assertNotIn("shell", run.call_args_list[0].kwargs)

    def test_failed_or_incomplete_attempt_is_not_exported(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            attempt = root / "attempts" / "one"
            attempt.mkdir(parents=True)
            (attempt / "attempt.json").write_text(
                json.dumps(
                    {
                        "discovery_id": "DISC_00000000000000000001",
                        "status": "completed",
                        "exit_code": 1,
                        "test_executable": "/target/test",
                    }
                )
            )
            with self.assertRaisesRegex(ValueError, "successful completed"):
                export_attempt_coverage(
                    root=root,
                    trace_root=root,
                    llvm_tools_dir=root,
                    source_paths=("src/lib.rs",),
                )

    def test_export_command_rejects_paths_outside_repository(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "escapes repository"):
                llvm_cov_export_command(
                    cov_tool=root / "llvm-cov",
                    merged_profile=root / "merged.profdata",
                    executable=root / "test-bin",
                    root=root,
                    source_paths=("../outside.rs",),
                )


if __name__ == "__main__":
    unittest.main()
