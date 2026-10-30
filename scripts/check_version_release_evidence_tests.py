"""The verification of a released version (specification 24, "Release after main CI"): the
annotated tag on the released commit, the final GitHub release named Latest with its evidence
assets, and the Marketplace listing the version for every target within the wait. Every expected
value follows from the state each test gives."""

from __future__ import annotations

import contextlib
import io
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
RELEASED_SHA = "a" * 40
OTHER_SHA = "b" * 40
TAG_OBJECT = "c" * 40
EXPECTED_TAG = f"v{CURRENT_VERSION}"
EXTENSION = "publisher.extension"


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
        "published_at": "2026-10-30T12:00:00Z",
        "html_url": f"https://github.example/releases/{EXPECTED_TAG}",
        "assets": [{"name": name} for name in names],
    }


def annotated_listing(commit: str = RELEASED_SHA) -> str:
    return f"{TAG_OBJECT}\trefs/tags/{EXPECTED_TAG}\n{commit}\trefs/tags/{EXPECTED_TAG}^{{}}\n"


def every_target(version: str = CURRENT_VERSION) -> dict[str, str]:
    return {target: version for target in guard.MARKETPLACE_TARGETS}


class MainHarness:
    """Runs the verifier against a given repository, GitHub and Marketplace, on a clock that each
    pause between Marketplace answers advances by the pause."""

    def __init__(
        self,
        *,
        tag: str = EXPECTED_TAG,
        version_at_sha: str | None = CURRENT_VERSION,
        listing: str | None = None,
        api=None,
        marketplace: list[Any] | None = None,
        wait_seconds: int = 0,
        env: dict[str, str] | None = None,
    ) -> None:
        self.tag = tag
        self.version_at_sha = version_at_sha
        self.listing = annotated_listing() if listing is None else listing
        self.api = api or self.success_api
        # Each Marketplace query takes the next answer: a mapping of target to version, or an
        # exception it raises; the last answer repeats.
        self.marketplace = marketplace if marketplace is not None else [every_target()]
        self.wait_seconds = wait_seconds
        self.env = env if env is not None else {"GITHUB_TOKEN": "github-token"}
        self.git_calls: list[list[str]] = []
        self.api_calls: list[tuple[str, str, str, dict[str, str] | None]] = []
        self.marketplace_calls = 0
        self.clock = 0.0

    def argv(self) -> list[str]:
        return [
            "check_version_release_evidence.py",
            "--repo",
            "owner/repo",
            "--tag",
            self.tag,
            "--sha",
            RELEASED_SHA,
            "--marketplace-wait-seconds",
            str(self.wait_seconds),
            "--poll-interval-seconds",
            "60",
        ]

    def run_git(self, args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        self.git_calls.append(list(args))
        if args[:2] == ["ls-remote", "--tags"]:
            return completed(args, stdout=self.listing)
        raise AssertionError(f"unexpected git call: {args}")

    def success_api(
        self,
        repo: str,
        path: str,
        token: str,
        query: dict[str, str] | None = None,
    ) -> tuple[int, dict]:
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

    def marketplace_versions(self, extension: str) -> dict[str, str]:
        assert extension == EXTENSION, extension
        answer = self.marketplace[min(self.marketplace_calls, len(self.marketplace) - 1)]
        self.marketplace_calls += 1
        if isinstance(answer, Exception):
            raise answer
        return answer

    def sleep(self, seconds: float) -> None:
        self.clock += seconds

    def run(self) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with (
            patch.object(sys, "argv", self.argv()),
            patch.object(guard, "workspace_version_at_rev", return_value=self.version_at_sha),
            patch.object(guard, "run_git", side_effect=self.run_git),
            patch.object(guard, "github_api_get", side_effect=self.api_get),
            patch.object(guard, "extension_id", return_value=EXTENSION),
            patch.object(guard, "marketplace_versions", side_effect=self.marketplace_versions),
            patch.object(guard.time, "monotonic", side_effect=lambda: self.clock),
            patch.object(guard.time, "sleep", side_effect=self.sleep),
            patch.dict(os.environ, self.env, clear=True),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            result = guard.main(self.argv()[1:])
        return result, stdout.getvalue(), stderr.getvalue()


class ReleaseVerificationTests(unittest.TestCase):
    def test_workspace_version_parser_reads_exact_workspace_package_value(self) -> None:
        content = """
[workspace]
members = []

[workspace.package]
version = "0.24.99"
"""
        self.assertEqual(guard.workspace_version_from_toml(content), "0.24.99")

    def test_workspace_version_parser_rejects_missing_or_wrong_shaped_value(self) -> None:
        for content in [
            "[package]\nversion = \"1.0.0\"\n",
            "[workspace]\npackage = \"wrong\"\n",
            "[workspace.package]\nname = \"trust\"\n",
        ]:
            with self.subTest(content=content):
                with self.assertRaisesRegex(RuntimeError, "workspace.package"):
                    guard.workspace_version_from_toml(content)

    def test_a_tag_of_another_version_fails_before_git_or_api_access(self) -> None:
        # Catches a verification of the wrong release: the tag is the version at the commit.
        harness = MainHarness(tag="v9.9.9")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn(f"does not match the workspace version {CURRENT_VERSION}", stderr)
        self.assertEqual((harness.git_calls, harness.api_calls), ([], []))

    def test_an_unreadable_version_at_the_commit_fails(self) -> None:
        code, _stdout, stderr = MainHarness(version_at_sha=None).run()
        self.assertEqual(code, 1)
        self.assertIn("cannot be read", stderr)

    def test_the_tag_must_exist_be_annotated_and_peel_to_the_released_commit(self) -> None:
        for listing, message in [
            ("", "does not exist on origin"),
            (f"{RELEASED_SHA}\trefs/tags/{EXPECTED_TAG}\n", "must be an annotated tag object"),
            (annotated_listing(OTHER_SHA), f"points to {OTHER_SHA}, not the released commit"),
        ]:
            with self.subTest(message=message):
                harness = MainHarness(listing=listing)
                code, _stdout, stderr = harness.run()
                self.assertEqual(code, 1)
                self.assertIn(message, stderr)
                self.assertEqual(harness.api_calls, [])

    def test_missing_github_token_fails_before_api_access(self) -> None:
        harness = MainHarness(env={})
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("GITHUB_TOKEN is required", stderr)
        self.assertEqual(harness.api_calls, [])

    def test_github_token_precedes_gh_token_without_leaking_either(self) -> None:
        harness = MainHarness(env={"GITHUB_TOKEN": "preferred-secret", "GH_TOKEN": "fallback-secret"})
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertTrue(harness.api_calls)
        self.assertTrue(all(call[2] == "preferred-secret" for call in harness.api_calls))
        self.assertNotIn("preferred-secret", stdout + stderr)
        self.assertNotIn("fallback-secret", stdout + stderr)

    def test_success_payload_shape_error_is_a_stable_failure(self) -> None:
        endpoint = f"/releases/tags/{EXPECTED_TAG}"

        def malformed_api(_repo, path, _token, _query=None):
            raise ReleaseEvidenceError(f"GitHub API endpoint {path} returned HTTP 200 with malformed JSON")

        code, stdout, stderr = MainHarness(api=malformed_api).run()
        self.assertEqual(code, 1)
        self.assertIn("::error::", stderr)
        self.assertIn(endpoint, stderr)
        self.assertIn("HTTP 200", stderr)
        self.assertNotIn("Traceback", stdout + stderr)

    def test_missing_or_unpublished_release_is_rejected(self) -> None:
        for status, release in [
            (404, {"message": "not found"}),
            (200, {**published_release(), "published_at": None}),
            (200, {**published_release(), "draft": True}),
        ]:
            def api(_repo, path, _token, _query=None, status=status, release=release):
                if path == f"/releases/tags/{EXPECTED_TAG}":
                    return status, release
                raise AssertionError(path)

            with self.subTest(status=status, release=release):
                code, _stdout, stderr = MainHarness(api=api).run()
                self.assertEqual(code, 1)
                self.assertRegex(stderr, r"(not found|not published)")

    def test_latest_release_api_failure_is_visible(self) -> None:
        def api(_repo, path, _token, _query=None):
            if path == f"/releases/tags/{EXPECTED_TAG}":
                return 200, published_release()
            if path == "/releases/latest":
                return 502, {"message": "bad gateway"}
            raise AssertionError(path)

        code, _stdout, stderr = MainHarness(api=api).run()
        self.assertEqual(code, 1)
        self.assertIn("status 502", stderr)

    def test_publication_contract_failures_are_returned(self) -> None:
        # A prerelease, Latest on another tag, or a missing evidence asset is not a release.
        for release, latest, message in [
            ({**published_release(), "prerelease": True}, EXPECTED_TAG, "not a final publication"),
            (published_release(), "v1.2.2", "GitHub Latest points to 'v1.2.2'"),
            (published_release(assets=["SHA256SUMS"]), EXPECTED_TAG, "missing required assets"),
        ]:
            def api(_repo, path, _token, _query=None, release=release, latest=latest):
                if path == f"/releases/tags/{EXPECTED_TAG}":
                    return 200, release
                if path == "/releases/latest":
                    return 200, {"tag_name": latest}
                raise AssertionError(path)

            with self.subTest(message=message):
                harness = MainHarness(api=api)
                code, _stdout, stderr = harness.run()
                self.assertEqual(code, 1)
                self.assertIn(message, stderr)
                self.assertEqual(harness.marketplace_calls, 0)

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

    def test_the_marketplace_must_list_the_version_for_every_target(self) -> None:
        # Catches a release that one platform's failed upload leaves half published.
        listed = every_target()
        listed["win32-x64"] = "1.2.2"
        code, _stdout, stderr = MainHarness(marketplace=[listed]).run()
        self.assertEqual(code, 1)
        self.assertIn("win32-x64 is '1.2.2'", stderr)
        self.assertNotIn("linux-x64 is", stderr)

    def test_the_wait_polls_until_every_target_is_listed(self) -> None:
        # The Marketplace lists a published version after a delay: two answers without it, then
        # the version for every target, within a wait of three minutes polled each minute.
        partial = {"linux-x64": CURRENT_VERSION}
        harness = MainHarness(marketplace=[partial, {}, every_target()], wait_seconds=180)
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertEqual(harness.marketplace_calls, 3)
        self.assertEqual(harness.clock, 120)
        self.assertIn(f"marketplace={EXTENSION} {CURRENT_VERSION}", stdout)

    def test_the_wait_ends_with_the_targets_still_missing(self) -> None:
        # One answer at the start and one each minute until the wait of two minutes has passed.
        harness = MainHarness(marketplace=[{}], wait_seconds=120)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertEqual(harness.marketplace_calls, 3)
        self.assertIn("after 120s", stderr)
        for target in guard.MARKETPLACE_TARGETS:
            self.assertIn(f"{target} is None", stderr)

    def test_an_unreadable_marketplace_answer_is_never_a_listing(self) -> None:
        # Polling never turns an unknown answer into success; the last error is named.
        error = urllib.error.URLError("connection refused")
        code, _stdout, stderr = MainHarness(marketplace=[error]).run()
        self.assertEqual(code, 1)
        self.assertIn("connection refused", stderr)
        malformed = KeyError("results")
        harness = MainHarness(marketplace=[malformed, every_target()], wait_seconds=60)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertEqual(harness.marketplace_calls, 2)

    def test_complete_exact_release_chain_is_accepted(self) -> None:
        harness = MainHarness()
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertIn(f"release evidence verified for {EXPECTED_TAG} at {RELEASED_SHA}", stdout)
        self.assertIn("release=https://github.example/releases/", stdout)
        self.assertEqual(
            [call[1] for call in harness.api_calls],
            [f"/releases/tags/{EXPECTED_TAG}", "/releases/latest"],
        )


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
