#!/usr/bin/env python3
"""Focused tests for the version/release evidence guard CLI contract."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

import check_version_release_evidence as guard


REQUIRED_ARGS = [
    "--event-name",
    "pull_request",
    "--ref",
    "refs/pull/1/merge",
    "--after",
    "a" * 40,
    "--repo",
    "example/trust-platform",
]


class VersionReleaseEvidenceParserTests(unittest.TestCase):
    def test_default_completion_wait_covers_slow_release_builds(self) -> None:
        args = guard.build_parser().parse_args(REQUIRED_ARGS)

        self.assertEqual(args.run_completion_timeout_seconds, 90 * 60)

    def test_completion_wait_can_be_overridden(self) -> None:
        args = guard.build_parser().parse_args(
            [*REQUIRED_ARGS, "--run-completion-timeout-seconds", "7200"]
        )

        self.assertEqual(args.run_completion_timeout_seconds, 7200)

    def test_ci_job_budget_exceeds_guard_wait(self) -> None:
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        job = re.search(
            r"(?ms)^  version-release-guard:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
            workflow,
        )

        self.assertIsNotNone(job)
        timeout = re.search(r"(?m)^    timeout-minutes: (?P<minutes>\d+)$", job["body"])
        self.assertIsNotNone(timeout)
        self.assertGreater(
            int(timeout["minutes"]) * 60,
            guard.DEFAULT_RUN_COMPLETION_TIMEOUT_SECONDS
            + 2 * guard.DEFAULT_RUN_DISCOVERY_TIMEOUT_SECONDS
            + 5 * 60,
        )


if __name__ == "__main__":
    unittest.main()
