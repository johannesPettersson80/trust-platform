#!/usr/bin/env python3
"""Enforce tag/release evidence for workspace version bumps on main/master pushes."""

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

NULL_SHA = "0" * 40


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


def current_workspace_version() -> str:
    return workspace_version_from_toml(Path("Cargo.toml").read_text(encoding="utf-8"))


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
            body = response.read().decode("utf-8")
            return response.status, json.loads(body) if body else {}
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {"message": raw}
        return exc.code, payload


def fail(message: str) -> int:
    print(f"::error::{message}", file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Fail CI when a workspace version bump on main/master is missing matching "
            "vX.Y.Z tag and successful release evidence."
        )
    )
    parser.add_argument("--event-name", required=True)
    parser.add_argument("--ref", required=True)
    parser.add_argument("--before", default="")
    parser.add_argument("--after", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument(
        "--run-discovery-timeout-seconds",
        type=int,
        default=180,
        help="How long to wait for the tag-triggered Release run to appear.",
    )
    parser.add_argument(
        "--run-completion-timeout-seconds",
        type=int,
        default=1800,
        help="How long to wait for the Release run to complete.",
    )
    parser.add_argument(
        "--poll-interval-seconds",
        type=int,
        default=15,
        help="Polling interval used while waiting for Release workflow evidence.",
    )
    args = parser.parse_args()

    if args.event_name != "push" or args.ref not in {"refs/heads/main", "refs/heads/master"}:
        print("version-release-guard: skipped (not a push to main/master).")
        return 0

    current_version = current_workspace_version()
    before_version: str | None = None
    if args.before and args.before != NULL_SHA:
        before_version = workspace_version_at_rev(args.before)

    if before_version == current_version:
        print(
            "version-release-guard: workspace version unchanged "
            f"({current_version}); no tag/release evidence required."
        )
        return 0

    expected_tag = f"v{current_version}"
    if before_version:
        print(
            "version-release-guard: detected workspace version bump "
            f"{before_version} -> {current_version}."
        )
    else:
        print(
            "version-release-guard: unable to resolve previous workspace version; "
            f"enforcing release evidence for {current_version}."
        )

    run_git(["fetch", "--tags", "--force", "origin"])

    tag_commit_proc = run_git(["rev-parse", f"refs/tags/{expected_tag}^{{}}"], check=False)
    if tag_commit_proc.returncode != 0:
        return fail(
            f"Workspace version bumped to {current_version}, but tag {expected_tag} does not "
            "exist on origin. Create/push the annotated tag and rerun CI."
        )
    tag_commit = tag_commit_proc.stdout.strip()

    ancestor_check = subprocess.run(
        ["git", "merge-base", "--is-ancestor", tag_commit, args.after],
        check=False,
    )
    if ancestor_check.returncode != 0:
        return fail(
            f"Tag {expected_tag} exists but points to {tag_commit}, which is not reachable from "
            f"{args.after}. Tag must match the released main history for version {current_version}."
        )

    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if not token:
        return fail("GITHUB_TOKEN is required to verify release workflow/release API evidence.")

    poll_interval = max(args.poll_interval_seconds, 1)
    run_discovery_timeout = max(args.run_discovery_timeout_seconds, 0)
    run_completion_timeout = max(args.run_completion_timeout_seconds, 0)

    run_for_tag: dict | None = None
    discovery_deadline = time.monotonic() + run_discovery_timeout
    while True:
        runs_status, runs_payload = github_api_get(
            args.repo,
            "/actions/workflows/release.yml/runs",
            token,
            query={"event": "push", "per_page": "100"},
        )
        if runs_status != 200:
            return fail(
                "Could not query release workflow runs from GitHub API "
                f"(status {runs_status}): {runs_payload.get('message', 'unknown error')}"
            )

        run_for_tag = next(
            (
                run
                for run in runs_payload.get("workflow_runs", [])
                if run.get("head_branch") == expected_tag
            ),
            None,
        )
        if run_for_tag is not None:
            break
        if time.monotonic() >= discovery_deadline:
            return fail(
                f"Tag {expected_tag} exists, but no Release workflow run was found for that tag "
                f"within {run_discovery_timeout}s."
            )
        time.sleep(poll_interval)

    run_id = run_for_tag.get("id")
    run_status = run_for_tag.get("status")
    run_conclusion = run_for_tag.get("conclusion")
    run_url = run_for_tag.get("html_url")
    if run_status != "completed":
        if run_id is None:
            return fail(
                f"Release workflow for {expected_tag} has no run id and cannot be polled. "
                f"Run: {run_url}"
            )
        completion_deadline = time.monotonic() + run_completion_timeout
        while time.monotonic() < completion_deadline:
            time.sleep(poll_interval)
            run_status_code, run_payload = github_api_get(
                args.repo,
                f"/actions/runs/{run_id}",
                token,
            )
            if run_status_code != 200:
                return fail(
                    "Could not query Release run status from GitHub API "
                    f"(status {run_status_code}): {run_payload.get('message', 'unknown error')}"
                )
            run_for_tag = run_payload
            run_status = run_for_tag.get("status")
            run_conclusion = run_for_tag.get("conclusion")
            run_url = run_for_tag.get("html_url")
            if run_status == "completed":
                break
        if run_status != "completed":
            return fail(
                f"Release workflow for {expected_tag} did not complete within "
                f"{run_completion_timeout}s (latest status={run_status}). Run: {run_url}"
            )

    if run_conclusion != "success":
        return fail(
            f"Release workflow for {expected_tag} is not successful "
            f"(status={run_status}, conclusion={run_conclusion}). Run: {run_url}"
        )

    release_status, release_payload = github_api_get(
        args.repo,
        f"/releases/tags/{expected_tag}",
        token,
    )
    if release_status != 200:
        return fail(
            f"Release for tag {expected_tag} not found (status {release_status}): "
            f"{release_payload.get('message', 'unknown error')}"
        )
    if release_payload.get("draft") or release_payload.get("published_at") is None:
        return fail(
            f"Release for tag {expected_tag} exists but is not published yet. "
            f"Release URL: {release_payload.get('html_url', '<missing>')}"
        )

    print(
        "version-release-guard: release evidence verified for "
        f"{expected_tag}. release={release_payload.get('html_url')} "
        f"workflow={run_url}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
