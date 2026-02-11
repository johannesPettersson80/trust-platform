#!/usr/bin/env python3
"""Validate release version alignment across workspace and VS Code manifests."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path


def workspace_version_from_cargo(path: Path) -> str:
    payload = tomllib.loads(path.read_text(encoding="utf-8"))
    try:
        return payload["workspace"]["package"]["version"]
    except (KeyError, TypeError) as exc:
        raise RuntimeError(
            f"Could not read [workspace.package].version from {path}"
        ) from exc


def package_json_version(path: Path) -> str:
    payload = json.loads(path.read_text(encoding="utf-8"))
    version = payload.get("version")
    if not isinstance(version, str) or not version.strip():
        raise RuntimeError(f"Could not read non-empty string version from {path}")
    return version.strip()


def package_lock_versions(path: Path) -> tuple[str, str]:
    payload = json.loads(path.read_text(encoding="utf-8"))

    top_level = payload.get("version")
    if not isinstance(top_level, str) or not top_level.strip():
        raise RuntimeError(
            f"Could not read non-empty top-level version from {path}"
        )

    root_package = payload.get("packages", {}).get("", {})
    root_level = root_package.get("version")
    if not isinstance(root_level, str) or not root_level.strip():
        raise RuntimeError(
            f"Could not read non-empty packages[''].version from {path}"
        )

    return top_level.strip(), root_level.strip()


def fail(messages: list[str]) -> int:
    for message in messages:
        print(f"::error::{message}", file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Fail when workspace version and VS Code extension manifest versions "
            "are not aligned."
        )
    )
    parser.add_argument("--cargo-toml", default="Cargo.toml")
    parser.add_argument("--package-json", default="editors/vscode/package.json")
    parser.add_argument(
        "--package-lock-json", default="editors/vscode/package-lock.json"
    )
    args = parser.parse_args()

    cargo_path = Path(args.cargo_toml)
    package_json_path = Path(args.package_json)
    package_lock_path = Path(args.package_lock_json)

    workspace_version = workspace_version_from_cargo(cargo_path)
    extension_version = package_json_version(package_json_path)
    lock_top_version, lock_root_version = package_lock_versions(package_lock_path)

    errors: list[str] = []
    if extension_version != workspace_version:
        errors.append(
            f"Version mismatch: {package_json_path}={extension_version} "
            f"but {cargo_path}={workspace_version}."
        )
    if lock_top_version != workspace_version:
        errors.append(
            f"Version mismatch: {package_lock_path} top-level version={lock_top_version} "
            f"but {cargo_path}={workspace_version}."
        )
    if lock_root_version != workspace_version:
        errors.append(
            f"Version mismatch: {package_lock_path} packages[''].version={lock_root_version} "
            f"but {cargo_path}={workspace_version}."
        )
    if lock_top_version != lock_root_version:
        errors.append(
            f"Version mismatch inside {package_lock_path}: top-level version={lock_top_version} "
            f"but packages[''].version={lock_root_version}."
        )

    if errors:
        errors.append(
            "Keep workspace and VS Code extension versions synchronized to avoid "
            "release publish conflicts."
        )
        return fail(errors)

    print(
        "release-version-alignment: OK "
        f"(workspace={workspace_version}, package.json={extension_version}, "
        f"package-lock={lock_top_version}/{lock_root_version})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
