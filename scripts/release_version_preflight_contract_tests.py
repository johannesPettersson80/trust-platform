from __future__ import annotations

import contextlib
import io
import json
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
        on_main: bool = True,
    ) -> None:
        self.supplied_tag = supplied_tag
        self.workspace = workspace_version
        self.peeled_sha = peeled_sha
        self.checked_out_sha = checked_out_sha
        self.tag_type = tag_type
        self.git_error = git_error
        self.on_main = on_main
        self.git_calls: list[list[str]] = []
        self.main_ancestry_calls: list[str] = []

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

    def tag_is_on_main(self, sha: str) -> bool:
        self.main_ancestry_calls.append(sha)
        return self.on_main

    def run(self) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        argv = ["check_release_tag_preflight.py", "--tag", self.supplied_tag]
        with (
            patch.object(sys, "argv", argv),
            patch.object(preflight, "workspace_version", return_value=self.workspace),
            patch.object(preflight, "run_git", side_effect=self.run_git),
            patch.object(preflight, "tag_is_on_main", side_effect=self.tag_is_on_main),
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

    def test_tag_preflight_has_no_ci_api_or_token_dependency(self) -> None:
        source = Path(preflight.__file__).read_text(encoding="utf-8")
        self.assertNotIn("ci_green_for_sha", source)
        self.assertNotIn("github_api_get", source)
        self.assertNotIn("GITHUB_TOKEN", source)

    def test_tag_mismatch_fails_before_git_or_main_ancestry_queries(self) -> None:
        harness = TagPreflightMainHarness(supplied_tag="v9.9.9")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn(f"expected {TAG}", stderr)
        self.assertEqual(harness.git_calls, [])
        self.assertEqual(harness.main_ancestry_calls, [])

    def test_tag_resolution_error_is_visible_and_stops_main_ancestry_query(self) -> None:
        harness = TagPreflightMainHarness(git_error="tag cannot be resolved")
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("tag cannot be resolved", stderr)
        self.assertEqual(harness.main_ancestry_calls, [])

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
        harness = TagPreflightMainHarness(peeled_sha="b" * 40)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("does not match", stderr)
        self.assertEqual(harness.main_ancestry_calls, [])

    def test_release_tag_must_be_reachable_from_main(self) -> None:
        harness = TagPreflightMainHarness(on_main=False)
        code, _stdout, stderr = harness.run()
        self.assertEqual(code, 1)
        self.assertIn("not reachable from origin/main", stderr)
        self.assertEqual(harness.main_ancestry_calls, [SHA])

    def test_exact_annotated_tag_checkout_on_main_passes(self) -> None:
        harness = TagPreflightMainHarness()
        code, stdout, stderr = harness.run()
        self.assertEqual(code, 0, stderr)
        self.assertIn(f"release-tag-preflight: OK ({TAG} -> {SHA})", stdout)
        self.assertEqual(harness.main_ancestry_calls, [SHA])


if __name__ == "__main__":
    unittest.main()
