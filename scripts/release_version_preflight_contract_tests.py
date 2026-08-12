from __future__ import annotations

import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import check_release_tag_preflight as preflight
from scripts import check_release_version_alignment as alignment


VERSION = "2.3.4"
TAG = f"v{VERSION}"
SHA = "a" * 40


def write_alignment_manifests(
    root: Path,
    *,
    cargo: str = VERSION,
    package: str = VERSION,
    lock_top: str = VERSION,
    lock_root: str = VERSION,
) -> tuple[Path, Path, Path]:
    cargo_path = root / "Cargo.toml"
    package_path = root / "package.json"
    lock_path = root / "package-lock.json"
    cargo_path.write_text(
        f"[workspace.package]\nversion = \"{cargo}\"\n", encoding="utf-8"
    )
    package_path.write_text(json.dumps({"version": package}), encoding="utf-8")
    lock_path.write_text(
        json.dumps(
            {
                "version": lock_top,
                "packages": {"": {"version": lock_root}},
            }
        ),
        encoding="utf-8",
    )
    return cargo_path, package_path, lock_path


def run_alignment(
    cargo_path: Path, package_path: Path, lock_path: Path
) -> tuple[int, str, str]:
    stdout = io.StringIO()
    stderr = io.StringIO()
    argv = [
        "check_release_version_alignment.py",
        "--cargo-toml",
        str(cargo_path),
        "--package-json",
        str(package_path),
        "--package-lock-json",
        str(lock_path),
    ]
    with (
        patch.object(sys, "argv", argv),
        contextlib.redirect_stdout(stdout),
        contextlib.redirect_stderr(stderr),
    ):
        result = alignment.main()
    return result, stdout.getvalue(), stderr.getvalue()


