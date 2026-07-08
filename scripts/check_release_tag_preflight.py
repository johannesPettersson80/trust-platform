#!/usr/bin/env python3
"""Validate tag-triggered release preconditions."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


def fail(message: str) -> int:
    print(f"::error::{message}", file=sys.stderr)
    return 1


def workspace_version() -> str:
    data = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
    return data["workspace"]["package"]["version"]


def run_git(args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def github_api_get(
    repo: str,
    path: str,
    token: str,
    query: dict[str, str] | None = None,
) -> tuple[int, dict[str, Any]]:
    qs = ""
    if query:
        qs = "?" + urllib.parse.urlencode(query)
    req = urllib.request.Request(
        f"https://api.github.com/repos/{repo}{path}{qs}",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "trust-release-tag-preflight",
        },
    )
    try:
        with urllib.request.urlopen(req) as response:
            body = response.read().decode("utf-8")
            return response.status, json.loads(body) if body else {}
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {"message": raw}
        return exc.code, payload


def ci_green_for_sha(repo: str, sha: str, token: str) -> tuple[bool, str]:
    status, payload = github_api_get(
        repo,
        "/actions/workflows/ci.yml/runs",
        token,
        query={"head_sha": sha, "per_page": "100"},
    )
    if status != 200:
        return False, f"GitHub API status {status}: {payload.get('message', 'unknown error')}"

    runs = payload.get("workflow_runs", [])
    successful = [
        run
        for run in runs
        if run.get("status") == "completed" and run.get("conclusion") == "success"
    ]
    if successful:
        html_url = successful[0].get("html_url", "<missing url>")
        return True, f"CI success found for {sha}: {html_url}"
    return False, f"No successful CI workflow run found for tagged SHA {sha}."


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail a tag-triggered release when tag/version/CI evidence is missing."
    )
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repo", required=True)
    args = parser.parse_args()

    version = workspace_version()
    expected_tag = f"v{version}"
    if args.tag != expected_tag:
        return fail(
            f"Release tag {args.tag} does not match workspace version {version}; "
            f"expected {expected_tag}."
        )

    try:
        tagged_sha = run_git(["rev-parse", f"{args.tag}^{{}}"])
    except RuntimeError as error:
        return fail(str(error))

    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if not token:
        return fail("GITHUB_TOKEN is required to verify CI evidence for the release tag.")

    ok, message = ci_green_for_sha(args.repo, tagged_sha, token)
    if not ok:
        return fail(message)

    print(f"release-tag-preflight: OK ({expected_tag} -> {tagged_sha})")
    print(message)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
