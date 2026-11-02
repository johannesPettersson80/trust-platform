#!/usr/bin/env python3
"""The version bump of a release (AGENTS.md "Releases", specification 22, "Change and release
workflow"): the workspace version, the ESP32-C6 firmware's version, the workspace crates in the three
Cargo lockfiles, the VS Code package and its lockfile's root entries, and the changelog: the
fragments under `changelog.d/` are gathered into `## [NEW] - DATE` under an empty `## [Unreleased]`,
the newest first, and removed. Every replacement must find exactly what it expects, or nothing is
written.

Usage: bump_version.py OLD NEW DATE   (run at the root of the checkout to bump)"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import changelog_fragments  # noqa: E402

LOCKFILES = ("Cargo.lock", "firmware/trust-esp32c6/Cargo.lock", "firmware/trust-stm32u5a9/Cargo.lock")


def replace_once(files: dict[Path, str], path: Path, before: str, after: str) -> None:
    text = files.get(path, path.read_text(encoding="utf-8"))
    count = text.count(before)
    if count != 1:
        raise ValueError(f"{path}: expected one {before!r}, found {count}")
    files[path] = text.replace(before, after)


def bump_lock(files: dict[Path, str], path: Path, old: str, new: str) -> int:
    """The workspace crates of a lockfile have no `source`: their version moves, the registry and git
    packages stay. Answers how many moved."""
    blocks = path.read_text(encoding="utf-8").split("\n[[package]]\n")
    changed = 0
    for index, block in enumerate(blocks[1:], start=1):
        if "\nsource = " not in "\n" + block and f'\nversion = "{old}"' in "\n" + block:
            blocks[index] = block.replace(f'version = "{old}"', f'version = "{new}"', 1)
            changed += 1
    if not changed:
        raise ValueError(f"{path}: no workspace crate at {old}")
    files[path] = "\n[[package]]\n".join(blocks)
    return changed


def added_at(root: Path, path: Path) -> int:
    """When `path` was added to the repository (seconds), 0 when git does not know it."""
    result = subprocess.run(
        ["git", "log", "--diff-filter=A", "--format=%ct", "-1", "--", str(path.relative_to(root))],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    text = result.stdout.strip()
    return int(text) if result.returncode == 0 and text.isdigit() else 0


def bump(root: Path, old: str, new: str, day: str) -> list[str]:
    """Bumps `old` to `new` in the checkout at `root`; answers what it changed."""
    files: dict[Path, str] = {}
    report = []
    cargo = root / "Cargo.toml"
    workspace = cargo.read_text(encoding="utf-8")
    section = re.search(r"(?ms)^\[workspace\.package\]\n(.*?)(?=^\[)", workspace)
    if not section or f'version = "{old}"' not in section.group(1):
        raise ValueError("Cargo.toml: [workspace.package] is not at the old version")
    files[cargo] = workspace.replace(
        section.group(0), section.group(0).replace(f'version = "{old}"', f'version = "{new}"', 1)
    )
    replace_once(files, root / "firmware/trust-esp32c6/Cargo.toml", f'version = "{old}"', f'version = "{new}"')
    for lock in LOCKFILES:
        report.append(f"{lock}: {bump_lock(files, root / lock, old, new)} workspace crates")
    package = root / "editors/vscode/package.json"
    replace_once(files, package, f'"version": "{old}"', f'"version": "{new}"')
    package_lock = root / "editors/vscode/package-lock.json"
    lock = json.loads(package_lock.read_text(encoding="utf-8"))
    if lock["version"] != old or lock["packages"][""]["version"] != old:
        raise ValueError("editors/vscode/package-lock.json: its root is not at the old version")
    replace_once(
        files,
        package_lock,
        f'  "name": "trust-lsp",\n  "version": "{old}",',
        f'  "name": "trust-lsp",\n  "version": "{new}",',
    )
    replace_once(
        files,
        package_lock,
        f'      "name": "trust-lsp",\n      "version": "{old}",',
        f'      "name": "trust-lsp",\n      "version": "{new}",',
    )
    paths = changelog_fragments.fragment_paths(root)
    newest_first = sorted(paths, key=lambda path: (-added_at(root, path), path.name))
    fragments = [changelog_fragments.fragment(path) for path in newest_first]
    changelog = root / "CHANGELOG.md"
    files[changelog] = changelog_fragments.gather(changelog.read_text(encoding="utf-8"), fragments, new, day)
    for path, text in files.items():
        path.write_text(text, encoding="utf-8")
    for path in paths:
        path.unlink()
    report.append(f"bumped {old} -> {new} in {len(files)} files; {len(paths)} changelog fragments gathered")
    return report


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    old, new, day = argv
    try:
        report = bump(Path.cwd(), old, new, day)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 1
    print("\n".join(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
