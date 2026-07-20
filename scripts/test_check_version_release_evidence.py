#!/usr/bin/env python3
"""Regression tests for the version/release evidence guard CLI contract."""

from __future__ import annotations

import re
import subprocess
import unittest
from pathlib import Path
from unittest import mock

try:
    from scripts import check_release_tag_preflight as tag_preflight
    from scripts import check_version_release_evidence as guard
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    import check_release_tag_preflight as tag_preflight  # type: ignore[no-redef]
    import check_version_release_evidence as guard  # type: ignore[no-redef]


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

    def test_release_workflow_does_not_wait_on_ci(self) -> None:
        repo = Path(__file__).resolve().parents[1]
        release_workflow = (repo / ".github/workflows/release.yml").read_text(encoding="utf-8")

        self.assertNotIn("ci_run_id", release_workflow)
        self.assertNotIn("gh run download", release_workflow)
        self.assertNotRegex(release_workflow, r"(?m)^  release-conformance:$")
        self.assertIn("name: Runtime VM Validation", release_workflow)
        self.assertIn("name: release-conformance", release_workflow)

    def test_release_preflight_has_no_ci_api_contract(self) -> None:
        source = Path(tag_preflight.__file__).read_text(encoding="utf-8")

        self.assertNotIn("ci_green_for_sha", source)
        self.assertNotIn("GITHUB_TOKEN", source)
        self.assertNotIn("github_api_get", source)

    def test_release_tag_must_be_on_main(self) -> None:
        with mock.patch.object(
            tag_preflight.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 0, "", ""),
        ):
            self.assertTrue(tag_preflight.tag_is_on_main("a" * 40))
        with mock.patch.object(
            tag_preflight.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 1, "", ""),
        ):
            self.assertFalse(tag_preflight.tag_is_on_main("a" * 40))


if __name__ == "__main__":
    unittest.main()
