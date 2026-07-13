from __future__ import annotations

import hashlib
import io
import json
import tempfile
import unittest
import urllib.request
import zipfile
from pathlib import Path
from typing import Any

from scripts.fetch_windows_ads_candidate import (
    ACCEPTANCE_BUNDLE_MEMBERS,
    ACCEPTANCE_BUNDLE_NAME,
    CandidateFetchError,
    _SafeArtifactRedirect,
    fetch_candidate,
    github_download,
)
from scripts.write_windows_ads_candidate_manifest import build_manifest


REPO = "example/trust-platform"
CANDIDATE = "a" * 40
RUN_HEAD = CANDIDATE
RUN_ID = 123
ARTIFACT = f"windows-ads-msvc-candidate-{CANDIDATE}"


def candidate_archive(
    acceptance_members: set[str] | frozenset[str] | None = None,
    *,
    include_acceptance_bundle: bool = True,
) -> tuple[bytes, dict[str, Any]]:
    with tempfile.TemporaryDirectory(prefix="trust-fetch-fixture-") as temp:
        vsix = Path(temp) / "trust-lsp-0.24.33-win32-x64.vsix"
        vsix.write_bytes(b"synthetic exact VSIX bytes")
        manifest = build_manifest(
            vsix,
            CANDIDATE,
            "0.24.33",
            repository=REPO,
            workflow_run_id=RUN_ID,
            workflow_run_attempt=2,
            workflow_run_head_sha=RUN_HEAD,
            workflow_event="pull_request",
        )
        acceptance_stream = io.BytesIO()
        with zipfile.ZipFile(acceptance_stream, "w") as acceptance:
            members = (
                ACCEPTANCE_BUNDLE_MEMBERS
                if acceptance_members is None
                else acceptance_members
            )
            for name in sorted(members):
                acceptance.writestr(name, f"synthetic {name}".encode())
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, "w") as archive:
            archive.writestr(vsix.name, vsix.read_bytes())
            archive.writestr(
                "windows-ads-msvc-candidate.json",
                json.dumps(manifest, sort_keys=True).encode(),
            )
            if include_acceptance_bundle:
                archive.writestr(ACCEPTANCE_BUNDLE_NAME, acceptance_stream.getvalue())
        return stream.getvalue(), manifest


def responses(archive: bytes) -> dict[str, tuple[int, dict[str, Any]]]:
    return {
        f"/actions/runs/{RUN_ID}": (
            200,
            {
                "id": RUN_ID,
                "run_attempt": 2,
                "event": "pull_request",
                "head_sha": RUN_HEAD,
                "path": ".github/workflows/ci.yml",
                "repository": {"full_name": REPO},
                "pull_requests": [],
            },
        ),
        f"/actions/runs/{RUN_ID}/jobs": (
            200,
            {
                "jobs": [
                    {
                        "name": "Windows Packaged Simulator + Native ADS/TcAdsDll Contract",
                        "status": "completed",
                        "conclusion": "success",
                    }
                ]
            },
        ),
        f"/actions/runs/{RUN_ID}/artifacts": (
            200,
            {
                "artifacts": [
                    {
                        "id": 456,
                        "name": ARTIFACT,
                        "expired": False,
                        "digest": "sha256:" + hashlib.sha256(archive).hexdigest(),
                        "size_in_bytes": len(archive),
                        "archive_download_url": (
                            f"https://api.github.com/repos/{REPO}/actions/artifacts/456/zip"
                        ),
                        "workflow_run": {"id": RUN_ID},
                    }
                ]
            },
        ),
    }


