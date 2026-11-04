"""The check that a release never publishes other packages than those its GitHub release already holds
(specification 24, "Release after main CI"). Each published release is what GitHub's API answers, with the SHA-256
digest it records for every uploaded asset; the names are those of 0.30.0's release."""

from __future__ import annotations

import contextlib
import hashlib
import io
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_published_release as published  # noqa: E402

REPOSITORY = "johannesPettersson80/trust-platform"
TAG = "v0.31.0"
PLATFORMS = ("linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64")


def package_names() -> list[str]:
    names = []
    for platform in PLATFORMS:
        archive = "zip" if platform.startswith("win32") else "tar.gz"
        names += [f"vsix-artifacts/trust-lsp-{platform}-0.31.0.vsix", f"runtime-artifacts/trust-runtime-{platform}.{archive}",
                  f"trust-lsp-artifacts/trust-lsp-{platform}.{archive}", f"toolchain-artifacts/trust-toolchain-{platform}.tar.gz"]
    return names


def asset(name: str, content: bytes, state: str = "uploaded") -> dict:
    return {"name": name, "state": state, "digest": f"sha256:{hashlib.sha256(content).hexdigest()}"}


class PublishedReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name)
        self.content = {}
        for name in package_names():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(f"{name}, as this run publishes it".encode())
            self.content[Path(name).name] = path.read_bytes()

    def run_check(self, release: dict | None) -> tuple[int, str]:
        errors = io.StringIO()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(errors):
            status = published.check(REPOSITORY, TAG, "token", self.root, release_of=lambda repo, tag, token: release)
        return status, errors.getvalue()

    def same_release(self) -> dict:
        return {"tag_name": TAG, "assets": [asset(name, content) for name, content in self.content.items()]}

    def test_without_a_release_of_the_tag_the_run_publishes(self) -> None:
        # Catches a check that stops the first publication of every version.
        self.assertEqual(self.run_check(None), (0, ""))

    def test_a_release_that_holds_this_runs_packages_continues(self) -> None:
        # Specification 24: re-running a release continues where it stopped, also from another run with the same files,
        # as the release after main's CI finishing a promotion with its rehearsal's files. The provenance, the
        # conformance status and SHA256SUMS that every run writes anew, and an asset still uploading, are not compared.
        # Catches a check that refuses such a continuation.
        release = self.same_release()
        release["assets"] += [asset("release-provenance.json", b"an earlier run's record"),
                              asset("SHA256SUMS", b"an earlier run's checksums"),
                              asset("conformance-status.json", b"an earlier attempt's status"),
                              asset("trust-lsp-linux-x64-0.31.0.vsix", b"half an upload", state="open")]
        self.assertEqual(self.run_check(release)[0], 0)
        partial = {"tag_name": TAG, "assets": self.same_release()["assets"][:3]}
        self.assertEqual(self.run_check(partial)[0], 0, "an upload that stopped before SHA256SUMS")

    def test_a_published_package_with_other_bytes_stops_the_run(self) -> None:
        # Review finding of 4 November 2026: after a promotion that published and then failed, the release after main's
        # CI built every package again and replaced every GitHub asset, while the Marketplace kept the rehearsal's VS
        # Code packages. Catches a run that publishes over a package that differs from its own.
        release = self.same_release()
        release["assets"][0] = asset(release["assets"][0]["name"], b"the rehearsal's build of it")
        status, errors = self.run_check(release)
        self.assertEqual(status, 1)
        self.assertIn("trust-lsp-linux-x64-0.31.0.vsix is published as sha256:", errors)
        self.assertIn("this run publishes nothing", errors)

    def test_a_published_package_that_this_run_lacks_stops_the_run(self) -> None:
        # Catches a release that would keep a package of another build next to this run's.
        release = self.same_release()
        release["assets"].append(asset("trust-lsp-linux-riscv64-0.31.0.vsix", b"a package of another build"))
        status, errors = self.run_check(release)
        self.assertEqual(status, 1)
        self.assertIn("trust-lsp-linux-riscv64-0.31.0.vsix is published, and this run does not publish it", errors)

    def test_a_package_without_githubs_digest_stops_the_run(self) -> None:
        # Catches a check that takes a package it cannot compare as the same.
        release = self.same_release()
        release["assets"][1]["digest"] = None
        status, errors = self.run_check(release)
        self.assertEqual(status, 1)
        self.assertIn("is published as an asset without a digest", errors)

    def test_githubs_answer_not_found_means_no_release_yet(self) -> None:
        # GitHub answers 404 for a tag without a release, which is every version's first publication. Catches a lookup
        # that fails it, and one that takes another failure for "no release" and publishes over a release it never saw.
        def answer(code: int):
            return mock.patch.object(published.urllib.request, "urlopen", side_effect=urllib.error.HTTPError(
                "https://api.github.com", code, "answer", {}, None))

        with answer(404):
            self.assertIsNone(published.github_release(REPOSITORY, TAG, "token"))
        with answer(403), self.assertRaises(urllib.error.HTTPError):
            published.github_release(REPOSITORY, TAG, "token")

    def test_a_failed_request_stops_the_run(self) -> None:
        # Catches a check that publishes when GitHub could not say whether a release exists.
        failure = urllib.error.HTTPError("https://api.github.com", 502, "Bad Gateway", {}, None)
        errors = io.StringIO()
        with mock.patch.object(published, "github_release", side_effect=failure), \
                contextlib.redirect_stderr(errors), contextlib.redirect_stdout(io.StringIO()):
            status = published.main(["--repository", REPOSITORY, "--tag", TAG, "--root", str(self.root)])
        self.assertEqual(status, 1)
        self.assertIn("cannot compare with the release of v0.31.0", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
