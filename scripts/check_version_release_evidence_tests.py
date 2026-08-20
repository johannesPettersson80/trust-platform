from __future__ import annotations

import contextlib
import io
import json
import os
import subprocess
import sys
import unittest
import urllib.error
from typing import Any
from unittest.mock import patch

from scripts import check_version_release_evidence as guard
from scripts.release_evidence_contract import (
    ReleaseEvidenceError,
    validate_release_publication,
)


CURRENT_VERSION = "1.2.3"
CURRENT_SHA = "a" * 40
PREVIOUS_SHA = "b" * 40
TAG_SHA = CURRENT_SHA
EXPECTED_TAG = f"v{CURRENT_VERSION}"


def completed(
    args: list[str],
    *,
    returncode: int = 0,
    stdout: str = "",
    stderr: str = "",
) -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(args, returncode, stdout=stdout, stderr=stderr)


def published_release(*, assets: list[str] | None = None) -> dict[str, Any]:
    names = assets or sorted(guard.REQUIRED_EVIDENCE_ASSETS)
    return {
        "tag_name": EXPECTED_TAG,
        "draft": False,
        "prerelease": False,
        "published_at": "2026-07-30T12:00:00Z",
        "html_url": f"https://github.example/releases/{EXPECTED_TAG}",
        "assets": [{"name": name} for name in names],
    }


def completed_release_run(*, head_sha: str = TAG_SHA) -> dict[str, Any]:
    return {
        "id": 77,
        "event": "push",
        "head_branch": EXPECTED_TAG,
        "head_sha": head_sha,
        "status": "completed",
        "conclusion": "success",
        "html_url": "https://github.example/actions/runs/77",
    }


