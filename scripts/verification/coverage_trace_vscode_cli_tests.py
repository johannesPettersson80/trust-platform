"""Focused tests for VS Code trace selection and attempt validation."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.verification.coverage_trace_vscode_cli import (
    build_vscode_selection,
    validate_vscode_attempts,
    vscode_launcher_command,
)


DISCOVERY_ID = "DISC_" + "A" * 20


class CoverageTraceVscodeCliTests(unittest.TestCase):
    def test_selection_uses_compiled_file_and_literal_scanner_title(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            compiled = root / "editors/vscode/out/test/suite/live.test.js"
            compiled.parent.mkdir(parents=True)
            compiled.write_text("test('works', () => {});\n")

            selection = build_vscode_selection(
                [
                    {
                        "discovery_id": DISCOVERY_ID,
                        "discovery_source_kind": "vscode_test",
                        "path": "editors/vscode/src/test/suite/live.test.ts",
                        "name": "works",
                        "command_hint": "cd editors/vscode && npm test",
                    }
                ],
                root=root,
            )

        self.assertEqual(
            {
                "schema_version": 1,
                "tests": [
                    {
                        "discovery_id": DISCOVERY_ID,
                        "source_path": (
                            "editors/vscode/src/test/suite/live.test.ts"
                        ),
                        "compiled_file": (
                            "editors/vscode/out/test/suite/live.test.js"
                        ),
                        "title": "works",
                    }
                ],
            },
            selection,
        )

    def test_attempts_require_exact_denominator_and_contained_profiles(self) -> None:
        with TemporaryDirectory() as directory:
            coverage_root = Path(directory)
            profile = coverage_root / "coverage.json"
            profile.write_text(json.dumps({"result": []}))
            document = {
                "schema_version": 1,
                "requested_discovery_ids": [DISCOVERY_ID],
                "attempts": [
                    {
                        "discovery_id": DISCOVERY_ID,
                        "attempt_id": f"vscode:{DISCOVERY_ID}",
                        "exit_code": 0,
                        "duration_ms": 4,
                        "raw_coverage_files": [str(profile)],
                    }
                ],
            }

            indexed = validate_vscode_attempts(
                document,
                requested_ids={DISCOVERY_ID},
                coverage_root=coverage_root,
            )

        self.assertEqual([DISCOVERY_ID], list(indexed))

    def test_attempt_without_coverage_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "no raw V8 coverage"):
            validate_vscode_attempts(
                {
                    "schema_version": 1,
                    "requested_discovery_ids": [DISCOVERY_ID],
                    "attempts": [
                        {
                            "discovery_id": DISCOVERY_ID,
                            "attempt_id": f"vscode:{DISCOVERY_ID}",
                            "exit_code": 0,
                            "duration_ms": 1,
                            "raw_coverage_files": [],
                        }
                    ],
                },
                requested_ids={DISCOVERY_ID},
                coverage_root=Path("/tmp"),
            )

    def test_launcher_uses_headless_x_display(self) -> None:
        root = Path("/repo")
        command = vscode_launcher_command(
            root=root,
            selection_path=root / "selection.json",
            attempts_path=root / "attempts.json",
            coverage_root=root / "v8",
            cargo_target_dir=Path("/target"),
        )

        self.assertEqual(
            [
                "xvfb-run",
                "-a",
                "-n",
                "120",
                "-s",
                "-screen 0 1920x1080x24",
                "node",
            ],
            command[:7],
        )
        self.assertNotIn("NODE_OPTIONS", " ".join(command))


if __name__ == "__main__":
    unittest.main()