class WindowsAdsCandidateFetchTests(unittest.TestCase):
    def test_ci_bundle_sources_and_generation_match_the_fetch_contract(self) -> None:
        root = Path(__file__).resolve().parents[2]
        source_members = {
            "scripts/accept_windows_packaged_simulator.ps1",
            "scripts/accept_windows_twincat_ads.ps1",
            *(
                f"scripts/windows_twincat_ads_acceptance/{path.name}"
                for path in (root / "scripts/windows_twincat_ads_acceptance").iterdir()
                if path.is_file()
            ),
        }
        self.assertEqual(source_members, ACCEPTANCE_BUNDLE_MEMBERS)

        workflow = (root / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn(
            "from scripts.fetch_windows_ads_candidate import "
            "ACCEPTANCE_BUNDLE_MEMBERS",
            workflow,
        )
        self.assertIn("for name in sorted(ACCEPTANCE_BUNDLE_MEMBERS):", workflow)

        with tempfile.TemporaryDirectory(prefix="trust-acceptance-bundle-") as temp:
            output = Path(temp) / ACCEPTANCE_BUNDLE_NAME
            with zipfile.ZipFile(output, "w") as archive:
                for name in sorted(ACCEPTANCE_BUNDLE_MEMBERS):
                    archive.write(root / name, name)
            with zipfile.ZipFile(output) as archive:
                generated = {
                    info.filename for info in archive.infolist() if not info.is_dir()
                }
        self.assertEqual(generated, ACCEPTANCE_BUNDLE_MEMBERS)

    def test_cross_host_artifact_redirect_does_not_forward_github_token(self) -> None:
        request = urllib.request.Request(
            "https://api.github.com/repos/example/trust/actions/artifacts/1/zip",
            headers={"Authorization": "Bearer private-token"},
        )
        redirected = _SafeArtifactRedirect().redirect_request(
            request,
            None,
            302,
            "Found",
            {},
            "https://artifact-storage.example.test/candidate.zip",
        )
        self.assertIsNotNone(redirected)
        self.assertIsNone(redirected.get_header("Authorization"))
        with self.assertRaisesRegex(urllib.error.HTTPError, "non-HTTPS"):
            _SafeArtifactRedirect().redirect_request(
                request,
                None,
                302,
                "Found",
                {},
                "http://api.github.com/insecure-artifact.zip",
            )

    def test_initial_artifact_url_must_be_exact_https_github_api_endpoint(self) -> None:
        archive, _ = candidate_archive()
        for unsafe in (
            "https://attacker.example/candidate.zip",
            f"http://api.github.com/repos/{REPO}/actions/artifacts/456/zip",
            f"https://api.github.com/repos/other/repo/actions/artifacts/456/zip",
            f"https://api.github.com/repos/{REPO}/actions/artifacts/999/zip",
        ):
            with self.subTest(url=unsafe):
                api = responses(archive)
                api[f"/actions/runs/{RUN_ID}/artifacts"][1]["artifacts"][0][
                    "archive_download_url"
                ] = unsafe
                calls: list[str] = []
                with tempfile.TemporaryDirectory(prefix="trust-fetched-candidate-") as temp:
                    with self.assertRaisesRegex(CandidateFetchError, "download URL"):
                        fetch_candidate(
                            repo=REPO,
                            run_id=RUN_ID,
                            artifact_name=ARTIFACT,
                            expected_candidate_sha=CANDIDATE,
                            output_dir=Path(temp),
                            api_get=lambda path, _query=None: api[path],
                            download=lambda url: calls.append(url) or archive,
                        )
                self.assertEqual(calls, [], "unsafe initial URL reached the downloader")

        with self.assertRaisesRegex(CandidateFetchError, "safe HTTPS GitHub API URL"):
            github_download("private-token", "https://attacker.example/candidate.zip")

    def test_api_run_job_artifact_and_archive_digest_bind_offline_bundle(self) -> None:
        archive, _ = candidate_archive()
        api = responses(archive)
        with tempfile.TemporaryDirectory(prefix="trust-fetched-candidate-") as temp:
            output = Path(temp)
            provenance = fetch_candidate(
                repo=REPO,
                run_id=RUN_ID,
                artifact_name=ARTIFACT,
                expected_candidate_sha=CANDIDATE,
                output_dir=output,
                api_get=lambda path, _query=None: api[path],
                download=lambda _url: archive,
            )
            self.assertEqual(provenance["artifact_id"], 456)
            self.assertTrue(provenance["verification"]["artifact_archive_digest_verified"])
            self.assertTrue((output / provenance["artifact_archive_filename"]).is_file())
            self.assertTrue((output / "windows-ads-msvc-candidate-provenance.json").is_file())
            self.assertEqual(
                provenance["acceptance_member_count"], len(ACCEPTANCE_BUNDLE_MEMBERS)
            )
            self.assertTrue((output / ACCEPTANCE_BUNDLE_NAME).is_file())
            self.assertTrue((output / "scripts" / "accept_windows_twincat_ads.ps1").is_file())

    def test_missing_or_unsafe_acceptance_bundle_fails_closed(self) -> None:
        archive, _ = candidate_archive(include_acceptance_bundle=False)
        api = responses(archive)
        with tempfile.TemporaryDirectory(prefix="trust-missing-bundle-") as temp:
            output = Path(temp)
            with self.assertRaisesRegex(
                CandidateFetchError, "laptop acceptance bundle"
            ):
                fetch_candidate(
                    repo=REPO,
                    run_id=RUN_ID,
                    artifact_name=ARTIFACT,
                    expected_candidate_sha=CANDIDATE,
                    output_dir=output,
                    api_get=lambda path, _query=None: api[path],
                    download=lambda _url: archive,
                )
            self.assertEqual(list(output.iterdir()), [])

        required = set(ACCEPTANCE_BUNDLE_MEMBERS)
        missing = set(required)
        missing.remove("scripts/accept_windows_twincat_ads.ps1")
        cases = (
            (missing, "missing required Windows journey files"),
            (required | {"../escape.ps1"}, "unsafe or unexpected member"),
            (
                required
                | {"scripts/windows_twincat_ads_acceptance/Unexpected.js"},
                "unsafe or unexpected member",
            ),
        )
        for members, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                archive, _ = candidate_archive(members)
                api = responses(archive)
                with tempfile.TemporaryDirectory(
                    prefix="trust-rejected-candidate-"
                ) as temp:
                    output = Path(temp)
                    with self.assertRaisesRegex(CandidateFetchError, expected_error):
                        fetch_candidate(
                            repo=REPO,
                            run_id=RUN_ID,
                            artifact_name=ARTIFACT,
                            expected_candidate_sha=CANDIDATE,
                            output_dir=output,
                            api_get=lambda path, _query=None: api[path],
                            download=lambda _url: archive,
                        )
                    self.assertEqual(list(output.iterdir()), [])

    def test_tampered_archive_or_wrong_workflow_head_fails_closed(self) -> None:
        archive, _ = candidate_archive()
        api = responses(archive)
        with tempfile.TemporaryDirectory(prefix="trust-fetched-candidate-") as temp:
            with self.assertRaisesRegex(CandidateFetchError, "API digest"):
                fetch_candidate(
                    repo=REPO,
                    run_id=RUN_ID,
                    artifact_name=ARTIFACT,
                    expected_candidate_sha=CANDIDATE,
                    output_dir=Path(temp),
                    api_get=lambda path, _query=None: api[path],
                    download=lambda _url: archive + b"tampered",
                )

            wrong_size = responses(archive)
            wrong_size[f"/actions/runs/{RUN_ID}/artifacts"][1]["artifacts"][0][
                "size_in_bytes"
            ] = len(archive) + 1
            with self.assertRaisesRegex(CandidateFetchError, "archive size"):
                fetch_candidate(
                    repo=REPO,
                    run_id=RUN_ID,
                    artifact_name=ARTIFACT,
                    expected_candidate_sha=CANDIDATE,
                    output_dir=Path(temp),
                    api_get=lambda path, _query=None: wrong_size[path],
                    download=lambda _url: archive,
                )

            wrong = responses(archive)
            wrong[f"/actions/runs/{RUN_ID}"][1]["head_sha"] = "c" * 40
            with self.assertRaisesRegex(CandidateFetchError, "workflow run head"):
                fetch_candidate(
                    repo=REPO,
                    run_id=RUN_ID,
                    artifact_name=ARTIFACT,
                    expected_candidate_sha=CANDIDATE,
                    output_dir=Path(temp),
                    api_get=lambda path, _query=None: wrong[path],
                    download=lambda _url: archive,
                )


if __name__ == "__main__":
    unittest.main()
