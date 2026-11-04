#!/usr/bin/env python3
"""Generate canonical release artifact provenance (specification 24, "Artifact And Version Provenance").

The record names the released commit and its tree, the workflow run that released it, and whether its packages were
promoted from a rehearsal ("Release after main CI"). A promoted release passes the rehearsal's own record, already
checked against the rehearsal's SHA256SUMS: the new record then names that rehearsal's run and commit, and lists exactly
the artifacts, with the same digests, that the rehearsal built.
"""

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
    elif name.startswith("trust-toolchain-"):
        kind = "toolchain_bundle"
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
    tree: str,
    workflow_run_id: str,
    workflow_run_url: str,
    timestamp: str,
    rehearsal: dict | None = None,
) -> dict:
    """The release's provenance record. `rehearsal` is the record of the rehearsal whose files a promoted release
    publishes; None for a release whose own run built its packages, and for the rehearsal itself."""
    if not TAG_RE.fullmatch(tag):
        raise ValueError("tag must use vMAJOR.MINOR.PATCH")
    if not COMMIT_RE.fullmatch(commit):
        raise ValueError("commit must be a clean full Git SHA")
    if not COMMIT_RE.fullmatch(tree):
        raise ValueError("tree must be the commit's full Git tree id")
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
        "schema_version": 2,
        "tag": tag,
        "commit": commit,
        "tree": tree,
        "workflow_run_id": workflow_run_id,
        "workflow_run_url": workflow_run_url,
        "promoted": rehearsal is not None,
        "rehearsal": None if rehearsal is None else _promoted_from(rehearsal, tag=tag, tree=tree, rows=rows),
        "timestamp": timestamp,
        "artifacts": rows,
    }


def _promoted_from(record: dict, *, tag: str, tree: str, rows: list[dict]) -> dict:
    """The rehearsal run and commit that built a promoted release's artifacts, from the rehearsal's own record. The
    rehearsal built them from a commit with the released commit's tree, for the same tag, and the release publishes
    exactly the artifacts it recorded, byte for byte."""
    if record.get("promoted") is not False:
        raise ValueError("a promoted release takes the files of a rehearsal that built them, not of another promotion")
    if record.get("tag") != tag:
        raise ValueError(f"the rehearsal recorded {record.get('tag')!r}, not the release's tag {tag}")
    if record.get("tree") != tree:
        raise ValueError(f"the rehearsal built tree {record.get('tree')!r}, not the released commit's tree {tree}")
    if record.get("artifacts") != rows:
        raise ValueError("the promoted artifacts differ from those the rehearsal recorded")
    run_id = str(record.get("workflow_run_id") or "").strip()
    run_url = str(record.get("workflow_run_url") or "")
    commit = str(record.get("commit") or "")
    if not run_id.isdigit() or not run_url.startswith("https://github.com/") or not COMMIT_RE.fullmatch(commit):
        raise ValueError("the rehearsal's record must name its workflow run, its URL and its full commit")
    return {"workflow_run_id": run_id, "workflow_run_url": run_url, "commit": commit}


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
    parser.add_argument("--tree", required=True, help="The commit's tree (git rev-parse <commit>^{tree}).")
    parser.add_argument("--workflow-run-id", required=True)
    parser.add_argument("--workflow-run-url", required=True)
    parser.add_argument("--timestamp", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--rehearsal-provenance",
        default="",
        help="For a promoted release, the record of the rehearsal whose files it publishes; empty otherwise.",
    )
    args = parser.parse_args()
    try:
        rehearsal = None
        if args.rehearsal_provenance:
            rehearsal = json.loads(Path(args.rehearsal_provenance).read_text(encoding="utf-8"))
            if not isinstance(rehearsal, dict):
                raise ValueError("the rehearsal's provenance must be a JSON object")
        payload = build_release_provenance(
            files=_collect_artifacts(args.artifact_root),
            tag=args.tag,
            commit=args.commit,
            tree=args.tree,
            workflow_run_id=args.workflow_run_id,
            workflow_run_url=args.workflow_run_url,
            timestamp=args.timestamp,
            rehearsal=rehearsal,
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
