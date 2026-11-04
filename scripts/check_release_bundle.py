#!/usr/bin/env python3
"""Check the release files a rehearsal kept, before a release promotes them (specification 24, "Release after main CI").

The rehearsal's bundle, `release-bundle-v<version>-<tree>`, holds the release folders (the VS Code packages, the runtime
and language-server archives, the toolchain bundles, and `release-evidence/` with the conformance status and the
rehearsal's `release-provenance.json` and `SHA256SUMS`) and the release text, `release-notes.md`. A promoted release
publishes them only when
- every line of SHA256SUMS names a file of a release folder by its relative path, and that file has the digest;
- every file of the release folders is named by SHA256SUMS, so that nothing unchecked is published;
- the rehearsal's provenance names the release's tag, the released commit's tree and the rehearsal run the plan chose,
  and lists exactly the files of SHA256SUMS with the same digests;
- every platform has one VS Code package, one runtime and one language-server archive and one toolchain bundle, and the
  conformance status is there;
- the release text is there.
The bundle is downloaded into an empty folder outside the checkout (`--bundle`), so that none of its entries can replace
a file of the checkout, this script among them. It must hold the release folders and the release text and nothing
else. Only once all of it is checked does the script move them into the workspace, refusing to replace anything there,
and then it moves the rehearsal's provenance, checksums and release text to the `--set-aside` folder: the release writes
its own provenance and checksums, and compares the text it writes from the changelog with the rehearsal's.

Usage, at the root of the workspace:

    python3 scripts/check_release_bundle.py --bundle DOWNLOADED --tag vX.Y.Z --tree TREE --run-id RUN \
        --set-aside rehearsal-evidence
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path, PurePosixPath
from typing import Sequence

try:
    from scripts.generate_release_provenance import PLATFORMS
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    from generate_release_provenance import PLATFORMS  # type: ignore[no-redef]

# The folders whose files the Publish job releases, as the release-files action lays them out.
FOLDERS = ("vsix-artifacts", "runtime-artifacts", "trust-lsp-artifacts", "toolchain-artifacts", "release-evidence")
SUMS = PurePosixPath("release-evidence/SHA256SUMS")
PROVENANCE = PurePosixPath("release-evidence/release-provenance.json")
TEXT = PurePosixPath("release-notes.md")
# Each platform's packages, by the kinds of the provenance record, and the evidence that holds for every platform.
PACKAGE_KINDS = ("vsix", "runtime", "language_server", "toolchain_bundle")
EVIDENCE_KINDS = ("conformance_json", "conformance_markdown")
# A line that `sha256sum` writes: the digest, two spaces, the path.
SUMS_LINE = re.compile(r"([0-9a-f]{64})  (\S.*)")


class BundleError(ValueError):
    """Why a rehearsal's files cannot be promoted."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _regular(path: Path) -> bool:
    return path.is_file() and not path.is_symlink()


def read_sums(root: Path) -> dict[PurePosixPath, str]:
    """The files that the rehearsal's SHA256SUMS lists, each with its digest."""
    if not _regular(root / SUMS):
        raise BundleError(f"the bundle has no {SUMS}")
    listed: dict[PurePosixPath, str] = {}
    for number, line in enumerate((root / SUMS).read_text(encoding="utf-8").splitlines(), 1):
        match = SUMS_LINE.fullmatch(line)
        if not match:
            raise BundleError(f"{SUMS} line {number} is not '<sha256>  <path>': {line!r}")
        digest, name = match.groups()
        path = PurePosixPath(name)
        outside = path.is_absolute() or ".." in path.parts or "\\" in name or len(path.parts) < 2
        if outside or path.parts[0] not in FOLDERS:
            raise BundleError(f"{SUMS} names {name!r}, which is not a file of a release folder")
        if path in listed:
            raise BundleError(f"{SUMS} names {name} twice")
        listed[path] = digest
    if not listed:
        raise BundleError(f"{SUMS} lists no file")
    return listed


def check_files(root: Path, listed: dict[PurePosixPath, str]) -> None:
    """Every listed file has its digest, and the release folders hold no file that SHA256SUMS does not list."""
    for path, digest in listed.items():
        file = root.joinpath(*path.parts)
        if not _regular(file):
            raise BundleError(f"{path} is listed in {SUMS} but is not a file of the bundle")
        if _sha256(file) != digest:
            raise BundleError(f"{path} does not have the digest that {SUMS} lists")
    for folder in FOLDERS:
        base = root / folder
        if base.is_symlink() or not base.is_dir():
            raise BundleError(f"the bundle has no folder {folder}")
        for file in sorted(base.rglob("*")):
            relative = PurePosixPath(file.relative_to(root).as_posix())
            if file.is_symlink() or not (file.is_file() or file.is_dir()):
                raise BundleError(f"{relative} is not a regular file or folder")
            if file.is_file() and relative != SUMS and relative not in listed:
                raise BundleError(f"{relative} is in the bundle but not in {SUMS}")