class MainHarness:
    def __init__(
        self,
        *,
        event_name: str = "push",
        ref: str = "refs/heads/main",
        before: str = PREVIOUS_SHA,
        after: str = CURRENT_SHA,
        before_version: str | None = "1.2.2",
        tag_sha: str | None = TAG_SHA,
        tag_type: str = "tag",
        merge_base_returncode: int = 0,
        api=None,
        env: dict[str, str] | None = None,
        extra_args: list[str] | None = None,
    ) -> None:
        self.event_name = event_name
        self.ref = ref
        self.before = before
        self.after = after
        self.before_version = before_version
        self.tag_sha = tag_sha
        self.tag_type = tag_type
        self.merge_base_returncode = merge_base_returncode
        self.api = api or self.success_api
        self.env = env if env is not None else {"GITHUB_TOKEN": "github-token"}
        self.extra_args = extra_args or [
            "--run-discovery-timeout-seconds",
            "0",
            "--run-completion-timeout-seconds",
            "0",
            "--poll-interval-seconds",
            "0",
        ]
        self.git_calls: list[tuple[list[str], bool]] = []
        self.api_calls: list[tuple[str, str, str, dict[str, str] | None]] = []

    def argv(self) -> list[str]:
        return [
            "check_version_release_evidence.py",
            "--event-name",
            self.event_name,
            "--ref",
            self.ref,
            "--before",
            self.before,
            "--after",
            self.after,
            "--repo",
            "owner/repo",
            *self.extra_args,
        ]

    def run_git(
        self, args: list[str], *, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        self.git_calls.append((list(args), check))
        if args[:1] == ["fetch"]:
            return completed(args)
        if args[:1] == ["rev-parse"]:
            if self.tag_sha is None:
                return completed(args, returncode=128, stderr="unknown revision")
            return completed(args, stdout=f"{self.tag_sha}\n")
        if args[:2] == ["cat-file", "-t"]:
            return completed(args, stdout=f"{self.tag_type}\n")
        raise AssertionError(f"unexpected git call: {args}")

    def subprocess_run(self, args, **_kwargs):
        if list(args[:3]) == ["git", "merge-base", "--is-ancestor"]:
            return completed(list(args), returncode=self.merge_base_returncode)
        raise AssertionError(f"unexpected subprocess call: {args}")

    def success_api(
        self,
        repo: str,
        path: str,
        token: str,
        query: dict[str, str] | None = None,
    ) -> tuple[int, dict]:
        if path == "/actions/workflows/release.yml/runs":
            return 200, {"workflow_runs": [completed_release_run()]}
        if path == f"/releases/tags/{EXPECTED_TAG}":
            return 200, published_release()
        if path == "/releases/latest":
            return 200, {"tag_name": EXPECTED_TAG}
        raise AssertionError(f"unexpected API path: {path}")

    def api_get(
        self,
        repo: str,
        path: str,
        token: str,
        query: dict[str, str] | None = None,
    ) -> tuple[int, dict]:
        self.api_calls.append((repo, path, token, query))
        return self.api(repo, path, token, query)

    def run(self) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with (
            patch.object(sys, "argv", self.argv()),
            patch.object(guard, "current_workspace_version", return_value=CURRENT_VERSION),
            patch.object(
                guard, "workspace_version_at_rev", return_value=self.before_version
            ),
            patch.object(guard, "run_git", side_effect=self.run_git),
            patch.object(guard.subprocess, "run", side_effect=self.subprocess_run),
            patch.object(guard, "github_api_get", side_effect=self.api_get),
            patch.dict(os.environ, self.env, clear=True),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            result = guard.main()
        return result, stdout.getvalue(), stderr.getvalue()


class VersionReleaseGuardContractTests(unittest.TestCase):
    def test_workspace_version_parser_reads_exact_workspace_package_value(self) -> None:
        content = """
[workspace]
members = []

[workspace.package]
version = "0.24.99"
"""
        self.assertEqual(guard.workspace_version_from_toml(content), "0.24.99")

    def test_success_payload_shape_error_is_a_stable_guard_failure(self) -> None:
        endpoint = "/actions/workflows/release.yml/runs"

        def malformed_api(
            _repo: str,
            path: str,
            _token: str,
            _query: dict[str, str] | None = None,
        ) -> tuple[int, dict]:
            raise ReleaseEvidenceError(
                f"GitHub API endpoint {path} returned HTTP 200 with malformed JSON"
            )

        try:
            code, stdout, stderr = MainHarness(api=malformed_api).run()
        except Exception as exc:
            self.fail(f"version release guard leaked its response error: {exc}")
        self.assertEqual(code, 1)
        self.assertIn("::error::", stderr)
        self.assertIn(endpoint, stderr)
        self.assertIn("HTTP 200", stderr)
        self.assertNotIn("Traceback", stdout + stderr)

    def test_workspace_version_parser_rejects_missing_or_wrong_shaped_value(self) -> None:
        for content in [
            "[package]\nversion = \"1.0.0\"\n",
            "[workspace]\npackage = \"wrong\"\n",
            "[workspace.package]\nname = \"trust\"\n",
        ]:
            with self.subTest(content=content):
                with self.assertRaisesRegex(RuntimeError, "workspace.package"):
                    guard.workspace_version_from_toml(content)

    def test_non_main_push_events_are_the_only_unconditional_skip(self) -> None:
        for event, ref in [
            ("pull_request", "refs/heads/main"),
            ("workflow_dispatch", "refs/heads/main"),
            ("push", "refs/heads/feature"),
            ("push", "refs/tags/v1.2.3"),
        ]:
            harness = MainHarness(event_name=event, ref=ref)
            code, stdout, stderr = harness.run()
            self.assertEqual(code, 0, (event, ref, stderr))
            self.assertIn("skipped", stdout)
            self.assertEqual(harness.git_calls, [])
            self.assertEqual(harness.api_calls, [])

    def test_unchanged_workspace_version_skips_release_evidence(self) -> None:
        harness = MainHarness(before_version=CURRENT_VERSION)
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertIn("workspace version unchanged", stdout)
        self.assertEqual(harness.git_calls, [])
        self.assertEqual(harness.api_calls, [])

    def test_null_before_revision_enforces_instead_of_skipping(self) -> None:
        harness = MainHarness(before=guard.NULL_SHA, before_version=None)
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertIn("unable to resolve previous workspace version", stdout)
        self.assertTrue(harness.git_calls)
        self.assertTrue(harness.api_calls)

    def test_tag_discovery_timeout_is_a_visible_failure(self) -> None:
        harness = MainHarness(tag_sha=None)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn(f"tag {EXPECTED_TAG} does not exist", stderr)

    def test_release_tag_must_be_an_annotated_tag_object(self) -> None:
        harness = MainHarness(tag_type="commit")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("annotated", stderr)
        self.assertTrue(
            any(args[:2] == ["cat-file", "-t"] for args, _ in harness.git_calls),
            harness.git_calls,
        )

    def test_peeled_tag_must_equal_the_exact_main_push_sha(self) -> None:
        harness = MainHarness(tag_sha="c" * 40, merge_base_returncode=0)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertRegex(stderr, r"(exact|does not match)")

    def test_non_ancestor_tag_is_rejected(self) -> None:
        harness = MainHarness(tag_sha="c" * 40, merge_base_returncode=1)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("not reachable", stderr)

    def test_missing_github_token_fails_before_api_access(self) -> None:
        harness = MainHarness(env={})
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("GITHUB_TOKEN is required", stderr)
        self.assertEqual(harness.api_calls, [])

    def test_github_token_precedes_gh_token_without_leaking_either(self) -> None:
        harness = MainHarness(
            env={"GITHUB_TOKEN": "preferred-secret", "GH_TOKEN": "fallback-secret"}
        )
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertTrue(harness.api_calls)
        self.assertTrue(
            all(call[2] == "preferred-secret" for call in harness.api_calls)
        )
        self.assertNotIn("preferred-secret", stdout + stderr)
        self.assertNotIn("fallback-secret", stdout + stderr)

    def test_release_run_query_failure_is_visible(self) -> None:
        def api(_repo, path, _token, _query=None):
            self.assertEqual(path, "/actions/workflows/release.yml/runs")
            return 503, {"message": "temporarily unavailable"}

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("status 503", stderr)
        self.assertIn("temporarily unavailable", stderr)

    def test_release_run_discovery_timeout_is_visible(self) -> None:
        def api(_repo, path, _token, _query=None):
            self.assertEqual(path, "/actions/workflows/release.yml/runs")
            return 200, {"workflow_runs": []}

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("no Release workflow run", stderr)

    def test_same_tag_workflow_run_for_another_sha_is_not_accepted(self) -> None:
        def api(_repo, path, _token, _query=None):
            if path == "/actions/workflows/release.yml/runs":
                return 200, {
                    "workflow_runs": [completed_release_run(head_sha="d" * 40)]
                }
            if path == f"/releases/tags/{EXPECTED_TAG}":
                return 200, published_release()
            if path == "/releases/latest":
                return 200, {"tag_name": EXPECTED_TAG}
            raise AssertionError(path)

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertRegex(stderr, r"(head_sha|exact|no Release workflow run)")

    def test_incomplete_run_without_id_cannot_be_polled(self) -> None:
        def api(_repo, path, _token, _query=None):
            self.assertEqual(path, "/actions/workflows/release.yml/runs")
            run = completed_release_run()
            run.update({"id": None, "status": "queued", "conclusion": None})
            return 200, {"workflow_runs": [run]}

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("no run id", stderr)

    def test_run_completion_timeout_is_visible(self) -> None:
        def api(_repo, path, _token, _query=None):
            self.assertEqual(path, "/actions/workflows/release.yml/runs")
            run = completed_release_run()
            run.update({"status": "in_progress", "conclusion": None})
            return 200, {"workflow_runs": [run]}

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("did not complete", stderr)

    def test_completed_non_success_conclusions_are_rejected(self) -> None:
        for conclusion in ["failure", "cancelled", "timed_out", "skipped", None]:
            def api(_repo, path, _token, _query=None, conclusion=conclusion):
                self.assertEqual(path, "/actions/workflows/release.yml/runs")
                run = completed_release_run()
                run["conclusion"] = conclusion
                return 200, {"workflow_runs": [run]}

            with self.subTest(conclusion=conclusion):
                harness = MainHarness(api=api)
                code, _stdout, stderr = harness.run()
                self.assertEqual(code, 1)
                self.assertIn("not successful", stderr)

    def test_missing_or_unpublished_release_is_rejected(self) -> None:
        for status, release in [
            (404, {"message": "not found"}),
            (200, {**published_release(), "published_at": None}),
            (200, {**published_release(), "draft": True}),
        ]:
            def api(_repo, path, _token, _query=None, status=status, release=release):
                if path == "/actions/workflows/release.yml/runs":
                    return 200, {"workflow_runs": [completed_release_run()]}
                if path == f"/releases/tags/{EXPECTED_TAG}":
                    return status, release
                raise AssertionError(path)

            with self.subTest(status=status, release=release):
                harness = MainHarness(api=api)
                code, _stdout, stderr = harness.run()
                self.assertEqual(code, 1)
                self.assertRegex(stderr, r"(not found|not published)")

    def test_latest_release_api_failure_is_visible(self) -> None:
        def api(_repo, path, _token, _query=None):
            if path == "/actions/workflows/release.yml/runs":
                return 200, {"workflow_runs": [completed_release_run()]}
            if path == f"/releases/tags/{EXPECTED_TAG}":
                return 200, published_release()
            if path == "/releases/latest":
                return 502, {"message": "bad gateway"}
            raise AssertionError(path)

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("status 502", stderr)

    def test_publication_contract_failure_is_returned_by_guard(self) -> None:
        def api(_repo, path, _token, _query=None):
            if path == "/actions/workflows/release.yml/runs":
                return 200, {"workflow_runs": [completed_release_run()]}
            if path == f"/releases/tags/{EXPECTED_TAG}":
                return 200, published_release(assets=["SHA256SUMS"])
            if path == "/releases/latest":
                return 200, {"tag_name": EXPECTED_TAG}
            raise AssertionError(path)

        harness = MainHarness(api=api)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("missing required assets", stderr)

    def test_duplicate_release_asset_names_are_rejected(self) -> None:
        release = published_release()
        release["assets"].append({"name": "SHA256SUMS"})
        with self.assertRaisesRegex(ReleaseEvidenceError, "duplicated"):
            validate_release_publication(
                expected_tag=EXPECTED_TAG,
                release=release,
                latest_release={"tag_name": EXPECTED_TAG},
                required_assets=guard.REQUIRED_EVIDENCE_ASSETS,
            )

    def test_complete_exact_release_chain_is_accepted(self) -> None:
        harness = MainHarness()
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertIn(f"release evidence verified for {EXPECTED_TAG}", stdout)
        self.assertIn("release=", stdout)
        self.assertIn("workflow=", stdout)


class GitHubApiBoundaryTests(unittest.TestCase):
    def test_http_error_with_non_json_body_preserves_status_and_message(self) -> None:
        error = urllib.error.HTTPError(
            url="https://api.github.example",
            code=503,
            msg="unavailable",
            hdrs=None,
            fp=io.BytesIO(b"not-json"),
        )
        try:
            with patch.object(guard.urllib.request, "urlopen", side_effect=error):
                status, payload = guard.github_api_get(
                    "owner/repo", "/releases/latest", "secret"
                )
        finally:
            error.close()
        self.assertEqual(status, 503)
        self.assertEqual(payload, {"message": "not-json"})

    def test_empty_success_body_decodes_to_empty_object(self) -> None:
        class Response:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            @staticmethod
            def read() -> bytes:
                return b""

        with patch.object(guard.urllib.request, "urlopen", return_value=Response()):
            status, payload = guard.github_api_get(
                "owner/repo", "/releases/latest", "secret"
            )
        self.assertEqual(status, 200)
        self.assertEqual(payload, {})

    def test_successful_malformed_or_non_object_body_requires_an_object(self) -> None:
        class Response:
            status = 200

            def __init__(self, body: bytes) -> None:
                self.body = body

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self) -> bytes:
                return self.body

        endpoint = "/releases/latest"
        for body, reason in ((b"{", "malformed JSON"), (b"[]", "non-object")):
            with self.subTest(body=body), patch.object(
                guard.urllib.request, "urlopen", return_value=Response(body)
            ):
                try:
                    guard.github_api_get("owner/repo", endpoint, "secret")
                except ReleaseEvidenceError as exc:
                    message = str(exc)
                except Exception as exc:
                    self.fail(f"wrong response exception escaped: {exc}")
                else:
                    self.fail("successful malformed GitHub body was accepted")
                self.assertIn(endpoint, message)
                self.assertIn("HTTP 200", message)
                self.assertIn(reason, message)


if __name__ == "__main__":
    unittest.main()
