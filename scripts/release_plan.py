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
  published, so a release whose publishing or verification failed has one, but no such run;
- on a push to this repository's `main`, the pushed commit's release is promoted when the same
  conditions would release it and a passed rehearsal kept its files for the commit's tree (owner,
  2 November 2026: build once): GitHub lists the artifact `release-bundle-v<version>-<tree>` of a
  successful Release run that this repository's pull request or a manual run started, GitHub's
  records give the tree to that run's head commit and to the commit its workflow ran from, and CI
  passed on that head. The release then publishes that rehearsal's files without waiting for main's
  CI; without such a rehearsal the push releases nothing, and the release follows main's CI. The
  release after main's CI promotes the same rehearsal when there is one, so that both publish the
  same bytes, and builds every package otherwise.

The plan never publishes; it prints and writes `mode`, `sha`, `version`, `tag`, the matrices of the
toolchain-bundle and build jobs (`toolchain_targets`, `build_targets`), and for a promoted release the
rehearsal run and the id of its bundle (`rehearsal_run`, `rehearsal_artifact`) to `$GITHUB_OUTPUT`.
A failed git or API request fails it instead of choosing a mode. `--check-unreleased SHA` instead
fails when a Release run of SHA already verified its release: the Publish job asks it once it holds
the release concurrency group.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.request
from dataclasses import dataclass, replace
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Callable, Sequence

try:
    from scripts import ci_changes
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    import ci_changes  # type: ignore[no-redef]

# The Release workflow's job and step that verify a release; `test_release_workflow` holds them to it.
VERIFY_JOB = "Publish"
VERIFY_STEP = "Verify the release"
# The name of a Release run that main's CI of a commit started; GitHub gives such a run main's newest commit as its
# head, so the name tells which commit it released. `test_release_workflow` holds the run-name to it.
AFTER_CI_RUN_NAME = "Release after CI of {sha}"
# The rehearsal's artifact that holds every file its release publishes; `test_release_workflow` holds the Rehearse job
# to it.
BUNDLE_NAME = "release-bundle-{tag}-{tree}"
RELEASE_WORKFLOW = ".github/workflows/release.yml"
CI_WORKFLOW = ".github/workflows/ci.yml"
# The local reusable workflow that release.yml calls: GitHub's record of that call names the commit whose workflow a run
# ran. `test_release_workflow` holds release.yml to it.
CALLED_WORKFLOW = ".github/workflows/openot-real-databases.yml"
FULL_SHA = re.compile(r"[0-9a-f]{40}")
# The events of the Release runs that rehearse: a pull request of this repository, and a maintainer's manual run.
REHEARSAL_EVENTS = ("pull_request", "workflow_dispatch")
# The pages of Release runs after CI that the completeness check reads, 100 runs each. GitHub lists at most 1,000 runs
# for a filtered query and answers an empty page 11, so a full page 10 means the list may be cut off.
MAX_RUN_PAGES = 10
MAIN_BRANCHES = ("main", "master")


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
class Rehearsal:
    """A passed rehearsal whose kept files a release promotes: its run, its bundle artifact and the commit it built."""

    run_id: int
    artifact_id: int
    commit: str


@dataclass(frozen=True)
class Plan:
    mode: str  # "rehearsal", "release", "package" or "skip"
    sha: str
    version: str
    reason: str
    platforms: tuple[str, ...] = ()
    # The rehearsal whose files a promoted release publishes; None for every other plan.
    rehearsal: Rehearsal | None = None

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


def tree_of(sha: str) -> str:
    """The tree of the commit `sha`."""
    return git("rev-parse", f"{sha}^{{tree}}").strip()


def runs_since(sha: str) -> str:
    """A day no later than the first Release run of `sha` can start: the day before its commit date, in UTC, so that
    a clock or time-zone difference cannot hide a run."""
    committed = datetime.fromtimestamp(int(git("show", "-s", "--format=%ct", sha).strip()), timezone.utc)
    return (committed - timedelta(days=1)).date().isoformat()