def check_provenance(root: Path, listed: dict[PurePosixPath, str], *, tag: str, tree: str, run_id: str) -> None:
    """The rehearsal's own record: of this release's tag and the released commit's tree, written by the run the plan
    chose, listing exactly the checked files with their digests and every platform's packages."""
    try:
        record = json.loads((root / PROVENANCE).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise BundleError(f"the rehearsal's {PROVENANCE} cannot be read: {error}") from error
    if not isinstance(record, dict):
        raise BundleError(f"the rehearsal's {PROVENANCE} is not a JSON object")
    if record.get("tag") != tag:
        raise BundleError(f"the rehearsal kept the files of {record.get('tag')!r}, not of {tag}")
    if record.get("tree") != tree:
        raise BundleError(f"the rehearsal built tree {record.get('tree')!r}, not the released commit's tree {tree}")
    if str(record.get("workflow_run_id") or "") != run_id:
        raise BundleError(
            f"the files were written by run {record.get('workflow_run_id')!r}, not by rehearsal run {run_id}"
        )
    if record.get("promoted") is not False:
        raise BundleError("the files must be those a rehearsal built, not those of a promoted release")
    rows = record.get("artifacts")
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise BundleError(f"the rehearsal's {PROVENANCE} has no list of artifacts")
    checked: dict[str, str] = {}
    for path, digest in listed.items():
        if path == PROVENANCE:
            continue
        if path.name in checked:
            raise BundleError(f"two files of the bundle are named {path.name}")
        checked[path.name] = digest
    recorded = {row.get("path"): row.get("sha256") for row in rows}
    if len(recorded) != len(rows) or recorded != checked:
        raise BundleError(f"the rehearsal's {PROVENANCE} does not list exactly the files of {SUMS} with their digests")
    counts: dict[tuple[object, object], int] = {}
    for row in rows:
        key = (row.get("kind"), row.get("platform"))
        counts[key] = counts.get(key, 0) + 1
    wrong = [
        f"{kind} for {platform}"
        for platform in PLATFORMS
        for kind in PACKAGE_KINDS
        if counts.get((kind, platform)) != 1
    ]
    wrong += [kind for kind in EVIDENCE_KINDS if counts.get((kind, "all")) != 1]
    if wrong:
        raise BundleError("the bundle does not hold exactly one " + ", ".join(wrong))


def check_entries(root: Path) -> None:
    """The downloaded bundle holds the release folders and the release text, and nothing else."""
    expected = set(FOLDERS) | {str(TEXT)}
    for entry in sorted(root.iterdir()):
        if entry.name not in expected:
            raise BundleError(f"the bundle holds {entry.name}, which is no release file")
        if entry.is_symlink():
            raise BundleError(f"the bundle's {entry.name} is a link")


def check_bundle(root: Path, *, tag: str, tree: str, run_id: str) -> dict[PurePosixPath, str]:
    """The files of the bundle at `root`, checked as the module describes; a BundleError says what is wrong."""
    check_entries(root)
    listed = read_sums(root)
    check_files(root, listed)
    check_provenance(root, listed, tag=tag, tree=tree, run_id=run_id)
    text = root / TEXT
    if not _regular(text) or not text.read_text(encoding="utf-8").strip():
        raise BundleError(f"the bundle has no release text, {TEXT}")
    return listed


def move_into(bundle: Path, root: Path) -> None:
    """Move the checked release folders and text from the download folder into the workspace, replacing nothing."""
    names = [*FOLDERS, str(TEXT)]
    for name in names:
        if (root / name).exists() or (root / name).is_symlink():
            raise BundleError(f"{name} is in the workspace already")
    for name in names:
        shutil.move(bundle / name, root / name)


def set_aside(root: Path, folder: Path) -> None:
    """Move the rehearsal's provenance, checksums and release text out of the release's way."""
    target = root / folder
    target.mkdir(parents=True)
    for path in (PROVENANCE, SUMS, TEXT):
        shutil.move(root.joinpath(*path.parts), target / path.name)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", required=True, help="The release tag, v<version>.")
    parser.add_argument("--tree", required=True, help="The released commit's tree.")
    parser.add_argument("--run-id", required=True, help="The rehearsal run that the plan chose.")
    parser.add_argument("--set-aside", type=Path, required=True, help="Where the rehearsal's own records go.")
    parser.add_argument("--bundle", type=Path, required=True, help="The empty folder the bundle was downloaded into.")
    parser.add_argument("--root", type=Path, default=Path("."), help="The workspace that receives the release files.")
    args = parser.parse_args(argv)
    try:
        listed = check_bundle(args.bundle, tag=args.tag, tree=args.tree, run_id=args.run_id)
        move_into(args.bundle, args.root)
        set_aside(args.root, args.set_aside)
    except (BundleError, OSError) as error:
        print(f"::error::release bundle: {error}", file=sys.stderr)
        return 1
    print(f"release bundle: the {len(listed)} files of rehearsal run {args.run_id} match its SHA256SUMS, "
          f"for {args.tag} and tree {args.tree}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
