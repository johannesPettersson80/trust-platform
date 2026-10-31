#!/usr/bin/env python3
"""The artifact `docs-captures`: the screenshots that the Docs Captures workflow renders and that
the docs site publishes without a commit of the images.

Specification 22, "Docs Captures": the workflow renders the documentation's screenshots on every
relevant push to `main`, every night and by hand. `collect` copies what the capture steps wrote,
the `.png` and `.gif` files below the images folder that are newer than a mark set before the
captures, into the folder the workflow uploads as the artifact; images nobody renders, such as the
brand and the hand-made ones, never enter it. Collecting nothing fails, because the captures then
rendered nothing.

The Docs Pages deploy runs `take` before it builds the site. It lays the artifact of the newest
successful Docs Captures run of this repository's `main` that a push, the schedule or a dispatch
started, and that kept the artifact, over the images folder, so the published site shows the
product at `main`. A pull request's run never counts. When no such run exists the images in git
stay, and `take` says so; a failed GitHub request fails it.

Usage (`take` needs `gh` authenticated with GH_TOKEN and the repository's `actions: read`):

    python3 scripts/docs_captures_artifact.py collect --images docs/public/assets/images \\
        --since "$RUNNER_TEMP/captures-start" --out "$RUNNER_TEMP/docs-captures"
    python3 scripts/docs_captures_artifact.py take --repo OWNER/NAME --dest docs/public/assets/images
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path, PurePosixPath
from typing import Callable, Iterable

ARTIFACT = "docs-captures"
WORKFLOW_PATH = ".github/workflows/docs-captures.yml"
TRUSTED_EVENTS = {"push", "schedule", "workflow_dispatch"}
IMAGE_SUFFIXES = {".png", ".gif"}
# The newest successful runs of main that `take` looks through; the nightly run keeps one in reach.
RUNS_LOOKED_AT = 50


def gh_json(path: str) -> dict:
    result = subprocess.run(["gh", "api", path], check=True, capture_output=True, text=True)
    value = json.loads(result.stdout)
    if not isinstance(value, dict):
        raise ValueError(f"gh api {path} answered a non-object")
    return value


def newest_run(repo: str, runs: Iterable[dict], artifacts_of: Callable[[int], list[dict]]) -> int | None:
    """The id of the first run of `runs`, which GitHub lists newest first, that is a successful Docs
    Captures run of this repository's `main` that a push, the schedule or a dispatch started and
    that kept an unexpired `docs-captures` artifact; `None` when there is none."""
    for run in runs:
        run_id = run.get("id")
        if not (
            isinstance(run_id, int)
            and run.get("path") == WORKFLOW_PATH
            and run.get("event") in TRUSTED_EVENTS
            and run.get("conclusion") == "success"
            and run.get("head_branch") == "main"
            and (run.get("head_repository") or {}).get("full_name") == repo
        ):
            continue
        # A run whose change detection skipped the captures uploaded nothing.
        if any(artifact.get("name") == ARTIFACT and not artifact.get("expired") for artifact in artifacts_of(run_id)):
            return run_id
    return None


def copy_images(source: Path, dest: Path, newer_than: float | None = None) -> list[str]:
    """Copies every `.png` and `.gif` below `source`, or only those modified after `newer_than`, to
    the same relative path below `dest`, and returns those paths; a path that leaves `dest` is
    refused."""
    copied = []
    dest = dest.resolve()
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in IMAGE_SUFFIXES:
            continue
        if newer_than is not None and path.stat().st_mtime <= newer_than:
            continue
        relative = PurePosixPath(path.relative_to(source).as_posix())
        target = (dest / relative).resolve()
        if dest not in target.parents:
            raise ValueError(f"{relative} would leave {dest}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        copied.append(str(relative))
    return copied


def collect(images: Path, since: Path, out: Path) -> int:
    copied = copy_images(images, out, since.stat().st_mtime)
    if not copied:
        print(f"docs_captures_artifact: no screenshot below {images} is newer than {since}", file=sys.stderr)
        return 1
    print(f"docs_captures_artifact: {len(copied)} rendered screenshot(s)")
    for path in copied:
        print(f"  {path}")
    return 0


def take(repo: str, dest: Path) -> int:
    listing = gh_json(
        f"repos/{repo}/actions/workflows/{Path(WORKFLOW_PATH).name}/runs"
        f"?branch=main&status=success&per_page={RUNS_LOOKED_AT}"
    )
    runs = listing.get("workflow_runs")
    if not isinstance(runs, list):
        raise ValueError("gh api answered no run list")

    def artifacts_of(run_id: int) -> list[dict]:
        artifacts = gh_json(f"repos/{repo}/actions/runs/{run_id}/artifacts").get("artifacts")
        if not isinstance(artifacts, list):
            raise ValueError(f"gh api answered no artifact list for run {run_id}")
        return artifacts

    run_id = newest_run(repo, runs, artifacts_of)
    if run_id is None:
        print("docs_captures_artifact: no screenshots of main to take; the images in git stay")
        return 0
    with tempfile.TemporaryDirectory() as folder:
        subprocess.run(
            ["gh", "run", "download", str(run_id), "--repo", repo, "--name", ARTIFACT, "--dir", folder],
            check=True,
        )
        copied = copy_images(Path(folder), dest)
    print(f"docs_captures_artifact: {len(copied)} screenshot(s) from run {run_id} of main")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    collecting = commands.add_parser("collect", help="copy the screenshots the captures wrote")
    collecting.add_argument("--images", required=True, type=Path)
    collecting.add_argument("--since", required=True, type=Path)
    collecting.add_argument("--out", required=True, type=Path)
    taking = commands.add_parser("take", help="lay the newest screenshots of main over the docs")
    taking.add_argument("--repo", required=True)
    taking.add_argument("--dest", required=True, type=Path)
    args = parser.parse_args(argv)
    if args.command == "collect":
        return collect(args.images, args.since, args.out)
    return take(args.repo, args.dest)


if __name__ == "__main__":
    sys.exit(main())
