#!/usr/bin/env python3
"""Verify the public release state after an exact candidate is merged and the Release workflow ran.

The Release workflow tags and publishes the commit whose CI passed on main, and verifies the tag,
the release and the Marketplace itself (specification 24, "Release after main CI"); this check
runs afterwards and also downloads the assets against their published checksums.
"""

from __future__ import annotations

import json
import re
import sys
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any

import release_candidate_guard as guard

# The repository's own release verifier, one implementation of the Marketplace query and targets.
sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts"))
import check_version_release_evidence as release_evidence  # noqa: E402


def verify_downloaded_assets(directory: Path, assets: list[dict[str, Any]]) -> bool:
    names = {str(row.get("name")) for row in assets}
    manifest_name = next(
        (name for name in names if "sha256" in name.lower() and (directory / name).is_file()), None
    )
    if not manifest_name:
        return False
    checked = 0
    resolved_paths: set[Path] = set()
    for line in (directory / manifest_name).read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]{64})\s+\*?(.+)", line.strip())
        if not match:
            continue
        expected, name = match.groups()
        staged_path = PurePosixPath(name)
        if staged_path.is_absolute() or ".." in staged_path.parts:
            return False
        path = directory.joinpath(*staged_path.parts)
        if not path.is_file():
            basename = staged_path.name
            if basename not in names:
                return False
            path = directory / basename
        resolved = path.resolve()
        if (
            not path.is_file()
            or resolved in resolved_paths
            or guard.sha256_bytes(path.read_bytes()) != expected.lower()
        ):
            return False
        resolved_paths.add(resolved)
        checked += 1
    return checked > 0


def release_run_of(provenance: dict[str, Any], repo: Path) -> dict[str, Any]:
    """The workflow run that the release's provenance names, as GitHub reports it; empty when the
    provenance names none or GitHub cannot be asked."""
    run_id = str(provenance.get("workflow_run_id") or "").strip()
    if not run_id.isdigit():
        return {}
    viewed = guard.run(
        ["gh", "run", "view", run_id, "--json", "status,conclusion,workflowName,databaseId"], cwd=repo
    )
    return json.loads(viewed.stdout) if viewed.returncode == 0 else {}


def is_ancestor(repo: Path, older: str, newer: str) -> bool:
    if not older or not newer:
        return False
    return guard.run(["git", "-C", str(repo), "merge-base", "--is-ancestor", older, newer]).returncode == 0


def verify_release(args: Any) -> int:
    repo = guard.repo_root(Path(args.repo))
    fetch = guard.run(["git", "-C", str(repo), "fetch", "origin", "main", "--tags"])
    if fetch.returncode != 0:
        print(fetch.stdout, file=sys.stderr)
        return 2
    main_sha = guard.git(repo, "rev-parse", args.main_ref).strip()
    version = guard.workspace_version_at(repo, main_sha)
    if version is None:
        print("workspace version is missing", file=sys.stderr)
        return 2
    tag = f"v{version}"
    tag_type = guard.git(repo, "cat-file", "-t", tag, check=False).strip()
    tag_sha = guard.git(repo, "rev-parse", f"{tag}^{{commit}}", check=False).strip()
    release_result = guard.run(
        ["gh", "release", "view", tag, "--json", "tagName,isDraft,isPrerelease,assets"], cwd=repo
    )
    release = json.loads(release_result.stdout) if release_result.returncode == 0 else {}
    latest_result = guard.run(["gh", "release", "view", "--json", "tagName"], cwd=repo)
    latest = json.loads(latest_result.stdout) if latest_result.returncode == 0 else {}
    assets_verified = False
    provenance: dict[str, Any] = {}
    if release:
        with tempfile.TemporaryDirectory() as temp_dir:
            download = guard.run(["gh", "release", "download", tag, "--dir", temp_dir], cwd=repo)
            if download.returncode == 0:
                assets_verified = verify_downloaded_assets(Path(temp_dir), release.get("assets") or [])
                record = Path(temp_dir) / "release-provenance.json"
                if record.is_file():
                    provenance = json.loads(record.read_text(encoding="utf-8"))
    workflow = release_run_of(provenance, repo)
    package = json.loads((repo / "editors/vscode/package.json").read_text(encoding="utf-8"))
    extension_id = f"{package['publisher']}.{package['name']}"
    try:
        marketplace = release_evidence.marketplace_versions(extension_id)
    except Exception as error:  # Network/API failure is an explicit incomplete state.
        print(f"Marketplace query failed: {error}", file=sys.stderr)
        marketplace = {}
    state = {
        "tag_on_main": is_ancestor(repo, tag_sha, main_sha),
        "candidate_released": is_ancestor(repo, args.candidate_head, tag_sha),
        "annotated_tag_matches": tag_type == "tag",
        "release_workflow_success": provenance.get("commit") == tag_sha
        and workflow.get("workflowName") == "Release"
        and workflow.get("status") == "completed"
        and workflow.get("conclusion") == "success",
        "github_release_published": bool(release)
        and release.get("isDraft") is False
        and release.get("isPrerelease") is False,
        "github_release_latest": latest.get("tagName") == tag,
        "assets_verified": assets_verified,
        "marketplace_versions": marketplace,
    }
    failures = guard.validate_release_state(state, version, release_evidence.MARKETPLACE_TARGETS)
    print(
        json.dumps(
            {
                "version": version,
                "tag": tag,
                "tag_sha": tag_sha,
                "main_sha": main_sha,
                "release_run": workflow.get("databaseId"),
                **state,
            },
            indent=2,
        )
    )
    if failures:
        print("\n".join(f"INCOMPLETE: {failure}" for failure in failures), file=sys.stderr)
        return 1
    return 0