def github_get(path: str, token: str) -> dict | list:
    """One GitHub API answer: an object, or an array for a list such as a commit's pull requests; a failed request or
    another kind of answer fails the plan."""
    request = urllib.request.Request(
        f"https://api.github.com{path}",
        headers={"Accept": "application/vnd.github+json", "Authorization": f"Bearer {token}"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        answer = json.load(response)
    if not isinstance(answer, (dict, list)):
        raise ValueError(f"GitHub answered neither an object nor an array for {path}")
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


def release_runs(
    repository: str, sha: str, token: str, *, since: str, get: Callable[[str, str], dict] = github_get
) -> list[dict]:
    """The Release runs of `sha`: those that main's CI of `sha` started, which carry it in their run name, and those
    that a push of `sha` to main started, whose head is `sha`. A run after CI never counts by its head: that is main's
    newest commit when it starts, so it can be `sha` while the run releases an earlier commit. No other run counts: a
    pull request's or a manual run's workflow comes from the code it runs, which could name a job Publish. `since` is
    a day no later than the first of them started; when GitHub lists the most it lists, the check fails."""
    runs = {}
    for run in get(f"/repos/{repository}/actions/workflows/release.yml/runs?head_sha={sha}&per_page=100", token).get(
        "workflow_runs"
    ) or []:
        if run.get("event") == "push":
            runs[run["id"]] = run
    name = AFTER_CI_RUN_NAME.format(sha=sha)
    for page in range(1, MAX_RUN_PAGES + 1):
        listed = get(
            f"/repos/{repository}/actions/workflows/release.yml/runs?event=workflow_run&created=%3E%3D{since}"
            f"&per_page=100&page={page}",
            token,
        ).get("workflow_runs") or []
        runs.update((run["id"], run) for run in listed if run.get("display_title") == name)
        if len(listed) < 100:
            return list(runs.values())
    raise ValueError(
        f"more than {MAX_RUN_PAGES * 100} Release runs after CI since {since}; cannot tell whether {sha} was released"
    )


def verifying_run(
    repository: str, sha: str, token: str, *, since: str, get: Callable[[str, str], dict] = github_get
) -> dict | None:
    """A Release run of `sha` that, in any of its attempts, passed `Verify the release`; None when there is none."""
    for run in release_runs(repository, sha, token, since=since, get=get):
        jobs = get(f"/repos/{repository}/actions/runs/{run['id']}/jobs?filter=all&per_page=100", token)
        if verified(jobs.get("jobs") or []):
            return run
    return None


def release_complete(
    repository: str, sha: str, token: str, *, since: str, get: Callable[[str, str], dict] = github_get
) -> bool:
    """Whether a Release run of `sha`, in any of its attempts, passed `Verify the release`."""
    return verifying_run(repository, sha, token, since=since, get=get) is not None


def workflow_commit(run: dict, repository: str, own_ref: str) -> str | None:
    """The commit whose workflow `run` ran, by GitHub's record of its local call of CALLED_WORKFLOW. GitHub records a
    local call with the calling commit's full sha in its path, `<repository>/<workflow>@<sha>`, and with the run's own
    ref, `own_ref`; an explicit call of this repository's workflow records the ref name in the path, or no ref when it
    names a sha. Such a record, two records or none give None."""
    prefix = f"{repository}/{CALLED_WORKFLOW}@"
    records = [called for called in run.get("referenced_workflows") or []
               if isinstance(called, dict) and str(called.get("path") or "").startswith(prefix)]
    if len(records) != 1:
        return None
    sha = records[0].get("sha")
    if not isinstance(sha, str) or not FULL_SHA.fullmatch(sha):
        return None
    if records[0].get("path") != prefix + sha or records[0].get("ref") != own_ref:
        return None
    return sha


def merged_pull_request(run: dict, repository: str, token: str, get: Callable[[str, str], dict | list]) -> int | None:
    """The number of the pull request that a rehearsal ran for, when it is merged into this repository's main: of the
    pull requests that GitHub lists for the run's head, exactly one has that head, came from the run's head branch of
    this repository, targets this repository's main and is merged. After the merge GitHub lists only the merged pull
    request. A pull request of the same head that is open, into another base or from another branch, two candidates or
    none give None."""
    pulls = get(f"/repos/{repository}/commits/{run['head_sha']}/pulls?per_page=100", token)
    if not isinstance(pulls, list):
        return None
    candidates = [pull for pull in pulls
                  if isinstance(pull, dict) and (pull.get("head") or {}).get("sha") == run["head_sha"]]
    if len(candidates) != 1:
        return None
    pull = candidates[0]
    head, base = pull.get("head") or {}, pull.get("base") or {}
    if (
        base.get("ref") == "main"
        and (base.get("repo") or {}).get("full_name") == repository
        and head.get("ref") == run.get("head_branch")
        and (head.get("repo") or {}).get("full_name") == repository
        and pull.get("merged_at")
        and isinstance(pull.get("number"), int)
    ):
        return pull["number"]
    return None


def ci_passed(repository: str, commit: str, tree: str, token: str, get: Callable[[str, str], dict]) -> bool:
    """Whether a CI run of this repository for the head `commit` passed, whose head commit has `tree` by GitHub's
    record of it."""
    listed = get(f"/repos/{repository}/actions/workflows/ci.yml/runs?head_sha={commit}&status=success&per_page=100", token)
    return any(
        isinstance(run, dict)
        and run.get("path") == CI_WORKFLOW
        and run.get("status") == "completed"
        and run.get("conclusion") == "success"
        and (run.get("repository") or {}).get("full_name") == repository
        and (run.get("head_repository") or {}).get("full_name") == repository
        and (run.get("head_commit") or {}).get("id") == commit
        and (run.get("head_commit") or {}).get("tree_id") == tree
        for run in listed.get("workflow_runs") or []
    )


def find_rehearsal(
    repository: str, version: str, tree: str, token: str, get: Callable[[str, str], dict | list] = github_get,
) -> Rehearsal | None:
    """The passed rehearsal, of the highest artifact id, that kept the release files of `version` for a commit with
    `tree`.

    It asks GitHub for the artifacts named `release-bundle-v<version>-<tree>`, and takes one only when it has not
    expired and its run is a Release run of this repository that a pull request or a manual run started and that
    completed with success; when a pull request's rehearsal ran for the one pull request of its head that is merged
    into main from the run's head branch; when GitHub's records give `tree` to the run's head commit and to the commit
    whose workflow it ran, by the run's local call of CALLED_WORKFLOW at its own ref (a pull request's test merge
    commit `refs/pull/<number>/merge`, which merges that head, or a manual run's head on its branch); and when a CI run
    of the same head passed on `tree`. A pull request into a branch with another Release workflow runs a merge of
    another tree. No pull request text is read."""
    name = BUNDLE_NAME.format(tag=f"v{version}", tree=tree)
    listed = get(f"/repos/{repository}/actions/artifacts?name={name}&per_page=100", token).get("artifacts") or []
    candidates = [
        artifact for artifact in listed
        if isinstance(artifact, dict)
        and artifact.get("name") == name
        and artifact.get("expired") is False
        and isinstance(artifact.get("id"), int)
        and isinstance((artifact.get("workflow_run") or {}).get("id"), int)
    ]
    for artifact in sorted(candidates, key=lambda artifact: artifact["id"], reverse=True):
        run = get(f"/repos/{repository}/actions/runs/{artifact['workflow_run']['id']}", token)
        head = run.get("head_commit") or {}
        if not (
            run.get("path") == RELEASE_WORKFLOW
            and run.get("event") in REHEARSAL_EVENTS
            and run.get("status") == "completed"
            and run.get("conclusion") == "success"
            and (run.get("repository") or {}).get("full_name") == repository
            and (run.get("head_repository") or {}).get("full_name") == repository
            and head.get("id") == run.get("head_sha")
            and head.get("tree_id") == tree
        ):
            continue
        if run["event"] == "pull_request":
            number = merged_pull_request(run, repository, token, get)
            if number is None:
                continue
            own_ref = f"refs/pull/{number}/merge"
        else:
            own_ref = f"refs/heads/{run.get('head_branch')}"
        commit = workflow_commit(run, repository, own_ref)
        if commit is None or (run["event"] != "pull_request" and commit != run["head_sha"]):
            continue
        record = get(f"/repos/{repository}/git/commits/{commit}", token)
        parents = [parent.get("sha") for parent in record.get("parents") or [] if isinstance(parent, dict)]
        if (record.get("tree") or {}).get("sha") != tree:
            continue
        if run["event"] == "pull_request" and run["head_sha"] not in parents:
            continue
        if not ci_passed(repository, run["head_sha"], tree, token, get):
            continue
        return Rehearsal(run_id=run["id"], artifact_id=artifact["id"], commit=run["head_sha"])
    return None


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
    push_ref: str = "",
    push_sha: str = "",
    version_at: Callable[[str], str] = version_at,
    tag_commit: Callable[[str], tuple[str, str] | None] = tag_commit,
    release_complete: Callable[[str], bool] = lambda tag: False,
    version_commits: Callable[[str], list[str]] = version_commits,
    find_rehearsal: Callable[[str, str], Rehearsal | None] = lambda version, sha: None,
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
    released = dict(version_at=version_at, tag_commit=tag_commit, release_complete=release_complete,
                    version_commits=version_commits)
    if event == "push":
        if push_ref not in {f"refs/heads/{branch}" for branch in MAIN_BRANCHES}:
            return Plan("skip", push_sha, "", "only a push to main promotes a release")
        if repository != official:
            return Plan("skip", push_sha, "", "only the official repository releases")
        due = main_release(push_sha, **released)
        if due.mode != "release":
            return due
        rehearsal = find_rehearsal(due.version, push_sha)
        if rehearsal is None:
            return Plan("skip", push_sha, due.version,
                        f"no passed rehearsal kept the files of this commit's tree; {due.tag} follows main's CI")
        return promoted(due, rehearsal)
    if event != "workflow_run":
        return Plan("skip", "", "", f"a {event} event releases nothing")
    if (
        run_conclusion != "success"
        or run_event != "push"
        or run_branch not in MAIN_BRANCHES
        or run_head_repository != repository
    ):
        return Plan("skip", run_sha, "", "only a successful CI run of a push to this repository's main releases")
    if repository != official:
        return Plan("skip", run_sha, "", "only the official repository releases")
    due = main_release(run_sha, **released)
    if due.mode != "release":
        return due
    # The same rehearsal as a push's promotion, so that both publish the same bytes: a promotion that failed after
    # it published some packages is finished with the files it began with.
    rehearsal = find_rehearsal(due.version, run_sha)
    return due if rehearsal is None else promoted(due, rehearsal)


def promoted(due: Plan, rehearsal: Rehearsal) -> Plan:
    """The release `due`, promoted from the files of `rehearsal`: it builds no platform."""
    return replace(
        due,
        reason=f"{due.reason}; it promotes the files of rehearsal run {rehearsal.run_id}, which built "
               f"{rehearsal.commit[:9]} with this commit's tree",
        platforms=(),
        rehearsal=rehearsal,
    )


def main_release(
    sha: str,
    *,
    version_at: Callable[[str], str],
    tag_commit: Callable[[str], tuple[str, str] | None],
    release_complete: Callable[[str], bool],
    version_commits: Callable[[str], list[str]],
) -> Plan:
    """Whether the main commit `sha` is released: its version has no tag, or the tag is on `sha` and its release is
    not complete, and no earlier version of its first-parent line lacks a tag."""
    version = version_at(sha)
    tag = f"v{version}"
    found = tag_commit(tag)
    if found is not None:
        kind, commit = found
        if commit != sha:
            return Plan("skip", sha, version, f"{tag} was released from {commit}")
        if kind != "tag":
            raise ValueError(f"{tag} on {sha} is not an annotated tag")
        if release_complete(tag):
            return Plan("skip", sha, version, f"{tag} is released and complete")
    # A version brought before this one and never tagged is released first, by running its commit's
    # Release again; releasing past it would skip it for good.
    skipped = unreleased_earlier(
        sha, version, version_at=version_at, tag_commit=tag_commit, version_commits=version_commits
    )
    if skipped:
        listed = ", ".join(f"v{earlier} at {commit[:9]}" for earlier, commit in skipped)
        raise ValueError(f"{listed} has no tag; run the Release of that commit again before releasing {tag}")
    if found is None:
        return Plan("release", sha, version, f"{tag} has no tag yet", ALL_PLATFORMS)
    return Plan("release", sha, version, f"{tag} is on this commit and its release is not complete", ALL_PLATFORMS)


def check_unreleased(
    repository: str, sha: str, token: str, *, since: str, get: Callable[[str, str], dict] = github_get
) -> int:
    """0 when no Release run of `sha` has verified its release; 1, naming the run, when one has. The Publish job asks
    this once it holds the release concurrency group, since its plan decided before: a release never happens twice."""
    run = verifying_run(repository, sha, token, since=since, get=get)
    if run is not None:
        print(
            f"::error::Release run {run.get('html_url') or run['id']} already verified the release of {sha}; "
            "this run publishes nothing.",
            file=sys.stderr,
        )
        return 1
    print(f"release plan: no Release run of {sha} has verified its release")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    for name in (
        "event", "repository", "official", "pr-base", "pr-head", "pr-head-repository",
        "run-conclusion", "run-event", "run-branch", "run-sha", "run-head-repository", "dispatch-sha",
        "push-ref", "push-sha", "check-unreleased",
    ):
        parser.add_argument(f"--{name}", default="")
    parser.add_argument("--release-paths", type=Path, default=Path(".github/paths/release.txt"))
    parser.add_argument("--package-paths", type=Path, default=Path(".github/paths/package.txt"))
    args = parser.parse_args(argv)
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if args.check_unreleased:
        return check_unreleased(args.repository, args.check_unreleased, token, since=runs_since(args.check_unreleased))
    released_sha = args.push_sha if args.event == "push" else args.run_sha
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
        push_ref=args.push_ref,
        push_sha=args.push_sha,
        release_paths=args.release_paths,
        package_paths=args.package_paths,
        release_complete=lambda tag: release_complete(
            args.repository, released_sha, token, since=runs_since(released_sha)
        ),
        find_rehearsal=lambda version, sha: find_rehearsal(args.repository, version, tree_of(sha), token),
    )
    rehearsal = result.rehearsal
    lines = [f"mode={result.mode}", f"sha={result.sha}", f"version={result.version}", f"tag={result.tag}",
             f"rehearsal_run={rehearsal.run_id if rehearsal else ''}",
             f"rehearsal_artifact={rehearsal.artifact_id if rehearsal else ''}"]
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
