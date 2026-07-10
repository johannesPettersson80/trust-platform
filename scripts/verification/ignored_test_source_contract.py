"""Shared source-path contract for ignored-test discovery provenance."""

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path


def validate_discovery_path_contract(paths: Iterable[str]) -> list[str]:
    """Require every current discovery input to be visible to history checks."""

    unrecognized = sorted(
        {path for path in paths if not is_modeled_source_path(path)}
    )
    if not unrecognized:
        return []
    return [
        "current ignored-test discovery inputs are not recognized by the "
        "historical source predicate: " + ", ".join(unrecognized[:5])
    ]


def is_modeled_source_path(value: str) -> bool:
    """Identify tracked source paths whose deletion must stale a report."""

    if value.startswith(
        (
            "docs/internal/testing/evidence/",
            "target/",
            "gate-artifacts/",
            "conformance/reports/",
        )
    ):
        return False
    parts = value.split("/")
    name = parts[-1]
    suffix = Path(name).suffix

    if len(parts) == 3 and parts[0] == "crates" and name == "Cargo.toml":
        return True
    if len(parts) >= 4 and parts[0] == "crates":
        surface = parts[2]
        if surface == "src" and suffix == ".rs":
            return True
        if surface == "tests" and suffix in {".rs", ".st", ".pou"}:
            return True
        if "fuzz" in parts[2:] and suffix == ".rs":
            return True
    if parts[0] == "xtask" and suffix == ".rs":
        return True
    if parts[0] == "fuzz" and (suffix == ".rs" or value == "fuzz/Cargo.toml"):
        return True
    if value == "editors/vscode/package.json":
        return True
    if value.startswith("editors/vscode/src/test/") and suffix in {".ts", ".js"}:
        return True
    if value == "scripts/captures/package.json":
        return True
    if (
        value.startswith("scripts/captures/")
        and ".spec." in name
        and suffix in {".js", ".mjs", ".ts"}
    ):
        return True
    if _is_excluded_node_sentinel_path(parts, name, suffix):
        return True
    if value.startswith("conformance/cases/") and name == "manifest.toml":
        return True
    if (
        len(parts) == 2
        and parts[0] == "scripts"
        and "gate" in name
        and suffix in {".py", ".sh"}
    ):
        return True
    return (
        len(parts) == 3
        and parts[0] == ".github"
        and parts[1] == "workflows"
        and suffix in {".yml", ".yaml"}
    )


def _is_excluded_node_sentinel_path(
    parts: list[str],
    name: str,
    suffix: str,
) -> bool:
    if suffix not in {".js", ".mjs", ".cjs", ".ts"}:
        return False
    value = "/".join(parts)
    if value.startswith("editors/vscode/src/test/"):
        return False
    if value.startswith("scripts/captures/") and ".spec." in name:
        return False
    return (
        ".test." in name
        or ".spec." in name
        or ".e2e." in name
        or any(
            part in {"test", "tests", "__tests__", "spec", "specs"}
            for part in parts[:-1]
        )
    )
