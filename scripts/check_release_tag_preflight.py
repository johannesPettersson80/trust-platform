#!/usr/bin/env python3
"""Validate the tag-local preconditions for a release workflow."""

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


def tag_is_on_main(tagged_sha: str) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", tagged_sha, "origin/main"],
        check=False,
        text=True,
        capture_output=True,
    )
    return result.returncode == 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail a tag-triggered release when its tag or version is invalid."
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
        tag_type = run_git(["cat-file", "-t", f"refs/tags/{args.tag}"])
        tagged_sha = run_git(["rev-parse", f"{args.tag}^{{}}"])
        head_sha = run_git(["rev-parse", "HEAD"])
    except RuntimeError as error:
        return fail(str(error))

    if tag_type != "tag":
        return fail(f"Release tag {args.tag} must be an annotated tag.")
    if tagged_sha != head_sha:
        return fail(
            f"Release checkout HEAD {head_sha} does not match {args.tag} at {tagged_sha}."
        )
    if not tag_is_on_main(tagged_sha):
        return fail(f"Release tag {args.tag} is not reachable from origin/main.")

    print(f"release-tag-preflight: OK ({expected_tag} -> {tagged_sha})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