class ReleaseVersionAlignmentContractTests(unittest.TestCase):
    def test_workspace_version_reader_requires_workspace_package_version(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "Cargo.toml"
            path.write_text(
                "[workspace.package]\nversion = \" 2.3.4 \"\n", encoding="utf-8"
            )
            self.assertEqual(alignment.workspace_version_from_cargo(path), " 2.3.4 ")
            path.write_text("[package]\nversion = \"2.3.4\"\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "workspace.package"):
                alignment.workspace_version_from_cargo(path)

    def test_package_json_version_requires_nonempty_string_and_trims_edges(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "package.json"
            path.write_text(json.dumps({"version": " 2.3.4 "}), encoding="utf-8")
            self.assertEqual(alignment.package_json_version(path), VERSION)
            for value in [None, 234, "", "   "]:
                path.write_text(json.dumps({"version": value}), encoding="utf-8")
                with self.subTest(value=value):
                    with self.assertRaisesRegex(RuntimeError, "non-empty string"):
                        alignment.package_json_version(path)

    def test_package_lock_requires_both_nonempty_version_fields(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "package-lock.json"
            path.write_text(
                json.dumps(
                    {
                        "version": " 2.3.4 ",
                        "packages": {"": {"version": " 2.3.4 "}},
                    }
                ),
                encoding="utf-8",
            )
            self.assertEqual(alignment.package_lock_versions(path), (VERSION, VERSION))

            invalid = [
                {"packages": {"": {"version": VERSION}}},
                {"version": VERSION, "packages": {}},
                {"version": VERSION, "packages": {"": {"version": 234}}},
            ]
            for payload in invalid:
                path.write_text(json.dumps(payload), encoding="utf-8")
                with self.subTest(payload=payload):
                    with self.assertRaisesRegex(RuntimeError, "version"):
                        alignment.package_lock_versions(path)

    def test_fully_aligned_manifests_pass_with_exact_summary(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            paths = write_alignment_manifests(Path(raw))
            code, stdout, stderr = run_alignment(*paths)
        self.assertEqual(code, 0, stderr)
        self.assertIn("release-version-alignment: OK", stdout)
        self.assertIn(f"workspace={VERSION}", stdout)
        self.assertIn(f"package-lock={VERSION}/{VERSION}", stdout)

    def test_all_manifest_mismatches_are_reported_in_one_invocation(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            paths = write_alignment_manifests(
                Path(raw),
                cargo="1.0.0",
                package="2.0.0",
                lock_top="3.0.0",
                lock_root="4.0.0",
            )
            code, stdout, stderr = run_alignment(*paths)
        self.assertEqual(code, 1)
        self.assertEqual(stdout, "")
        self.assertEqual(stderr.count("Version mismatch"), 4)
        self.assertIn("package.json=2.0.0", stderr)
        self.assertIn("top-level version=3.0.0", stderr)
        self.assertIn("packages[''].version=4.0.0", stderr)
        self.assertIn("Keep workspace and VS Code extension versions synchronized", stderr)

    def test_lock_internal_mismatch_is_reported_even_when_one_side_matches_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            paths = write_alignment_manifests(Path(raw), lock_root="9.9.9")
            code, _stdout, stderr = run_alignment(*paths)
        self.assertEqual(code, 1)
        self.assertIn("packages[''].version=9.9.9", stderr)
        self.assertIn("mismatch inside", stderr)


class TagPreflightMainHarness:
    def __init__(
        self,
        *,
        supplied_tag: str = TAG,
        workspace_version: str = VERSION,
        peeled_sha: str = SHA,
        checked_out_sha: str = SHA,
        tag_type: str = "tag",
        git_error: str | None = None,
        ci_result: tuple[bool, str] = (
            True,
            f"CI success found for {SHA}: https://github.example/actions/runs/9",
        ),
        env: dict[str, str] | None = None,
    ) -> None:
        self.supplied_tag = supplied_tag
        self.workspace = workspace_version
        self.peeled_sha = peeled_sha
        self.checked_out_sha = checked_out_sha
        self.tag_type = tag_type
        self.git_error = git_error
        self.ci_result = ci_result
        self.env = env if env is not None else {"GITHUB_TOKEN": "github-secret"}
        self.git_calls: list[list[str]] = []
        self.ci_calls: list[tuple[str, str, str]] = []

    def run_git(self, args: list[str]) -> str:
        self.git_calls.append(list(args))
        if self.git_error is not None:
            raise RuntimeError(self.git_error)
        if args[:1] == ["rev-parse"] and args[-1] == "HEAD":
            return self.checked_out_sha
        if args[:1] == ["rev-parse"]:
            return self.peeled_sha
        if args[:2] == ["cat-file", "-t"]:
            return self.tag_type
        raise AssertionError(f"unexpected git args: {args}")

    def ci_green(self, repo: str, sha: str, token: str) -> tuple[bool, str]:
        self.ci_calls.append((repo, sha, token))
        return self.ci_result

    def run(self) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        argv = [
            "check_release_tag_preflight.py",
            "--tag",
            self.supplied_tag,
            "--repo",
            "owner/repo",
        ]
        with (
            patch.object(sys, "argv", argv),
            patch.object(preflight, "workspace_version", return_value=self.workspace),
            patch.object(preflight, "run_git", side_effect=self.run_git),
            patch.object(preflight, "ci_green_for_sha", side_effect=self.ci_green),
            patch.dict(os.environ, self.env, clear=True),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            result = preflight.main()
        return result, stdout.getvalue(), stderr.getvalue()


class ReleaseTagPreflightContractTests(unittest.TestCase):
    def test_git_wrapper_returns_trimmed_stdout_and_raises_stderr(self) -> None:
        success = subprocess.CompletedProcess(
            ["git", "rev-parse", "HEAD"], 0, stdout=f" {SHA}\n", stderr=""
        )
        with patch.object(preflight.subprocess, "run", return_value=success):
            self.assertEqual(preflight.run_git(["rev-parse", "HEAD"]), SHA)

        failure = subprocess.CompletedProcess(
            ["git", "rev-parse", "bad"], 128, stdout="", stderr="unknown ref\n"
        )
        with patch.object(preflight.subprocess, "run", return_value=failure):
            with self.assertRaisesRegex(RuntimeError, "unknown ref"):
                preflight.run_git(["rev-parse", "bad"])

    def test_ci_query_api_failure_is_a_stable_negative_result(self) -> None:
        with patch.object(
            preflight,
            "github_api_get",
            return_value=(503, {"message": "unavailable"}),
        ):
            ok, message = preflight.ci_green_for_sha("owner/repo", SHA, "secret")
        self.assertFalse(ok)
        self.assertIn("status 503", message)
        self.assertIn("unavailable", message)

    def test_ci_requires_exact_main_push_success_not_any_success_for_sha(self) -> None:
        runs = [
            {
                "event": "pull_request",
                "head_branch": "feature",
                "head_sha": SHA,
                "status": "completed",
                "conclusion": "success",
                "html_url": "https://github.example/pr-success",
            },
            {
                "event": "push",
                "head_branch": "main",
                "head_sha": SHA,
                "status": "completed",
                "conclusion": "failure",
                "html_url": "https://github.example/main-failure",
            },
        ]
        with patch.object(
            preflight,
            "github_api_get",
            return_value=(200, {"workflow_runs": runs}),
        ):
            ok, message = preflight.ci_green_for_sha("owner/repo", SHA, "secret")
        self.assertFalse(ok)
        self.assertIn("No successful CI", message)

    def test_ci_rejects_tag_or_dispatch_success_as_final_main_evidence(self) -> None:
        for event, branch in [
            ("push", TAG),
            ("workflow_dispatch", "main"),
            ("schedule", "main"),
            ("push", "feature"),
        ]:
            run = {
                "event": event,
                "head_branch": branch,
                "head_sha": SHA,
                "status": "completed",
                "conclusion": "success",
                "html_url": "https://github.example/not-main-push",
            }
            with self.subTest(event=event, branch=branch):
                with patch.object(
                    preflight,
                    "github_api_get",
                    return_value=(200, {"workflow_runs": [run]}),
                ):
                    ok, _message = preflight.ci_green_for_sha(
                        "owner/repo", SHA, "secret"
                    )
                self.assertFalse(ok)

    def test_ci_accepts_completed_successful_main_or_master_push_with_url(self) -> None:
        for branch in ["main", "master"]:
            run = {
                "event": "push",
                "head_branch": branch,
                "head_sha": SHA,
                "status": "completed",
                "conclusion": "success",
                "html_url": f"https://github.example/{branch}",
            }
            with self.subTest(branch=branch):
                with patch.object(
                    preflight,
                    "github_api_get",
                    return_value=(200, {"workflow_runs": [run]}),
                ):
                    ok, message = preflight.ci_green_for_sha(
                        "owner/repo", SHA, "secret"
                    )
                self.assertTrue(ok)
                self.assertIn(f"/{branch}", message)

    def test_ci_success_without_url_is_not_usable_evidence(self) -> None:
        run = {
            "event": "push",
            "head_branch": "main",
            "head_sha": SHA,
            "status": "completed",
            "conclusion": "success",
        }
        with patch.object(
            preflight,
            "github_api_get",
            return_value=(200, {"workflow_runs": [run]}),
        ):
            ok, message = preflight.ci_green_for_sha("owner/repo", SHA, "secret")
        self.assertFalse(ok)
        self.assertIn("URL", message)

    def test_ci_malformed_run_collection_fails_closed(self) -> None:
        with patch.object(
            preflight,
            "github_api_get",
            return_value=(200, {"workflow_runs": {"unexpected": "object"}}),
        ):
            ok, message = preflight.ci_green_for_sha("owner/repo", SHA, "secret")
        self.assertFalse(ok)
        self.assertIn("malformed", message)

    def test_tag_mismatch_fails_before_git_or_ci_queries(self) -> None:
        harness = TagPreflightMainHarness(supplied_tag="v9.9.9")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn(f"expected {TAG}", stderr)
        self.assertEqual(harness.git_calls, [])
        self.assertEqual(harness.ci_calls, [])

    def test_tag_resolution_error_is_visible_and_stops_ci_query(self) -> None:
        harness = TagPreflightMainHarness(git_error="tag cannot be resolved")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("tag cannot be resolved", stderr)
        self.assertEqual(harness.ci_calls, [])

    def test_release_tag_must_be_annotated(self) -> None:
        harness = TagPreflightMainHarness(tag_type="commit")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("annotated", stderr)
        self.assertTrue(
            any(args[:2] == ["cat-file", "-t"] for args in harness.git_calls),
            harness.git_calls,
        )

    def test_tag_must_peel_to_the_checked_out_release_sha(self) -> None:
        harness = TagPreflightMainHarness(
            peeled_sha="b" * 40,
            checked_out_sha=SHA,
        )
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertRegex(stderr, r"(checked.out|does not match|exact)")

    def test_missing_token_fails_before_ci_query(self) -> None:
        harness = TagPreflightMainHarness(env={})
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("GITHUB_TOKEN is required", stderr)
        self.assertEqual(harness.ci_calls, [])

    def test_github_token_precedes_fallback_and_is_not_printed(self) -> None:
        harness = TagPreflightMainHarness(
            env={"GITHUB_TOKEN": "preferred-secret", "GH_TOKEN": "fallback-secret"}
        )
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertEqual(harness.ci_calls, [("owner/repo", SHA, "preferred-secret")])
        self.assertNotIn("preferred-secret", stdout + stderr)
        self.assertNotIn("fallback-secret", stdout + stderr)

    def test_negative_ci_result_fails_with_its_evidence_message(self) -> None:
        harness = TagPreflightMainHarness(
            ci_result=(False, "No successful exact-main CI run")
        )
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("No successful exact-main CI run", stderr)

    def test_exact_tag_checkout_and_ci_chain_passes(self) -> None:
        harness = TagPreflightMainHarness()
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertIn(f"release-tag-preflight: OK ({TAG} -> {SHA})", stdout)
        self.assertIn("CI success found", stdout)


if __name__ == "__main__":
    unittest.main()
