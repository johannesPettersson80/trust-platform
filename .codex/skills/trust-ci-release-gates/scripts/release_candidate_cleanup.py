#!/usr/bin/env python3
"""Audit release-candidate state that should be removed after a merge."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import release_candidate_guard as guard


def worktree_rows(repo: Path) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for line in guard.git(repo, "worktree", "list", "--porcelain").splitlines():
        if not line:
            if current:
                rows.append(current)
                current = {}
            continue
        key, _, value = line.partition(" ")
        current[key] = value
    if current:
        rows.append(current)
    return rows


def ref_head(repo: Path, ref: str) -> str | None:
    value = guard.git(repo, "rev-parse", "--verify", ref, check=False).strip()
    return value if guard.SHA_RE.fullmatch(value) else None


def audit_post_merge(args: Any) -> int:
    repo = guard.repo_root(Path(args.repo))
    candidate = args.candidate_head
    failures: list[str] = []
    fetch = guard.run(["git", "-C", str(repo), "fetch", "origin", "--prune"])
    if fetch.returncode != 0:
        failures.append(f"cannot refresh origin before cleanup audit: {fetch.stdout.strip()}")
    if guard.SHA_RE.fullmatch(candidate) is None:
        failures.append("candidate head must be a full Git SHA")
    elif guard.git(repo, "cat-file", "-e", f"{candidate}^{{commit}}", check=False) != "":
        failures.append(f"candidate head {candidate} is not available")
    elif guard.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", candidate, args.main_ref]
    ).returncode != 0:
        failures.append(f"candidate head {candidate} is not contained by {args.main_ref}")

    targets: list[dict[str, str]] = []
    branch_valid = (
        guard.run(["git", "check-ref-format", "--branch", args.branch]).returncode == 0
        and args.branch not in {"main", "master"}
    )
    if not branch_valid:
        failures.append(f"candidate branch {args.branch!r} is invalid or protected")
    else:
        local_ref = f"refs/heads/{args.branch}"
        remote_ref = f"refs/remotes/origin/{args.branch}"
        for kind, ref in (("local_branch", local_ref), ("remote_branch", remote_ref)):
            head = ref_head(repo, ref)
            if head is None:
                continue
            if head != candidate:
                failures.append(f"{ref} points at {head}, not candidate head {candidate}")
            else:
                targets.append({"kind": kind, "target": ref})

        for row in worktree_rows(repo):
            head = row.get("HEAD")
            branch = row.get("branch")
            is_candidate = branch == local_ref or ("detached" in row and head == candidate)
            if not is_candidate:
                continue
            path = Path(row["worktree"]).resolve()
            if head != candidate:
                failures.append(f"candidate worktree {path} is at {head}, not {candidate}")
            elif guard.git(path, "status", "--porcelain", check=False).strip():
                failures.append(f"candidate worktree {path} is dirty")
            else:
                targets.append({"kind": "worktree", "target": str(path)})

    if failures:
        targets = []
        status = "blocked"
        return_code = 1
    elif targets:
        status = "cleanup_required"
        return_code = 2
    else:
        status = "clean"
        return_code = 0
    print(
        json.dumps(
            {
                "branch": args.branch,
                "candidate_head": candidate,
                "cleanup_targets": targets,
                "failures": failures,
                "main_ref": args.main_ref,
                "status": status,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return return_code
