#!/usr/bin/env python3
"""Verify the public release state after an exact candidate is merged and tagged."""

from __future__ import annotations

import json
import re
import sys
import tempfile
import urllib.request
from pathlib import Path
from typing import Any

import release_candidate_guard as guard


def marketplace_versions(extension_id: str) -> dict[str, str]:
    payload = {
        "filters": [
            {
                "criteria": [{"filterType": 7, "value": extension_id}],
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
        "https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery",
        data=guard.canonical_json_bytes(payload),
        headers={
            "Accept": "application/json;api-version=7.1-preview.1",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        data = json.load(response)
    versions = data["results"][0]["extensions"][0]["versions"]
    found: dict[str, str] = {}
    for row in versions:
        target = row.get("targetPlatform")
        version = row.get("version")
        if (
            target in guard.MARKETPLACE_TARGETS
            and target not in found
            and isinstance(version, str)
        ):
            found[target] = version
    return found


def verify_downloaded_assets(directory: Path, assets: list[dict[str, Any]]) -> bool:
    names = {str(row.get("name")) for row in assets}
    manifest_name = next(
        (name for name in names if "sha256" in name.lower() and (directory / name).is_file()), None
    )
    if not manifest_name:
        return False
    checked = 0
    for line in (directory / manifest_name).read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]{64})\s+\*?(.+)", line.strip())
        if not match:
            continue
        expected, name = match.groups()
        path = directory / name
        if not path.is_file() or guard.sha256_bytes(path.read_bytes()) != expected.lower():
            return False
        checked += 1
    return checked > 0


def verify_release(args: Any) -> int:
    repo = guard.repo_root(Path(args.repo))
    fetch = guard.run(["git", "-C", str(repo), "fetch", "origin", "main", "--tags"])
    if fetch.returncode != 0:
        print(fetch.stdout, file=sys.stderr)
        return 2
    main_sha = guard.git(repo, "rev-parse", "origin/main").strip()
    version = guard.workspace_version_at(repo, main_sha)
    if version is None:
        print("workspace version is missing", file=sys.stderr)
        return 2
    tag = f"v{version}"
    tag_type = guard.git(repo, "cat-file", "-t", tag, check=False).strip()
    tag_sha = guard.git(repo, "rev-parse", f"{tag}^{{commit}}", check=False).strip()
    workflows = guard.run(
        [
            "gh",
            "run",
            "list",
            "--workflow",
            "release.yml",
            "--branch",
            tag,
            "--limit",
            "20",
            "--json",
            "conclusion,status,headSha,databaseId",
        ],
        cwd=repo,
    )
    workflow_rows = json.loads(workflows.stdout) if workflows.returncode == 0 else []
    workflow_success = any(
        row.get("headSha") == main_sha
        and row.get("status") == "completed"
        and row.get("conclusion") == "success"
        for row in workflow_rows
    )
    release_result = guard.run(
        ["gh", "release", "view", tag, "--json", "tagName,isDraft,isPrerelease,assets"], cwd=repo
    )
    release = json.loads(release_result.stdout) if release_result.returncode == 0 else {}
    latest_result = guard.run(["gh", "release", "view", "--json", "tagName"], cwd=repo)
    latest = json.loads(latest_result.stdout) if latest_result.returncode == 0 else {}
    assets_verified = False
    if release:
        with tempfile.TemporaryDirectory() as temp_dir:
            download = guard.run(["gh", "release", "download", tag, "--dir", temp_dir], cwd=repo)
            if download.returncode == 0:
                assets_verified = verify_downloaded_assets(Path(temp_dir), release.get("assets") or [])
    package = json.loads((repo / "editors/vscode/package.json").read_text(encoding="utf-8"))
    extension_id = f"{package['publisher']}.{package['name']}"
    try:
        marketplace = marketplace_versions(extension_id)
    except Exception as error:  # Network/API failure is an explicit incomplete state.
        print(f"Marketplace query failed: {error}", file=sys.stderr)
        marketplace = {}
    state = {
        "main_sha_matches": tag_sha == main_sha,
        "annotated_tag_matches": tag_type == "tag" and tag == f"v{version}",
        "release_workflow_success": workflow_success,
        "github_release_published": bool(release)
        and release.get("isDraft") is False
        and release.get("isPrerelease") is False,
        "github_release_latest": latest.get("tagName") == tag,
        "assets_verified": assets_verified,
        "marketplace_versions": marketplace,
    }
    failures = guard.validate_release_state(state, version, guard.MARKETPLACE_TARGETS)
    print(json.dumps({"version": version, "tag": tag, "main_sha": main_sha, **state}, indent=2))
    if failures:
        print("\n".join(f"INCOMPLETE: {failure}" for failure in failures), file=sys.stderr)
        return 1
    return 0
