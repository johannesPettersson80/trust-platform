#!/usr/bin/env python3
"""The workflow's pre-push check, a pull request's failure logs, and release verification and its cleanup audit."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Sequence


SHA_RE = re.compile(r"^[0-9a-f]{40}$")
ACTION_JOB_RE = re.compile(r"/actions/runs/(\d+)/job/(\d+)")
PENDING_STATES = {"EXPECTED", "IN_PROGRESS", "PENDING", "QUEUED", "REQUESTED", "WAITING"}
FAIL_STATES = {
    "ACTION_REQUIRED",
    "CANCELLED",
    "FAILURE",
    "STALE",
    "STARTUP_FAILURE",
    "TIMED_OUT",
}


def run(
    command: Sequence[str],
    *,
    cwd: Path | None = None,
    check: bool = False,
    input_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    # A command's output is read as UTF-8 with any broken sequence replaced: two processes that
    # write to the same pipe can split a character, and a log must never stop the guard.
    return subprocess.run(
        list(command),
        cwd=cwd,
        check=check,
        input=input_text,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def git(repo: Path, *args: str, check: bool = True) -> str:
    result = run(["git", "-C", str(repo), *args])
    if check and result.returncode != 0:
        raise RuntimeError(result.stdout.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def repo_root(path: Path) -> Path:
    return Path(git(path, "rev-parse", "--show-toplevel").strip()).resolve()


def common_git_dir(repo: Path) -> Path:
    raw = Path(git(repo, "rev-parse", "--git-common-dir").strip())
    return (repo / raw).resolve() if not raw.is_absolute() else raw.resolve()


def state_root(repo: Path) -> Path:
    return common_git_dir(repo) / "trust-release-candidates"


def failure_ledger_path(repo: Path, head: str) -> Path:
    return state_root(repo) / "failures" / f"{head}.json"


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(canonical_json_bytes(value))
    temporary.replace(path)


def command_state(check: dict[str, Any]) -> str:
    conclusion = check.get("conclusion")
    status = check.get("status")
    state = check.get("state")
    return str(conclusion or state or status or "UNKNOWN").upper()


def workspace_version_at(repo: Path, revision: str) -> str | None:
    text = git(repo, "show", f"{revision}:Cargo.toml", check=False)
    match = re.search(r"(?ms)^\[workspace\.package\].*?^version\s*=\s*\"([^\"]+)\"", text)
    return match.group(1) if match else None


def validate_push(*, remote_ref: str) -> list[str]:
    """The pushes the workflow refuses (specification 22, "Change and release workflow"): `main` changes only through a
    merged pull request, and a release tag only through the Release workflow, so neither is pushed nor deleted from here.
    Every other push goes through; CI is the gate."""
    if remote_ref in {"refs/heads/main", "refs/heads/master"}:
        branch = remote_ref.removeprefix("refs/heads/")
        return [f"{branch} changes only through a merged pull request; push a branch and open one"]
    if remote_ref.startswith("refs/tags/v"):
        tag = remote_ref.removeprefix("refs/tags/")
        return [f"the release tag {tag} is made by the Release workflow after main's CI; never push it"]
    return []


def check_push_lines(lines: Iterable[str]) -> list[str]:
    failures: list[str] = []
    for line in lines:
        fields = line.split()
        if len(fields) != 4:
            failures.append(f"invalid pre-push input line: {line!r}")
            continue
        _local_ref, _local_oid, remote_ref, _remote_oid = fields
        failures.extend(validate_push(remote_ref=remote_ref))
    return failures


def build_failure_ledger(
    head: str,
    checks: list[dict[str, Any]],
    logs: dict[str, dict[str, str]],
) -> dict[str, Any]:
    pending = [row.get("name", "unnamed") for row in checks if command_state(row) in PENDING_STATES]
    if pending:
        raise ValueError(f"checks are still pending: {', '.join(sorted(map(str, pending)))}")
    failed = [row for row in checks if command_state(row) in FAIL_STATES]
    rows: list[dict[str, Any]] = []
    for check in failed:
        name = str(check.get("name", "unnamed"))
        log = logs.get(name)
        if not log:
            raise ValueError(f"missing failure log for {name}")
        rows.append(
            {
                "name": name,
                "state": command_state(check),
                "details_url": check.get("detailsUrl"),
                "log_path": log["path"],
                "log_sha256": log["sha256"],
            }
        )
    return {
        "schema_version": 1,
        "head": head,
        "complete": True,
        "collected_at": datetime.now(timezone.utc).isoformat(),
        "check_count": len(checks),
        "failed": sorted(rows, key=lambda row: row["name"]),
    }


def validate_release_state(
    state: dict[str, Any], version: str, marketplace_targets: Sequence[str]
) -> list[str]:
    failures: list[str] = []
    requirements = {
        "tag_on_main": "release tag is not on main",
        "candidate_released": "the release does not contain the merged candidate",
        "annotated_tag_matches": "release tag is not an annotated tag of the workspace version",
        "release_workflow_success": "Release workflow has not succeeded",
        "github_release_published": "GitHub release is not published",
        "github_release_latest": "GitHub release is not marked Latest",
        "assets_verified": "release assets and published checksums are not verified",
    }
    for key, message in requirements.items():
        if state.get(key) is not True:
            failures.append(message)
    versions = state.get("marketplace_versions")
    if not isinstance(versions, dict):
        failures.append("VS Code Marketplace target versions were not collected")
    else:
        for target in marketplace_targets:
            if versions.get(target) != version:
                failures.append(
                    f"VS Code Marketplace target {target} is {versions.get(target)!r}, expected {version}"
                )
    return failures


def collect_failures(args: argparse.Namespace) -> int:
    repo = repo_root(Path(args.repo))
    deadline = time.monotonic() + args.timeout
    while True:
        result = run(
            [
                "gh",
                "pr",
                "view",
                str(args.pr),
                "--json",
                "headRefOid,statusCheckRollup,url",
            ],
            cwd=repo,
        )
        if result.returncode != 0:
            print(result.stdout, file=sys.stderr)
            return 2
        pr = json.loads(result.stdout)
        checks = pr.get("statusCheckRollup") or []
        pending = [str(row.get("name")) for row in checks if command_state(row) in PENDING_STATES]
        # Right after a push GitHub may not have registered any check for the new head yet;
        # an empty rollup is not a completed run.
        if checks and not pending:
            break
        if not args.wait or time.monotonic() >= deadline:
            if checks:
                print(f"checks are still pending: {', '.join(sorted(pending))}", file=sys.stderr)
            else:
                print("no status checks are registered for the PR head yet", file=sys.stderr)
            return 3
        time.sleep(args.interval)

    head = pr["headRefOid"]
    log_dir = state_root(repo) / "failures" / head / "logs"
    logs: dict[str, dict[str, str]] = {}
    for check in checks:
        if command_state(check) not in FAIL_STATES:
            continue
        name = str(check.get("name", "unnamed"))
        details = str(check.get("detailsUrl") or "")
        match = ACTION_JOB_RE.search(details)
        if not match:
            print(f"cannot locate Actions job log for failed check {name}: {details}", file=sys.stderr)
            return 4
        run_id, job_id = match.groups()
        log = run(["gh", "run", "view", run_id, "--job", job_id, "--log"], cwd=repo)
        if log.returncode != 0:
            print(log.stdout, file=sys.stderr)
            return 4
        safe_name = re.sub(r"[^A-Za-z0-9_.-]+", "_", name).strip("_") or "unnamed"
        path = log_dir / f"{safe_name}.log"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(log.stdout, encoding="utf-8")
        logs[name] = {"path": str(path), "sha256": sha256_bytes(log.stdout.encode("utf-8"))}
    try:
        ledger = build_failure_ledger(head, checks, logs)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 5
    path = failure_ledger_path(repo, head)
    write_json(path, ledger)
    print(path)
    return 1 if ledger["failed"] else 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--repo", default=".")
    subparsers = result.add_subparsers(dest="command", required=True)

    push_parser = subparsers.add_parser("check-push")
    push_parser.set_defaults(handler=lambda args: check_push_command(args))

    collect_parser = subparsers.add_parser("collect-failures")
    collect_parser.add_argument("--pr", required=True)
    collect_parser.add_argument("--wait", action="store_true")
    collect_parser.add_argument("--timeout", type=int, default=7200)
    collect_parser.add_argument("--interval", type=int, default=30)
    collect_parser.set_defaults(handler=collect_failures)

    release_parser = subparsers.add_parser("verify-release")
    release_parser.add_argument("--candidate-head", required=True)
    release_parser.add_argument("--branch", required=True)
    release_parser.add_argument("--main-ref", default="origin/main")
    release_parser.set_defaults(handler=verify_release_command)

    cleanup_parser = subparsers.add_parser("audit-post-merge")
    cleanup_parser.add_argument("--candidate-head", required=True)
    cleanup_parser.add_argument("--branch", required=True)
    cleanup_parser.add_argument("--main-ref", default="origin/main")
    cleanup_parser.set_defaults(handler=audit_post_merge_command)
    return result


def verify_release_command(args: argparse.Namespace) -> int:
    from release_candidate_release import verify_release

    release_result = verify_release(args)
    if release_result != 0:
        return release_result
    from release_candidate_cleanup import audit_post_merge

    return audit_post_merge(args)


def audit_post_merge_command(args: argparse.Namespace) -> int:
    from release_candidate_cleanup import audit_post_merge

    return audit_post_merge(args)


def check_push_command(args: argparse.Namespace) -> int:
    failures = check_push_lines(sys.stdin.read().splitlines())
    if failures:
        print("Push blocked:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    return int(args.handler(args))


if __name__ == "__main__":
    raise SystemExit(main())
