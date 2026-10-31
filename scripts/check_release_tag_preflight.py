#!/usr/bin/env python3
"""Validate the tag-local preconditions of a release (specification 24, "Release preflight").

The Release workflow creates the tag `v<version>` after the builds and tests, so before them the
tag must not exist yet, or must be an annotated tag on the checked-out commit when a release is
resumed. The checked-out commit must be on `origin/main`.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from pathlib import Path


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
        raise RuntimeError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def tag_exists(tag: str) -> bool:
    """Whether the checkout has the tag; a failing lookup is an error, not an absent tag."""
    result = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", f"refs/tags/{tag}"],
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode not in (0, 1):
        raise RuntimeError(f"git show-ref refs/tags/{tag} failed: {result.stderr.strip()}")
    return result.returncode == 0


def tag_is_on_main(sha: str) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", sha, "origin/main"],
        check=False,
        text=True,
        capture_output=True,
    )
    return result.returncode == 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail a release whose tag or version is invalid before it builds anything."
    )
    parser.add_argument("--tag", required=True)
    args = parser.parse_args()

    version = workspace_version()
    expected_tag = f"v{version}"
    if args.tag != expected_tag:
        return fail(
            f"Release tag {args.tag} does not match workspace version {version}; "
            f"expected {expected_tag}."
        )

    try:
        head_sha = run_git(["rev-parse", "HEAD"])
        if tag_exists(args.tag):
            tag_type = run_git(["cat-file", "-t", f"refs/tags/{args.tag}"])
            tagged_sha = run_git(["rev-parse", f"{args.tag}^{{}}"])
        else:
            tag_type, tagged_sha = None, None
    except RuntimeError as error:
        return fail(str(error))

    if tag_type is not None:
        if tag_type != "tag":
            return fail(f"Release tag {args.tag} must be an annotated tag.")
        if tagged_sha != head_sha:
            return fail(
                f"Release checkout HEAD {head_sha} does not match {args.tag} at {tagged_sha}."
            )
    if not tag_is_on_main(head_sha):
        return fail(f"Release commit {head_sha} is not reachable from origin/main.")

    state = f"on {head_sha}" if tag_type is not None else f"not yet tagged, {head_sha}"
    print(f"release-tag-preflight: OK ({expected_tag} {state})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
