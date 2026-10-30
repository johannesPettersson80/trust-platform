#!/usr/bin/env python3
"""Verify a released version: its annotated tag, its GitHub release and its Marketplace listing.

Specification 24, "Release after main CI": the Release workflow runs this as the last step of a
release, and the release skill's `verify-release` relies on that run. It requires
- the tag `v<version>` of the workspace version at the released commit to be an annotated tag
  object on origin that peels to that commit;
- the release at the tag to exist, be published, non-draft and non-prerelease, GitHub
  `releases/latest` to name the same tag, and the release's asset names to be nonempty, unique
  and to include the required evidence assets;
- the VS Code Marketplace to list the version for every released target within the wait.
Polling never converts an unknown or incomplete answer into success.

Usage, in a checkout of the released commit:

    python3 scripts/check_version_release_evidence.py --repo OWNER/NAME --tag vX.Y.Z --sha SHA
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

try:
    from scripts.release_evidence_contract import (
        ReleaseEvidenceError,
        decode_github_api_object,
        validate_release_publication,
    )
    from scripts.release_plan import parse_tag_listing
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    from release_evidence_contract import (  # type: ignore[no-redef]
        ReleaseEvidenceError,
        decode_github_api_object,
        validate_release_publication,
    )
    from release_plan import parse_tag_listing  # type: ignore[no-redef]

REQUIRED_EVIDENCE_ASSETS = {
    "SHA256SUMS",
    "release-provenance.json",
    "conformance-status.json",
    "conformance-status.md",
}
# Every platform the Release workflow packages a VS Code extension for.
MARKETPLACE_TARGETS = ("darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64", "win32-x64")
# The Marketplace lists a published version after a delay; an hour leaves room for a slow one.
DEFAULT_MARKETPLACE_WAIT_SECONDS = 60 * 60
MARKETPLACE_QUERY_URL = "https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery"


def run_git(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *args],
        capture_output=True,
        text=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed (exit {result.returncode}): {result.stderr.strip()}"
        )
    return result


def workspace_version_from_toml(content: str) -> str:
    data = tomllib.loads(content)
    try:
        return data["workspace"]["package"]["version"]
    except (KeyError, TypeError) as exc:
        raise RuntimeError("Could not read [workspace.package].version from Cargo.toml") from exc


def workspace_version_at_rev(rev: str) -> str | None:
    shown = run_git(["show", f"{rev}:Cargo.toml"], check=False)
    if shown.returncode != 0:
        return None
    return workspace_version_from_toml(shown.stdout)


def extension_id(package_json: Path) -> str:
    package = json.loads(package_json.read_text(encoding="utf-8"))
    return f"{package['publisher']}.{package['name']}"


def github_api_get(
    repo: str,
    path: str,
    token: str,
    query: dict[str, str] | None = None,
) -> tuple[int, dict]:
    qs = ""
    if query:
        qs = "?" + urllib.parse.urlencode(query)
    url = f"https://api.github.com/repos/{repo}{path}{qs}"
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "trust-version-release-guard",
        },
    )
    try:
        with urllib.request.urlopen(req) as response:
            return response.status, decode_github_api_object(
                response.read(), status=response.status, endpoint=path
            )
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {"message": raw}
        return exc.code, payload


def marketplace_versions(extension: str) -> dict[str, str]:
    """The newest version the Marketplace lists for each target of `extension`."""
    payload = {
        "filters": [
            {
                "criteria": [{"filterType": 7, "value": extension}],
                "pageNumber": 1,
                "pageSize": 1,
                "sortBy": 0,
                "sortOrder": 0,
            }
        ],
        "assetTypes": [],
        "flags": 914,
    }
    request = urllib.request.Request(
        MARKETPLACE_QUERY_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Accept": "application/json;api-version=7.1-preview.1",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        data = json.load(response)
    found: dict[str, str] = {}
    for row in data["results"][0]["extensions"][0]["versions"]:
        target = row.get("targetPlatform")
        version = row.get("version")
        if target in MARKETPLACE_TARGETS and target not in found and isinstance(version, str):
            found[target] = version
    return found


def fail(message: str) -> int:
    print(f"::error::{message}", file=sys.stderr)
    return 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--sha", required=True, help="The released commit.")
    parser.add_argument(
        "--package-json",
        type=Path,
        default=Path("editors/vscode/package.json"),
        help="The extension's manifest, which names its Marketplace identity.",
    )
    parser.add_argument(
        "--marketplace-wait-seconds",
        type=int,
        default=DEFAULT_MARKETPLACE_WAIT_SECONDS,
        help="How long to wait for the Marketplace to list the version for every target.",
    )
    parser.add_argument("--poll-interval-seconds", type=int, default=60)
    return parser


def wait_for_marketplace(
    extension: str, version: str, wait_seconds: int, interval_seconds: int
) -> list[str]:
    """The targets the Marketplace does not list `version` for once the wait ends; empty once all do.

    An unreadable answer counts as no listing and is retried until the wait ends."""
    deadline = time.monotonic() + max(wait_seconds, 0)
    while True:
        try:
            versions = marketplace_versions(extension)
            answer = ""
        except (OSError, ValueError, KeyError, IndexError, TypeError) as error:
            versions = {}
            answer = f" (last answer: {error})"
        missing = [
            f"{target} is {versions.get(target)!r}"
            for target in MARKETPLACE_TARGETS
            if versions.get(target) != version
        ]
        if not missing or time.monotonic() >= deadline:
            return [f"{entry}{answer}" for entry in missing]
        time.sleep(max(interval_seconds, 1))


def _main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    version = workspace_version_at_rev(args.sha)
    if version is None:
        return fail(f"Cargo.toml at the released commit {args.sha} cannot be read.")
    if args.tag != f"v{version}":
        return fail(f"Release tag {args.tag} does not match the workspace version {version} at {args.sha}.")

    listing = run_git(["ls-remote", "--tags", "origin", f"refs/tags/{args.tag}*"]).stdout
    found = parse_tag_listing(listing, args.tag)
    if found is None:
        return fail(f"Release tag {args.tag} does not exist on origin.")
    kind, commit = found
    if kind != "tag":
        return fail(f"Release tag {args.tag} must be an annotated tag object.")
    if commit != args.sha:
        return fail(f"Release tag {args.tag} points to {commit}, not the released commit {args.sha}.")

    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if not token:
        return fail("GITHUB_TOKEN is required to verify the release through the GitHub API.")

    release_status, release_payload = github_api_get(args.repo, f"/releases/tags/{args.tag}", token)
    if release_status != 200:
        return fail(
            f"Release for tag {args.tag} not found (status {release_status}): "
            f"{release_payload.get('message', 'unknown error')}"
        )
    if release_payload.get("draft") or release_payload.get("published_at") is None:
        return fail(
            f"Release for tag {args.tag} exists but is not published yet. "
            f"Release URL: {release_payload.get('html_url', '<missing>')}"
        )
    latest_status, latest_payload = github_api_get(args.repo, "/releases/latest", token)
    if latest_status != 200:
        return fail(
            "Could not query GitHub Latest release "
            f"(status {latest_status}): {latest_payload.get('message', 'unknown error')}"
        )
    try:
        validate_release_publication(
            expected_tag=args.tag,
            release=release_payload,
            latest_release=latest_payload,
            required_assets=REQUIRED_EVIDENCE_ASSETS,
        )
    except ReleaseEvidenceError as exc:
        return fail(str(exc))

    extension = extension_id(args.package_json)
    missing = wait_for_marketplace(
        extension, version, args.marketplace_wait_seconds, args.poll_interval_seconds
    )
    if missing:
        return fail(
            f"The Marketplace does not list {extension} {version} for every target after "
            f"{args.marketplace_wait_seconds}s: " + "; ".join(missing)
        )

    print(
        f"release evidence verified for {args.tag} at {args.sha}: "
        f"release={release_payload.get('html_url')} marketplace={extension} {version}"
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    try:
        return _main(argv)
    except (ReleaseEvidenceError, RuntimeError) as exc:
        return fail(str(exc))


if __name__ == "__main__":
    sys.exit(main())
