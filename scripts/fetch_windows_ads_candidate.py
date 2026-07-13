#!/usr/bin/env python3
"""Download and API/cryptographically bind an exact Windows ADS CI candidate.

Run this on the connected transfer host. It verifies the GitHub workflow run,
the successful Windows candidate job, the Actions artifact metadata, and the
API-published SHA-256 of the downloaded artifact archive before extracting a
small offline bundle for the TwinCAT laptop.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from collections.abc import Callable
from pathlib import Path, PurePosixPath
from typing import Any


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
HEX_256_RE = re.compile(r"^[0-9a-f]{64}$")
REPOSITORY_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
WORKFLOW_PATH = ".github/workflows/ci.yml"
JOB_NAME = "Windows Packaged Simulator + Native ADS/TcAdsDll Contract"
MANIFEST_NAME = "windows-ads-msvc-candidate.json"
PROVENANCE_NAME = "windows-ads-msvc-candidate-provenance.json"
ACCEPTANCE_BUNDLE_NAME = "windows-ads-laptop-acceptance.zip"
ACCEPTANCE_MODULE_NAMES = frozenset(
    {
        "AcceptanceIo.psm1",
        "AcceptancePlan.psm1",
        "AcceptanceRedaction.js",
        "AcceptanceWait.js",
        "AdsBrowseProof.psm1",
        "CandidateManifestProof.psm1",
        "CandidateProvenanceProof.psm1",
        "InstalledVsixPayloadProof.psm1",
        "PackagedAdsBrowseSelection.js",
        "PackagedAdsCustomPortAcceptance.js",
        "PackagedAdsCustomPortDom.js",
        "PackagedAdsCustomPorts.js",
        "PackagedAdsDapSnapshot.js",
        "PackagedAdsDiscoverySnapshot.js",
        "PackagedAdsGeneratedProof.js",
        "PackagedAdsImportProof.js",
        "PackagedAdsLiveValuesAcceptance.js",
        "PackagedAdsLiveValuesDapProof.js",
        "PackagedAdsLiveValuesRenderProof.js",
        "PackagedAdsRouteProof.js",
        "PackagedAdsSnapshotProof.js",
        "PackagedAdsTomlProof.js",
        "PackagedAdsUiAcceptance.js",
        "PackagedAdsUiCrosscheck.psm1",
        "PackagedBinaryIdentity.js",
        "PackagedDapIoState.js",
        "PackagedDapState.js",
        "PackagedExtensionInstall.psm1",
        "PackagedSimulatorAcceptance.js",
        "PackagedSimulatorCdp.js",
        "PackagedSimulatorLauncher.psm1",
        "PackagedSimulatorVisualProof.js",
        "PackagedTomlAssignment.js",
        "RuntimeControlToken.js",
        "StaticRouteProof.psm1",
    }
)
ACCEPTANCE_BUNDLE_MEMBERS = frozenset(
    {
        "scripts/accept_windows_packaged_simulator.ps1",
        "scripts/accept_windows_twincat_ads.ps1",
        *(
            f"scripts/windows_twincat_ads_acceptance/{name}"
            for name in ACCEPTANCE_MODULE_NAMES
        ),
    }
)

ApiGet = Callable[[str, dict[str, str] | None], tuple[int, dict[str, Any]]]
Download = Callable[[str], bytes]


class CandidateFetchError(ValueError):
    pass


class _SafeArtifactRedirect(urllib.request.HTTPRedirectHandler):
    """Never forward the GitHub bearer token to artifact storage hosts."""

    def redirect_request(  # type: ignore[override]
        self,
        req: urllib.request.Request,
        fp: Any,
        code: int,
        msg: str,
        headers: Any,
        newurl: str,
    ) -> urllib.request.Request | None:
        destination = urllib.parse.urlsplit(newurl)
        if destination.scheme.casefold() != "https":
            raise urllib.error.HTTPError(
                newurl,
                code,
                "refusing non-HTTPS GitHub artifact redirect",
                headers,
                fp,
            )
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        source = urllib.parse.urlsplit(req.full_url)
        if redirected is not None and (
            source.scheme.casefold(),
            source.hostname.casefold() if source.hostname else "",
            source.port,
        ) != (
            destination.scheme.casefold(),
            destination.hostname.casefold() if destination.hostname else "",
            destination.port,
        ):
            redirected.remove_header("Authorization")
        return redirected


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _require(value: bool, message: str) -> None:
    if not value:
        raise CandidateFetchError(message)


def _require_github_artifact_download_url(
    url: str,
    *,
    repo: str | None = None,
    artifact_id: int | None = None,
) -> None:
    """Reject any initial bearer-authenticated URL outside GitHub's API.

    Redirects intentionally leave ``api.github.com`` for GitHub's signed
    artifact storage URL, where the redirect handler strips Authorization.
    The initial URL is different: it comes from API JSON and receives the
    bearer token directly, so it must be validated before a request exists.
    """

    parsed = urllib.parse.urlsplit(url)
    _require(
        parsed.scheme.casefold() == "https"
        and (parsed.hostname or "").casefold() == "api.github.com"
        and parsed.port is None
        and parsed.username is None
        and parsed.password is None
        and not parsed.query
        and not parsed.fragment,
        "artifact download URL is not a safe HTTPS GitHub API URL",
    )
    if repo is not None or artifact_id is not None:
        _require(
            repo is not None and artifact_id is not None,
            "artifact download URL validation requires repository and artifact id",
        )
        expected_path = f"/repos/{repo}/actions/artifacts/{artifact_id}/zip"
        _require(
            parsed.path == expected_path,
            "artifact download URL does not match the selected repository and artifact id",
        )


def github_api_get(
    repo: str,
    token: str,
    path: str,
    query: dict[str, str] | None = None,
) -> tuple[int, dict[str, Any]]:
    suffix = "" if not query else "?" + urllib.parse.urlencode(query)
    request = urllib.request.Request(
        f"https://api.github.com/repos/{repo}{path}{suffix}",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "trust-windows-ads-candidate-fetch",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode("utf-8")
            return response.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as error:
        raw = error.read().decode("utf-8", errors="replace")
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {"message": raw}
        return error.code, payload


def github_download(token: str, url: str) -> bytes:
    _require_github_artifact_download_url(url)
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "trust-windows-ads-candidate-fetch",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    opener = urllib.request.build_opener(_SafeArtifactRedirect())
    with opener.open(request) as response:
        return response.read()


def _archive_files(archive_bytes: bytes) -> dict[str, bytes]:
    try:
        with zipfile.ZipFile(io.BytesIO(archive_bytes)) as archive:
            files: dict[str, bytes] = {}
            for info in archive.infolist():
                if info.is_dir():
                    continue
                normalized = info.filename.replace("\\", "/")
                path = PurePosixPath(normalized)
                _require(
                    not path.is_absolute()
                    and ".." not in path.parts
                    and len(path.parts) == 1,
                    f"candidate artifact contains unsafe member {info.filename!r}",
                )
                _require(normalized not in files, f"duplicate artifact member {normalized}")
                files[normalized] = archive.read(info)
    except (OSError, zipfile.BadZipFile) as error:
        raise CandidateFetchError(f"candidate artifact is not a valid ZIP: {error}") from error
    return files


def _acceptance_bundle_files(bundle_bytes: bytes) -> dict[str, bytes]:
    try:
        with zipfile.ZipFile(io.BytesIO(bundle_bytes)) as archive:
            files: dict[str, bytes] = {}
            for info in archive.infolist():
                if info.is_dir():
                    continue
                normalized = info.filename.replace("\\", "/")
                member = PurePosixPath(normalized)
                _require(
                    not member.is_absolute()
                    and ".." not in member.parts
                    and normalized in ACCEPTANCE_BUNDLE_MEMBERS,
                    f"acceptance bundle contains unsafe or unexpected member {info.filename!r}",
                )
                _require(normalized not in files, f"duplicate acceptance member {normalized}")
                files[normalized] = archive.read(info)
    except (OSError, zipfile.BadZipFile) as error:
        raise CandidateFetchError(
            f"candidate acceptance bundle is not a valid ZIP: {error}"
        ) from error
    _require(
        set(files) == ACCEPTANCE_BUNDLE_MEMBERS,
        "candidate acceptance bundle is missing required Windows journey files",
    )
    return files


def fetch_candidate(
    *,
    repo: str,
    run_id: int,
    artifact_name: str,
    expected_candidate_sha: str,
    output_dir: Path,
    api_get: ApiGet,
    download: Download,
) -> dict[str, Any]:
    candidate = expected_candidate_sha.lower()
    _require(REPOSITORY_RE.fullmatch(repo) is not None, "repository name is invalid")
    _require(COMMIT_RE.fullmatch(candidate) is not None, "expected candidate SHA is invalid")
    _require(run_id > 0, "workflow run id must be positive")
    _require(
        artifact_name == f"windows-ads-msvc-candidate-{candidate}",
        "artifact name does not match the exact candidate SHA",
    )

    status, run = api_get(f"/actions/runs/{run_id}", None)
    _require(status == 200, f"workflow run API lookup failed with status {status}")
    run_head = str(run.get("head_sha", "")).lower()
    run_event = str(run.get("event", ""))
    run_repository = run.get("repository")
    _require(run.get("id") == run_id, "workflow run id differs from the requested run")
    _require(
        isinstance(run_repository, dict)
        and run_repository.get("full_name") == repo,
        "workflow run belongs to a different repository",
    )
    _require(run.get("path") == WORKFLOW_PATH, "candidate came from the wrong workflow")
    _require(COMMIT_RE.fullmatch(run_head) is not None, "workflow run head SHA is invalid")
    _require(run_event in {"push", "pull_request"}, "candidate workflow event is unsupported")
    _require(candidate == run_head, "candidate differs from workflow run head SHA")
    candidate_source = (
        "pull_request_head" if run_event == "pull_request" else "workflow_head"
    )

    jobs_status, jobs_payload = api_get(
        f"/actions/runs/{run_id}/jobs", {"filter": "latest", "per_page": "100"}
    )
    _require(jobs_status == 200, f"workflow jobs API lookup failed with status {jobs_status}")
    matching_jobs = [
        job
        for job in jobs_payload.get("jobs", [])
        if isinstance(job, dict) and job.get("name") == JOB_NAME
    ]
    _require(len(matching_jobs) == 1, "candidate workflow has no unique Windows candidate job")
    _require(
        matching_jobs[0].get("status") == "completed"
        and matching_jobs[0].get("conclusion") == "success",
        "Windows candidate job is not successful",
    )

    artifacts_status, artifacts_payload = api_get(
        f"/actions/runs/{run_id}/artifacts", {"name": artifact_name, "per_page": "100"}
    )
    _require(
        artifacts_status == 200,
        f"workflow artifacts API lookup failed with status {artifacts_status}",
    )
    artifacts = [
        item
        for item in artifacts_payload.get("artifacts", [])
        if isinstance(item, dict) and item.get("name") == artifact_name
    ]
    _require(len(artifacts) == 1, "exactly one matching candidate artifact is required")
    artifact = artifacts[0]
    artifact_id = artifact.get("id")
    workflow_run = artifact.get("workflow_run")
    digest = str(artifact.get("digest", "")).lower()
    api_artifact_size = artifact.get("size_in_bytes")
    _require(artifact.get("expired") is False, "candidate artifact is expired")
    _require(
        isinstance(workflow_run, dict) and workflow_run.get("id") == run_id,
        "candidate artifact belongs to a different workflow run",
    )
    _require(
        digest.startswith("sha256:")
        and HEX_256_RE.fullmatch(digest.removeprefix("sha256:")) is not None,
        "GitHub artifact metadata has no SHA-256 digest",
    )
    download_url = artifact.get("archive_download_url")
    _require(isinstance(download_url, str) and download_url, "artifact has no download URL")
    _require(
        isinstance(artifact_id, int)
        and not isinstance(artifact_id, bool)
        and artifact_id > 0,
        "GitHub artifact id is invalid",
    )
    _require(
        isinstance(api_artifact_size, int)
        and not isinstance(api_artifact_size, bool)
        and api_artifact_size > 0,
        "GitHub artifact metadata has no positive size",
    )
    _require_github_artifact_download_url(
        download_url,
        repo=repo,
        artifact_id=artifact_id,
    )
    archive_bytes = download(download_url)
    archive_sha = _sha256(archive_bytes)
    _require(
        archive_sha == digest.removeprefix("sha256:"),
        "downloaded artifact archive differs from the GitHub API digest",
    )
    _require(
        len(archive_bytes) == api_artifact_size,
        "downloaded artifact archive size differs from the GitHub API metadata",
    )
    files = _archive_files(archive_bytes)
    vsix_names = [name for name in files if name.lower().endswith(".vsix")]
    _require(
        set(files) == {MANIFEST_NAME, ACCEPTANCE_BUNDLE_NAME, *vsix_names}
        and len(vsix_names) == 1,
        "candidate artifact must contain one VSIX, its manifest, and the laptop acceptance bundle",
    )
    acceptance_bundle = files[ACCEPTANCE_BUNDLE_NAME]
    acceptance_files = _acceptance_bundle_files(acceptance_bundle)
    try:
        manifest = json.loads(files[MANIFEST_NAME])
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CandidateFetchError(f"candidate manifest is invalid JSON: {error}") from error
    _require(isinstance(manifest, dict), "candidate manifest must be an object")
    vsix_name = vsix_names[0]
    vsix_bytes = files[vsix_name]
    workflow = manifest.get("workflow_provenance")
    _require(
        manifest.get("schema_version") == 2
        and manifest.get("artifact_kind") == "windows_ads_msvc_candidate"
        and manifest.get("artifact_name") == artifact_name
        and manifest.get("candidate_commit_sha") == candidate,
        "candidate manifest identity does not match the API-selected artifact",
    )
    _require(
        manifest.get("vsix_filename") == vsix_name
        and manifest.get("vsix_sha256") == _sha256(vsix_bytes)
        and manifest.get("vsix_size_bytes") == len(vsix_bytes),
        "candidate manifest does not bind the exact VSIX bytes",
    )
    _require(
        isinstance(workflow, dict)
        and workflow.get("repository") == repo
        and workflow.get("workflow_path") == WORKFLOW_PATH
        and workflow.get("workflow_run_id") == run_id
        and workflow.get("workflow_run_attempt") == run.get("run_attempt")
        and workflow.get("workflow_run_head_sha") == run_head
        and workflow.get("workflow_event") == run_event
        and workflow.get("candidate_source") == candidate_source
        and workflow.get("job_name") == JOB_NAME,
        "candidate manifest workflow provenance differs from the GitHub API",
    )

    provenance = {
        "schema_version": 1,
        "provenance_kind": "github_actions_artifact_api_v1",
        "repository": repo,
        "workflow_path": WORKFLOW_PATH,
        "workflow_run_id": run_id,
        "workflow_run_attempt": run.get("run_attempt"),
        "workflow_run_head_sha": run_head,
        "workflow_event": run_event,
        "candidate_commit_sha": candidate,
        "candidate_source": candidate_source,
        "job_name": JOB_NAME,
        "artifact_id": artifact_id,
        "artifact_name": artifact_name,
        "artifact_archive_filename": f"{artifact_name}.zip",
        "artifact_archive_sha256": archive_sha,
        "artifact_archive_size_bytes": len(archive_bytes),
        "github_api_artifact_size_bytes": api_artifact_size,
        "candidate_manifest_filename": MANIFEST_NAME,
        "candidate_manifest_sha256": _sha256(files[MANIFEST_NAME]),
        "vsix_filename": vsix_name,
        "vsix_sha256": _sha256(vsix_bytes),
        "vsix_size_bytes": len(vsix_bytes),
        "acceptance_bundle_filename": ACCEPTANCE_BUNDLE_NAME,
        "acceptance_bundle_sha256": _sha256(acceptance_bundle),
        "acceptance_bundle_size_bytes": len(acceptance_bundle),
        "acceptance_member_count": len(acceptance_files),
        "verification": {
            "github_api_run_exact": True,
            "github_api_job_success": True,
            "github_api_artifact_exact": True,
            "artifact_archive_digest_verified": True,
            "offline_bundle_integrity_ready": True,
        },
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / provenance["artifact_archive_filename"]).write_bytes(archive_bytes)
    (output_dir / MANIFEST_NAME).write_bytes(files[MANIFEST_NAME])
    (output_dir / vsix_name).write_bytes(vsix_bytes)
    (output_dir / ACCEPTANCE_BUNDLE_NAME).write_bytes(acceptance_bundle)
    for name, data in acceptance_files.items():
        destination = output_dir.joinpath(*PurePosixPath(name).parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    (output_dir / PROVENANCE_NAME).write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return provenance


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--artifact-name", required=True)
    parser.add_argument("--expected-candidate-sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if not token:
        print("Windows ADS candidate fetch failed: GITHUB_TOKEN or GH_TOKEN is required", file=sys.stderr)
        return 1
    api_get = lambda path, query=None: github_api_get(args.repo, token, path, query)
    download = lambda url: github_download(token, url)
    try:
        provenance = fetch_candidate(
            repo=args.repo,
            run_id=args.run_id,
            artifact_name=args.artifact_name,
            expected_candidate_sha=args.expected_candidate_sha,
            output_dir=args.output_dir,
            api_get=api_get,
            download=download,
        )
    except (CandidateFetchError, OSError, urllib.error.URLError) as error:
        print(f"Windows ADS candidate fetch failed: {error}", file=sys.stderr)
        return 1
    print(
        "Windows ADS candidate fetch: OK "
        f"(run={provenance['workflow_run_id']} artifact={provenance['artifact_id']} "
        f"sha256={provenance['vsix_sha256']})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
