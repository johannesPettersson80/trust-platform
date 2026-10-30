#!/usr/bin/env python3
"""What the Release workflow does for its event: a rehearsal, a release, or nothing.

Specification 24, "Release rehearsal on pull requests" and "Release after main CI":
- a pull request whose head changes the workspace version is rehearsed on its head; one from a fork
  is not, since the rehearsal uses the project's own runners (a maintainer dispatches it instead);
- a manual run rehearses the commit it runs on;
- after `main`'s CI succeeded for a push to this repository's `main`, the commit it checked is
  released when its version has no tag yet, or when the tag is on that commit and its release is
  not complete (a re-run resumes); a version already tagged on another commit was released before,
  so nothing happens. A release is complete once a Release run of its commit passed its last step,
  `Verify the release`, which checks the tag, the GitHub release with its evidence and the
  Marketplace listing of every target. The GitHub release exists before the Marketplace is
  published, so a release whose publishing or verification failed has one, but no such run.

The plan never publishes; it prints and writes `mode`, `sha`, `version` and `tag` to
`$GITHUB_OUTPUT`. A failed git or API request fails it instead of choosing a mode.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tomllib
import urllib.request
from dataclasses import dataclass
from typing import Callable, Sequence

# The Release workflow's job and step that verify a release; `test_release_workflow` holds them to it.
VERIFY_JOB = "Publish"
VERIFY_STEP = "Verify the release"


@dataclass(frozen=True)
class Plan:
    mode: str  # "rehearsal", "release" or "skip"
    sha: str
    version: str
    reason: str

    @property
    def tag(self) -> str:
        return f"v{self.version}" if self.version else ""


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def version_at(sha: str) -> str:
    """The workspace version at `sha`; a commit without one fails the plan."""
    package = tomllib.loads(git("show", f"{sha}:Cargo.toml"))["workspace"]["package"]
    version = package.get("version")
    if not isinstance(version, str) or not version:
        raise ValueError(f"no workspace version at {sha}")
    return version


def parse_tag_listing(listing: str, tag: str) -> tuple[str, str] | None:
    """The tag's kind and the commit it peels to, from `git ls-remote` lines; None without the tag.

    An annotated tag is listed with its peeled commit (`^{}`); a lightweight tag points at the
    commit itself."""
    rows = {}
    for line in listing.splitlines():
        if "\t" in line:
            sha, ref = line.split("\t", 1)
            rows[ref.strip()] = sha.strip()
    if f"refs/tags/{tag}" not in rows:
        return None
    peeled = rows.get(f"refs/tags/{tag}^{{}}")
    return ("tag", peeled) if peeled else ("commit", rows[f"refs/tags/{tag}"])


def tag_commit(tag: str) -> tuple[str, str] | None:
    """The tag's kind and commit on origin; None when origin has no such tag."""
    return parse_tag_listing(git("ls-remote", "--tags", "origin", f"refs/tags/{tag}*"), tag)


def github_get(path: str, token: str) -> dict:
    """One GitHub API object; a failed request or another kind of answer fails the plan."""
    request = urllib.request.Request(
        f"https://api.github.com{path}",
        headers={"Accept": "application/vnd.github+json", "Authorization": f"Bearer {token}"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        answer = json.load(response)
    if not isinstance(answer, dict):
        raise ValueError(f"GitHub answered a non-object for {path}")
    return answer


def verified(jobs: list) -> bool:
    """Whether these jobs of a Release run include the verification job with its last step passed."""
    return any(
        isinstance(job, dict)
        and job.get("name") == VERIFY_JOB
        and any(
            isinstance(step, dict)
            and step.get("name") == VERIFY_STEP
            and step.get("conclusion") == "success"
            for step in job.get("steps") or []
        )
        for job in jobs
    )


def release_complete(
    repository: str, sha: str, token: str, get: Callable[[str, str], dict] = github_get
) -> bool:
    """Whether a Release run of `sha`, in any of its attempts, passed `Verify the release`."""
    runs = get(f"/repos/{repository}/actions/workflows/release.yml/runs?head_sha={sha}&per_page=100", token)
    for run in runs.get("workflow_runs") or []:
        jobs = get(f"/repos/{repository}/actions/runs/{run['id']}/jobs?filter=all&per_page=100", token)
        if verified(jobs.get("jobs") or []):
            return True
    return False


def plan(
    *,
    event: str,
    repository: str,
    official: str,
    pr_base: str = "",
    pr_head: str = "",
    pr_head_repository: str = "",
    run_conclusion: str = "",
    run_event: str = "",
    run_branch: str = "",
    run_sha: str = "",
    run_head_repository: str = "",
    dispatch_sha: str = "",
    version_at: Callable[[str], str] = version_at,
    tag_commit: Callable[[str], tuple[str, str] | None] = tag_commit,
    release_complete: Callable[[str], bool] = lambda tag: False,
) -> Plan:
    if event == "pull_request":
        if pr_head_repository != repository:
            return Plan("skip", pr_head, "", "a fork's pull request is rehearsed by a maintainer's dispatch")
        version = version_at(pr_head)
        if version == version_at(pr_base):
            return Plan("skip", pr_head, version, "the pull request keeps the version")
        return Plan("rehearsal", pr_head, version, "the pull request changes the version")
    if event == "workflow_dispatch":
        return Plan("rehearsal", dispatch_sha, version_at(dispatch_sha), "a manual run rehearses")
    if event != "workflow_run":
        return Plan("skip", "", "", f"a {event} event releases nothing")
    if (
        run_conclusion != "success"
        or run_event != "push"
        or run_branch not in {"main", "master"}
        or run_head_repository != repository
    ):
        return Plan("skip", run_sha, "", "only a successful CI run of a push to this repository's main releases")
    if repository != official:
        return Plan("skip", run_sha, "", "only the official repository releases")
    version = version_at(run_sha)
    tag = f"v{version}"
    found = tag_commit(tag)
    if found is None:
        return Plan("release", run_sha, version, f"{tag} has no tag yet")
    kind, commit = found
    if commit != run_sha:
        return Plan("skip", run_sha, version, f"{tag} was released from {commit}")
    if kind != "tag":
        raise ValueError(f"{tag} on {run_sha} is not an annotated tag")
    if release_complete(tag):
        return Plan("skip", run_sha, version, f"{tag} is released and complete")
    return Plan("release", run_sha, version, f"{tag} is on this commit and its release is not complete")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    for name in (
        "event", "repository", "official", "pr-base", "pr-head", "pr-head-repository",
        "run-conclusion", "run-event", "run-branch", "run-sha", "run-head-repository", "dispatch-sha",
    ):
        parser.add_argument(f"--{name}", default="")
    args = parser.parse_args(argv)
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    result = plan(
        event=args.event,
        repository=args.repository,
        official=args.official,
        pr_base=args.pr_base,
        pr_head=args.pr_head,
        pr_head_repository=args.pr_head_repository,
        run_conclusion=args.run_conclusion,
        run_event=args.run_event,
        run_branch=args.run_branch,
        run_sha=args.run_sha,
        run_head_repository=args.run_head_repository,
        dispatch_sha=args.dispatch_sha,
        release_complete=lambda tag: release_complete(args.repository, args.run_sha, token),
    )
    lines = [f"mode={result.mode}", f"sha={result.sha}", f"version={result.version}", f"tag={result.tag}"]
    print(f"release plan: {result.mode} ({result.reason})")
    for line in lines:
        print(line)
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as handle:
            handle.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
