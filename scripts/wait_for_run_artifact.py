#!/usr/bin/env python3
"""Waits until another job of this workflow run has uploaded an artifact (specification 24).

The Release workflow's package build of a platform starts beside the toolchain-bundle jobs and takes its platform's
bundle only when it packages the extension, so the slowest bundle no longer holds back every build (checklist W43,
6 November 2026: the Intel-Mac bundle took 49 minutes and every build waited for it, then the Apple-silicon build took
80 more). `--run RUN --artifact NAME --job JOB` polls the run's artifacts until NAME is uploaded, and stops at once
when JOB, the job that writes it, has ended without it. It exits 0 once the artifact is there, and 1 when the job
ended without it or the wait ran out.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
import urllib.error
from typing import Callable

from release_plan import github_get

# The slowest bundle took 49 minutes on 6 November 2026; a build reaches its wait after its own binaries.
DEFAULT_TIMEOUT_SECONDS = 5400
POLL_SECONDS = 20


def uploaded(artifacts: dict, name: str) -> bool:
    """Whether the run's artifacts hold an unexpired one named `name`."""
    return any(
        artifact.get("name") == name and not artifact.get("expired", False)
        for artifact in artifacts.get("artifacts", [])
    )


def ended_without(jobs: dict, job: str) -> str | None:
    """The conclusion of the job named `job` when it has completed without success, else `None`."""
    for entry in jobs.get("jobs", []):
        if entry.get("name") == job and entry.get("status") == "completed":
            conclusion = entry.get("conclusion")
            return None if conclusion == "success" else str(conclusion)
    return None


def wait(
    repository: str,
    run: str,
    artifact: str,
    job: str,
    token: str,
    timeout: float,
    get: Callable[[str, str], dict | list] = github_get,
    sleep: Callable[[float], None] = time.sleep,
    clock: Callable[[], float] = time.monotonic,
) -> str | None:
    """`None` once `artifact` is uploaded; otherwise why it will not be. A failed request is asked again at the next
    poll, so one unanswered call does not fail a build."""
    artifacts_path = f"/repos/{repository}/actions/runs/{run}/artifacts?name={artifact}&per_page=100"
    jobs_path = f"/repos/{repository}/actions/runs/{run}/jobs?filter=latest&per_page=100"
    deadline = clock() + timeout
    while True:
        try:
            if uploaded(get(artifacts_path, token), artifact):
                return None
            conclusion = ended_without(get(jobs_path, token), job)
            # An artifact uploaded just before its job ended is listed by now.
            if conclusion is not None and not uploaded(get(artifacts_path, token), artifact):
                return f"{job} ended {conclusion} without uploading {artifact}"
            if conclusion is not None:
                return None
        except (urllib.error.URLError, OSError, ValueError) as error:
            print(f"asking GitHub failed, asking again: {error}", file=sys.stderr)
        if clock() >= deadline:
            return f"{artifact} was not uploaded within {int(timeout)} seconds"
        sleep(POLL_SECONDS)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Wait until another job of this run has uploaded an artifact.")
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--run", required=True, help="the workflow run's id")
    parser.add_argument("--artifact", required=True, help="the artifact's name")
    parser.add_argument("--job", required=True, help="the name of the job that uploads it")
    parser.add_argument("--timeout-seconds", type=float, default=DEFAULT_TIMEOUT_SECONDS)
    args = parser.parse_args(argv)
    token = os.environ.get("GITHUB_TOKEN", "")
    if not token or not args.repository:
        print("error: GITHUB_TOKEN and the repository are needed to ask for the run's artifacts", file=sys.stderr)
        return 1
    problem = wait(args.repository, args.run, args.artifact, args.job, token, args.timeout_seconds)
    if problem is not None:
        print(f"error: {problem}", file=sys.stderr)
        return 1
    print(f"{args.artifact} is uploaded")
    return 0


if __name__ == "__main__":
    sys.exit(main())
