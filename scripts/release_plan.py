#!/usr/bin/env python3
"""What the Release workflow does for its event: a rehearsal, a release, a package check, or nothing.

Specification 24, "Release rehearsal on pull requests" and "Release after main CI":
- a pull request whose head changes the workspace version, or changes the release workflow or a script
  it runs (`.github/paths/release.txt`), is rehearsed on its head with every target; one from a fork
  is not, since the rehearsal uses the project's own runners (a maintainer dispatches it instead);
- a pull request that keeps the version but changes what the packages hold (`.github/paths/package.txt`:
  the extension, the crates, the examples, the libraries) builds the linux-x64 package, on which the
  acceptance journeys run (specification 22, "Acceptance journeys on the packaged extension");
- a manual run rehearses the commit it runs on;
- a merge group releases and rehearses nothing: its pull request was rehearsed, and the release
  follows `main`'s CI on the commit the queue merged; the run only reports its conclusion;
- after `main`'s CI succeeded for a push to this repository's `main`, the commit it checked is
  released when its version has no tag yet, or when the tag is on that commit and its release is
  not complete (a re-run resumes); a version already tagged on another commit was released before,
  so nothing happens. A release is complete once a Release run of its commit passed its last step,
  `Verify the release`, which checks the tag, the GitHub release with its evidence and the
  Marketplace listing of every target. The GitHub release exists before the Marketplace is
  published, so a release whose publishing or verification failed has one, but no such run.

The plan never publishes; it prints and writes `mode`, `sha`, `version`, `tag` and the matrices of the
toolchain-bundle and build jobs (`toolchain_targets`, `build_targets`) to `$GITHUB_OUTPUT`. A failed git
or API request fails it instead of choosing a mode.
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
from pathlib import Path
from typing import Callable, Sequence

try:
    from scripts import ci_changes
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    import ci_changes  # type: ignore[no-redef]

# The Release workflow's job and step that verify a release; `test_release_workflow` holds them to it.
VERIFY_JOB = "Publish"
VERIFY_STEP = "Verify the release"


# The platforms of the release packages (specification 24): the runner that builds each one's toolchain bundle, and the
# runner and Rust target that build its packages, cross-compiled where the runner is not of that platform.
TARGETS = (
    {"platform": "linux-x64", "toolchain_os": "ubuntu-latest", "os": "ubuntu-latest", "target": "x86_64-unknown-linux-gnu"},
    {"platform": "linux-arm64", "toolchain_os": "ubuntu-24.04-arm", "os": "ubuntu-latest", "target": "aarch64-unknown-linux-gnu",
     "cross": True},
    {"platform": "darwin-x64", "toolchain_os": "macos-15-intel", "os": "macos-latest", "target": "x86_64-apple-darwin"},
    {"platform": "darwin-arm64", "toolchain_os": "macos-latest", "os": "macos-latest", "target": "aarch64-apple-darwin"},
    {"platform": "win32-x64", "toolchain_os": "windows-latest", "os": "windows-latest", "target": "x86_64-pc-windows-msvc"},
)
ALL_PLATFORMS = tuple(target["platform"] for target in TARGETS)
# The platform of the hosted runners, where the acceptance journeys run on the package.
PACKAGE_PLATFORMS = ("linux-x64",)


@dataclass(frozen=True)
class Plan:
    mode: str  # "rehearsal", "release", "package" or "skip"
    sha: str
    version: str
    reason: str
    platforms: tuple[str, ...] = ()

    @property
    def tag(self) -> str:
        return f"v{self.version}" if self.version else ""

    def matrices(self) -> dict[str, list[dict]]:
        """The include lists of the toolchain-bundle and build jobs. A run that builds nothing names the package
        platforms, so that neither list is ever empty; its jobs are skipped."""
        chosen = [target for target in TARGETS if target["platform"] in (self.platforms or PACKAGE_PLATFORMS)]
        return {
            "toolchain_targets": [{"os": target["toolchain_os"], "platform": target["platform"]} for target in chosen],
            "build_targets": [
                {"os": target["os"], "target": target["target"], "platform": target["platform"],
                 **({"cross": True} if target.get("cross") else {})}
                for target in chosen
            ],
        }


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def changes(base: str, head: str, paths_file: Path) -> bool:
    """Whether the pull request from `base` to `head` changes a path of the list, over its whole diff."""
    patterns = ci_changes.read_patterns(paths_file.read_text(encoding="utf-8"))
    relevant, _ = ci_changes.decide("pull_request", base, head, patterns)
    return relevant


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


def version_commits(sha: str) -> list[str]:
    """The commits on `sha`'s first-parent line, newest first, that changed `Cargo.toml`: those that can
    bring a version."""
    return git("rev-list", "--first-parent", sha, "--", "Cargo.toml").split()


def unreleased_earlier(
    sha: str,
    version: str,
    *,
    version_at: Callable[[str], str],
    tag_commit: Callable[[str], tuple[str, str] | None],
    version_commits: Callable[[str], list[str]],
) -> list[tuple[str, str]]:
    """The versions, with their commits, that `sha`'s line brought after its newest tagged version and
    before `version`, and that have no tag. GitHub keeps one waiting run per concurrency group, so a
    release whose run a later one replaced in the queue leaves such a version."""
    found: list[tuple[str, str]] = []
    seen = {version}
    for commit in version_commits(sha):
        earlier = version_at(commit)
        if earlier in seen:
            continue
        seen.add(earlier)
        if tag_commit(f"v{earlier}") is not None:
            break
        found.append((earlier, commit))
    return found


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
    version_commits: Callable[[str], list[str]] = version_commits,
    release_paths: Path = Path(".github/paths/release.txt"),
    package_paths: Path = Path(".github/paths/package.txt"),
    changes: Callable[[str, str, Path], bool] = changes,
) -> Plan:
    if event == "pull_request":
        if pr_head_repository != repository:
            return Plan("skip", pr_head, "", "a fork's pull request is rehearsed by a maintainer's dispatch")
        version = version_at(pr_head)
        if version != version_at(pr_base):
            return Plan("rehearsal", pr_head, version, "the pull request changes the version", ALL_PLATFORMS)
        if changes(pr_base, pr_head, release_paths):
            return Plan("rehearsal", pr_head, version, "the pull request changes the release workflow", ALL_PLATFORMS)
        if changes(pr_base, pr_head, package_paths):
            return Plan("package", pr_head, version, "the pull request changes what the packages hold", PACKAGE_PLATFORMS)
        return Plan("skip", pr_head, version, "the pull request keeps the version and the packages")
    if event == "workflow_dispatch":
        return Plan("rehearsal", dispatch_sha, version_at(dispatch_sha), "a manual run rehearses", ALL_PLATFORMS)
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
    if found is not None:
        kind, commit = found
        if commit != run_sha:
            return Plan("skip", run_sha, version, f"{tag} was released from {commit}")
        if kind != "tag":
            raise ValueError(f"{tag} on {run_sha} is not an annotated tag")
        if release_complete(tag):
            return Plan("skip", run_sha, version, f"{tag} is released and complete")
    # A version brought before this one and never tagged is released first, by running its commit's
    # Release again; releasing past it would skip it for good.
    skipped = unreleased_earlier(
        run_sha, version, version_at=version_at, tag_commit=tag_commit, version_commits=version_commits
    )
    if skipped:
        listed = ", ".join(f"v{earlier} at {commit[:9]}" for earlier, commit in skipped)
        raise ValueError(f"{listed} has no tag; run the Release of that commit again before releasing {tag}")
    if found is None:
        return Plan("release", run_sha, version, f"{tag} has no tag yet", ALL_PLATFORMS)
    return Plan("release", run_sha, version, f"{tag} is on this commit and its release is not complete", ALL_PLATFORMS)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    for name in (
        "event", "repository", "official", "pr-base", "pr-head", "pr-head-repository",
        "run-conclusion", "run-event", "run-branch", "run-sha", "run-head-repository", "dispatch-sha",
    ):
        parser.add_argument(f"--{name}", default="")
    parser.add_argument("--release-paths", type=Path, default=Path(".github/paths/release.txt"))
    parser.add_argument("--package-paths", type=Path, default=Path(".github/paths/package.txt"))
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
        release_paths=args.release_paths,
        package_paths=args.package_paths,
        release_complete=lambda tag: release_complete(args.repository, args.run_sha, token),
    )
    lines = [f"mode={result.mode}", f"sha={result.sha}", f"version={result.version}", f"tag={result.tag}"]
    lines += [f"{name}={json.dumps(matrix)}" for name, matrix in result.matrices().items()]
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
