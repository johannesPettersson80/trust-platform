#!/usr/bin/env python3
"""Generate canonical release artifact provenance."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Iterable


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
TAG_RE = re.compile(r"^v\d+\.\d+\.\d+$")
PLATFORMS = ("linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _artifact_identity(path: Path) -> tuple[str, str]:
    name = path.name
    platform = next((value for value in PLATFORMS if value in name), "all")
    if name.endswith(".vsix"):
        kind = "vsix"
    elif name.startswith("trust-runtime-"):
        kind = "runtime"
    elif name.startswith("trust-lsp-"):
        kind = "language_server"
    elif name == "conformance-status.json":
        kind = "conformance_json"
    elif name == "conformance-status.md":
        kind = "conformance_markdown"
    else:
        raise ValueError(f"unreviewed release artifact {name!r}")
    return kind, platform


def build_release_provenance(
    *,
    files: Iterable[Path],
    tag: str,
    commit: str,
    workflow_run_id: str,
    workflow_run_url: str,
    timestamp: str,
) -> dict:
    if not TAG_RE.fullmatch(tag):
        raise ValueError("tag must use vMAJOR.MINOR.PATCH")
    if not COMMIT_RE.fullmatch(commit):
        raise ValueError("commit must be a clean full Git SHA")
    parsed_time = datetime.fromisoformat(timestamp)
    if parsed_time.tzinfo is None:
        raise ValueError("timestamp must carry a timezone")
    if not workflow_run_id.strip():
        raise ValueError("workflow_run_id is required")
    if not workflow_run_url.startswith("https://github.com/"):
        raise ValueError("workflow_run_url must be a GitHub HTTPS URL")

    rows = []
    seen: set[str] = set()
    for path in sorted((Path(path) for path in files), key=lambda item: item.name):
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"artifact must be a regular non-symlink file: {path}")
        if path.name in seen:
            raise ValueError(f"duplicate release artifact basename {path.name}")
        seen.add(path.name)
        kind, platform = _artifact_identity(path)
        rows.append(
            {
                "kind": kind,
                "path": path.name,
                "platform": platform,
                "sha256": _sha256(path),
            }
        )
    if not rows:
        raise ValueError("release provenance requires artifacts")
    return {
        "schema_version": 1,
        "tag": tag,
        "commit": commit,
        "workflow_run_id": workflow_run_id,
        "workflow_run_url": workflow_run_url,
        "timestamp": timestamp,
        "artifacts": rows,
    }


def _collect_artifacts(roots: list[Path]) -> list[Path]:
    files: list[Path] = []
    for root in roots:
        if not root.is_dir() or root.is_symlink():
            raise ValueError(f"artifact root must be a regular directory: {root}")
        files.extend(
            path
            for path in root.rglob("*")
            if path.is_file()
            and (
                path.name.endswith((".vsix", ".tar.gz", ".zip"))
                or path.name in {"conformance-status.json", "conformance-status.md"}
            )
        )
    return files


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-root", action="append", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--workflow-run-id", required=True)
    parser.add_argument("--workflow-run-url", required=True)
    parser.add_argument("--timestamp", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        payload = build_release_provenance(
            files=_collect_artifacts(args.artifact_root),
            tag=args.tag,
            commit=args.commit,
            workflow_run_id=args.workflow_run_id,
            workflow_run_url=args.workflow_run_url,
            timestamp=args.timestamp,
        )
        args.output.write_text(
            json.dumps(payload, sort_keys=True, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    except (OSError, ValueError) as exc:
        print(f"release-provenance: FAIL: {exc}", file=sys.stderr)
        return 1
    print(f"release-provenance: wrote {len(payload['artifacts'])} artifacts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
