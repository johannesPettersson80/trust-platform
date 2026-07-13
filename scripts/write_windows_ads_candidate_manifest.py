#!/usr/bin/env python3
"""Write the immutable sidecar for a retained Windows ADS CI candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REPOSITORY_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_manifest(
    vsix: Path,
    candidate_commit_sha: str,
    expected_version: str,
    *,
    repository: str,
    workflow_run_id: int,
    workflow_run_attempt: int,
    workflow_run_head_sha: str,
    workflow_event: str,
) -> dict[str, object]:
    candidate = candidate_commit_sha.lower()
    run_head = workflow_run_head_sha.lower()
    if COMMIT_RE.fullmatch(candidate) is None:
        raise ValueError("candidate commit SHA must contain exactly 40 hex characters")
    if COMMIT_RE.fullmatch(run_head) is None:
        raise ValueError("workflow run head SHA must contain exactly 40 hex characters")
    if REPOSITORY_RE.fullmatch(repository) is None:
        raise ValueError("repository must use owner/name form")
    if workflow_run_id <= 0 or workflow_run_attempt <= 0:
        raise ValueError("workflow run id and attempt must be positive integers")
    if workflow_event not in {"push", "pull_request"}:
        raise ValueError("candidate workflow event must be push or pull_request")
    if candidate != run_head:
        raise ValueError("candidate commit must equal the workflow run head SHA")
    if not vsix.is_file() or vsix.stat().st_size <= 0:
        raise ValueError(f"candidate VSIX is missing or empty: {vsix}")
    return {
        "schema_version": 2,
        "artifact_kind": "windows_ads_msvc_candidate",
        "artifact_name": f"windows-ads-msvc-candidate-{candidate}",
        "candidate_commit_sha": candidate,
        "version": expected_version,
        "target_platform": "win32-x64",
        "vsix_filename": vsix.name,
        "vsix_sha256": sha256(vsix),
        "vsix_size_bytes": vsix.stat().st_size,
        "workflow_provenance": {
            "repository": repository,
            "workflow_path": ".github/workflows/ci.yml",
            "workflow_run_id": workflow_run_id,
            "workflow_run_attempt": workflow_run_attempt,
            "workflow_run_head_sha": run_head,
            "workflow_event": workflow_event,
            "candidate_source": (
                "pull_request_head" if workflow_event == "pull_request" else "workflow_head"
            ),
            "job_name": "Windows Packaged Simulator + Native ADS/TcAdsDll Contract",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vsix", type=Path, required=True)
    parser.add_argument("--candidate-commit-sha", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--workflow-run-id", type=int, required=True)
    parser.add_argument("--workflow-run-attempt", type=int, required=True)
    parser.add_argument("--workflow-run-head-sha", required=True)
    parser.add_argument("--workflow-event", required=True)
    parser.add_argument("--cargo-toml", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        with args.cargo_toml.open("rb") as handle:
            version = str(tomllib.load(handle)["workspace"]["package"]["version"])
        manifest = build_manifest(
            args.vsix,
            args.candidate_commit_sha,
            version,
            repository=args.repository,
            workflow_run_id=args.workflow_run_id,
            workflow_run_attempt=args.workflow_run_attempt,
            workflow_run_head_sha=args.workflow_run_head_sha,
            workflow_event=args.workflow_event,
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    except (OSError, KeyError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"Windows ADS candidate manifest failed: {error}", file=sys.stderr)
        return 1
    print(
        "Windows ADS candidate manifest: OK "
        f"({manifest['candidate_commit_sha']} {manifest['vsix_sha256']})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
