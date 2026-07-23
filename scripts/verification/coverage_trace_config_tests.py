"""Focused tests for isolated nextest wrapper configuration."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.coverage_trace_config import render_nextest_tool_config


class CoverageTraceConfigTests(unittest.TestCase):
    def test_config_enables_only_dedicated_profile_wrapper(self) -> None:
        rendered = render_nextest_tool_config(Path("/repo/wrapper.py"))

        self.assertIn(
            '[scripts.wrapper."@tool:trust-catalog:catalog-trace"]', rendered
        )
        self.assertIn('command = ["python3", "/repo/wrapper.py"]', rendered)
        self.assertIn("[[profile.catalog-trace.scripts]]", rendered)
        self.assertIn('filter = "all()"', rendered)
        self.assertIn(
            'run-wrapper = "@tool:trust-catalog:catalog-trace"', rendered
        )
        self.assertNotIn("profile.default", rendered)
        self.assertNotIn("experimental", rendered)

    def test_wrapper_path_must_be_absolute_and_quote_free(self) -> None:
        with self.assertRaisesRegex(ValueError, "absolute"):
            render_nextest_tool_config(Path("wrapper.py"))
        with self.assertRaisesRegex(ValueError, "unsupported"):
            render_nextest_tool_config(Path('/repo/wrapper"name.py'))


if __name__ == "__main__":
    unittest.main()
