#!/usr/bin/env python3
"""Refuse to publish packages that differ from those a GitHub release of the same tag already holds (specification
24, "Release after main CI").

A release can be continued by an attempt or a run other than the one that began it, as when the release after
main's CI finishes a promotion that failed after it published. The Create GitHub Release step replaces an asset of the
same name and the Marketplace keeps a VS Code package it already has, so packages that differ from those already
published would leave the GitHub release and the Marketplace with different bytes of one version. Before anything is
tagged or published, this check asks GitHub for the release of the tag. Without one it passes. With one, every package
on it (a VS Code package, a runtime or language-server archive, a toolchain bundle) must be one this run publishes, with
the SHA-256 digest that GitHub recorded for the uploaded asset; an asset still uploading is not published and is not
compared. The release evidence (provenance, conformance status, SHA256SUMS) is written anew by every run and attempt, so
it is not compared either.

Usage, at the root of the workspace that holds the release folders:

    GITHUB_TOKEN=... python3 scripts/check_published_release.py --repository OWNER/NAME --tag vX.Y.Z
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable, Sequence

# The packages that the Create GitHub Release step uploads: each folder with the endings it takes from it.
PACKAGES = {
    "vsix-artifacts": (".vsix",),
    "runtime-artifacts": (".tar.gz", ".zip"),
    "trust-lsp-artifacts": (".tar.gz", ".zip"),
    "toolchain-artifacts": (".tar.gz",),
}
PACKAGE_ENDINGS = (".vsix", ".tar.gz", ".zip")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def github_release(repository: str, tag: str, token: str) -> dict | None:
    """GitHub's release of `tag`, or None when GitHub has none; any other answer fails."""
    request = urllib.request.Request(
        f"https://api.github.com/repos/{repository}/releases/tags/{urllib.parse.quote(tag)}",
        headers={"Accept": "application/vnd.github+json", "Authorization": f"Bearer {token}"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            answer = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise
    if not isinstance(answer, dict):
        raise ValueError(f"GitHub answered a non-object for the release of {tag}")
    return answer


def local_packages(root: Path) -> dict[str, str]:
    """The packages this run publishes, by asset name, with their SHA-256 digests."""
    found: dict[str, str] = {}
    for folder, endings in PACKAGES.items():
        for path in sorted((root / folder).glob("*")):
            if path.is_file() and not path.is_symlink() and path.name.endswith(endings):
                if path.name in found:
                    raise ValueError(f"two packages are named {path.name}")
                found[path.name] = _sha256(path)
    return found


def differences(release: dict, ours: dict[str, str]) -> list[str]:
    """Why the packages already on `release` are not this run's, byte for byte; empty when they are."""
    problems = []
    for asset in release.get("assets") or []:
        if not isinstance(asset, dict):
            continue
        name = str(asset.get("name") or "")
        if not name.endswith(PACKAGE_ENDINGS) or asset.get("state") != "uploaded":
            continue
        if name not in ours:
            problems.append(f"{name} is published, and this run does not publish it")
        elif asset.get("digest") != f"sha256:{ours[name]}":
            problems.append(
                f"{name} is published as {asset.get('digest') or 'an asset without a digest'}, "
                f"and this run's is sha256:{ours[name]}"
            )
    return problems


def check(
    repository: str, tag: str, token: str, root: Path,
    release_of: Callable[[str, str, str], dict | None] | None = None,
) -> int:
    release = (release_of or github_release)(repository, tag, token)
    if release is None:
        print(f"published release: GitHub has no release of {tag} yet")
        return 0
    ours = local_packages(root)
    problems = differences(release, ours)
    if problems:
        for problem in problems:
            print(f"::error::published release: {problem}", file=sys.stderr)
        print(f"::error::published release: {tag} holds other packages than this run's; this run publishes nothing.",
              file=sys.stderr)
        return 1
    print(f"published release: the packages already on {tag} are this run's, byte for byte ({len(ours)} to publish)")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args(argv)
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    try:
        return check(args.repository, args.tag, token, args.root)
    except (OSError, ValueError) as error:
        print(f"::error::published release: cannot compare with the release of {args.tag}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
