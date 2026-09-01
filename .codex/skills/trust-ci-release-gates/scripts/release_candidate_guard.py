#!/usr/bin/env python3
"""Exact-SHA release-candidate preparation, push, merge, and release guard."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Sequence


ZERO_SHA = "0" * 40
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
ACTION_JOB_RE = re.compile(r"/actions/runs/(\d+)/job/(\d+)")
PENDING_STATES = {"EXPECTED", "IN_PROGRESS", "PENDING", "QUEUED", "REQUESTED", "WAITING"}
PASS_STATES = {"SUCCESS", "NEUTRAL", "SKIPPED"}
FAIL_STATES = {
    "ACTION_REQUIRED",
    "CANCELLED",
    "FAILURE",
    "STALE",
    "STARTUP_FAILURE",
    "TIMED_OUT",
}
BASE_REQUIRED_COMMANDS = (
    "bootstrap",
    "clean",
    "base_ancestor",
    "diff_check",
    "strict_gate",
    "remote_exact_head",
    "remote_disk_preflight",
    "remote_fmt",
    "remote_cross_target_warnings",
    "remote_supply_chain",
    "remote_architecture_safety",
    "remote_clippy",
    "remote_reclaim_before_test_all",
    "remote_test_all",
    "remote_clean_after",
)
MARKETPLACE_TARGETS = (
    "darwin-arm64",
    "darwin-x64",
    "linux-arm64",
    "linux-x64",
    "win32-x64",
)


def run(
    command: Sequence[str],
    *,
    cwd: Path | None = None,
    check: bool = False,
    input_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(command),
        cwd=cwd,
        check=check,
        input=input_text,
        text=True,
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


def artifact_path(repo: Path, head: str) -> Path:
    return state_root(repo) / "artifacts" / f"{head}.json"


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


def load_artifact(repo: Path, head: str) -> dict[str, Any] | None:
    path = artifact_path(repo, head)
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def changed_paths(repo: Path, base: str, head: str) -> list[str]:
    raw = git(repo, "diff", "--name-only", "-z", f"{base}...{head}")
    return sorted(path for path in raw.split("\0") if path)


def changed_paths_sha256(repo: Path, base: str, head: str) -> str:
    return sha256_bytes(canonical_json_bytes(changed_paths(repo, base, head)))


def required_command_ids(*, vscode_changed: bool) -> tuple[str, ...]:
    if vscode_changed:
        return BASE_REQUIRED_COMMANDS + (
            "remote_docs_capture_lifecycle",
            "remote_vscode",
        )
    return BASE_REQUIRED_COMMANDS


def command_state(check: dict[str, Any]) -> str:
    conclusion = check.get("conclusion")
    status = check.get("status")
    state = check.get("state")
    return str(conclusion or state or status or "UNKNOWN").upper()


def validate_artifact(repo: Path, artifact: dict[str, Any], head: str) -> list[str]:
    failures: list[str] = []
    if artifact.get("schema_version") != 1:
        failures.append("release-candidate artifact must use schema_version 1")
    if artifact.get("status") != "pass":
        failures.append("release-candidate artifact status must be pass")
    if artifact.get("head") != head:
        failures.append(f"artifact head {artifact.get('head')!r} does not match pushed head {head}")
    base_ref = artifact.get("base_ref")
    base_sha = artifact.get("base_sha")
    if not isinstance(base_ref, str) or not base_ref:
        failures.append("artifact base_ref is missing")
    elif not isinstance(base_sha, str) or not SHA_RE.fullmatch(base_sha):
        failures.append("artifact base_sha must be a full Git SHA")
    else:
        current_base = git(repo, "rev-parse", base_ref, check=False).strip()
        if current_base != base_sha:
            failures.append(
                f"artifact base {base_sha} is stale; {base_ref} currently resolves to {current_base or 'nothing'}"
            )
        elif SHA_RE.fullmatch(head):
            expected_changed = changed_paths_sha256(repo, base_sha, head)
            if artifact.get("changed_paths_sha256") != expected_changed:
                failures.append("artifact changed-path digest does not match the exact base/head diff")

    commands = artifact.get("commands")
    if not isinstance(commands, list):
        failures.append("artifact commands must be a list")
        return failures
    by_id: dict[str, dict[str, Any]] = {}
    for row in commands:
        if not isinstance(row, dict) or not isinstance(row.get("id"), str):
            failures.append("artifact contains an invalid command record")
            continue
        command_id = row["id"]
        if command_id in by_id:
            failures.append(f"artifact duplicates command {command_id}")
        by_id[command_id] = row

    vscode_changed = False
    base_exists = (
        isinstance(base_sha, str)
        and SHA_RE.fullmatch(base_sha) is not None
        and git(repo, "cat-file", "-e", f"{base_sha}^{{commit}}", check=False) == ""
    )
    head_exists = (
        SHA_RE.fullmatch(head) is not None
        and git(repo, "cat-file", "-e", f"{head}^{{commit}}", check=False) == ""
    )
    if base_exists and head_exists:
        vscode_changed = any(
            path == "editors/vscode" or path.startswith("editors/vscode/")
            for path in changed_paths(repo, base_sha, head)
        )
    for command_id in required_command_ids(vscode_changed=vscode_changed):
        row = by_id.get(command_id)
        if row is None:
            failures.append(f"artifact is missing required command {command_id}")
        elif row.get("exit_status") != 0:
            failures.append(f"artifact command {command_id} did not pass")
    return failures


def workspace_version_at(repo: Path, revision: str) -> str | None:
    text = git(repo, "show", f"{revision}:Cargo.toml", check=False)
    match = re.search(r"(?ms)^\[workspace\.package\].*?^version\s*=\s*\"([^\"]+)\"", text)
    return match.group(1) if match else None


def release_sensitive_push(repo: Path, local_ref: str, local_oid: str, remote_ref: str) -> bool:
    if remote_ref in {"refs/heads/main", "refs/heads/master"}:
        return True
    branch = remote_ref.removeprefix("refs/heads/")
    if branch.startswith(("integrate/", "release/")):
        return True
    if remote_ref.startswith("refs/tags/v"):
        return True
    if not SHA_RE.fullmatch(local_oid):
        return False
    base_version = workspace_version_at(repo, "origin/main")
    pushed_version = workspace_version_at(repo, local_oid)
    return base_version is None or pushed_version is None or base_version != pushed_version


def validate_tag_push(repo: Path, local_ref: str, local_oid: str) -> list[str]:
    failures: list[str] = []
    tag = local_ref.removeprefix("refs/tags/")
    if git(repo, "cat-file", "-t", local_ref, check=False).strip() != "tag":
        failures.append(f"release tag {tag} must be annotated")
        return failures
    commit = git(repo, "rev-parse", f"{local_ref}^{{commit}}", check=False).strip()
    main = git(repo, "rev-parse", "origin/main", check=False).strip()
    if commit != main:
        failures.append(f"release tag {tag} must point at current origin/main {main}, found {commit}")
    version = workspace_version_at(repo, commit) if SHA_RE.fullmatch(commit) else None
    if tag != f"v{version}":
        failures.append(f"release tag {tag} does not match workspace version {version}")
    return failures


def validate_push_candidate(
    repo: Path,
    *,
    local_ref: str,
    local_oid: str,
    remote_ref: str,
    artifact: dict[str, Any] | None,
) -> list[str]:
    if local_oid == ZERO_SHA:
        return []
    if remote_ref.startswith("refs/tags/v"):
        return validate_tag_push(repo, local_ref, local_oid)
    if not release_sensitive_push(repo, local_ref, local_oid, remote_ref):
        return []
    if artifact is None:
        return [
            f"missing exact-SHA release-candidate artifact for {local_oid}; run release_candidate_guard.py prepare"
        ]
    return validate_artifact(repo, artifact, local_oid)


def check_push_lines(repo: Path, lines: Iterable[str]) -> list[str]:
    failures: list[str] = []
    for line in lines:
        fields = line.split()
        if len(fields) != 4:
            failures.append(f"invalid pre-push input line: {line!r}")
            continue
        local_ref, local_oid, remote_ref, _remote_oid = fields
        artifact = load_artifact(repo, local_oid)
        failures.extend(
            validate_push_candidate(
                repo,
                local_ref=local_ref,
                local_oid=local_oid,
                remote_ref=remote_ref,
                artifact=artifact,
            )
        )
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


def validate_merge_state(pr: dict[str, Any], expected_head: str) -> list[str]:
    failures: list[str] = []
    if pr.get("headRefOid") != expected_head:
        failures.append(
            f"PR head {pr.get('headRefOid')} does not match validated head {expected_head}"
        )
    if pr.get("mergeStateStatus") != "CLEAN":
        failures.append(f"PR merge state must be CLEAN, found {pr.get('mergeStateStatus')}")
    checks = pr.get("statusCheckRollup")
    if not isinstance(checks, list) or not checks:
        failures.append("PR has no completed status checks")
        return failures
    for check in checks:
        state = command_state(check)
        if state not in PASS_STATES:
            failures.append(f"required candidate check {check.get('name', 'unnamed')} is {state}")
    return failures


def validate_release_state(
    state: dict[str, Any], version: str, marketplace_targets: Sequence[str]
) -> list[str]:
    failures: list[str] = []
    requirements = {
        "main_sha_matches": "release tag does not match the final main SHA",
        "annotated_tag_matches": "annotated tag does not match the workspace version",
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
        if not any(command_state(row) in PENDING_STATES for row in checks):
            break
        if not args.wait or time.monotonic() >= deadline:
            pending = [str(row.get("name")) for row in checks if command_state(row) in PENDING_STATES]
            print(f"checks are still pending: {', '.join(sorted(pending))}", file=sys.stderr)
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


def check_merge(args: argparse.Namespace) -> int:
    repo = repo_root(Path(args.repo))
    head = git(repo, "rev-parse", "HEAD").strip()
    artifact = load_artifact(repo, head)
    failures = (
        ["missing exact-SHA release-candidate artifact"]
        if artifact is None
        else validate_artifact(repo, artifact, head)
    )
    result = run(
        [
            "gh",
            "pr",
            "view",
            str(args.pr),
            "--json",
            "headRefOid,mergeStateStatus,statusCheckRollup,state,url",
        ],
        cwd=repo,
    )
    if result.returncode != 0:
        print(result.stdout, file=sys.stderr)
        return 2
    pr = json.loads(result.stdout)
    failures.extend(validate_merge_state(pr, head))
    if failures:
        print("\n".join(f"BLOCKED: {failure}" for failure in failures), file=sys.stderr)
        return 1
    if args.execute:
        merge = run(
            [
                "gh",
                "pr",
                "merge",
                str(args.pr),
                "--merge",
                "--match-head-commit",
                head,
            ],
            cwd=repo,
        )
        print(merge.stdout, end="")
        return merge.returncode
    print(f"PR {args.pr} is eligible to merge at exact head {head}")
    return 0



def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--repo", default=".")
    subparsers = result.add_subparsers(dest="command", required=True)

    prepare_parser = subparsers.add_parser("prepare")
    prepare_parser.add_argument("--base", default="origin/main")
    prepare_parser.add_argument(
        "--intent", choices=("bugfix", "docs", "feature", "refactor", "test-refactor"), default="bugfix"
    )
    prepare_parser.add_argument(
        "--canonical-repo",
        default=os.environ.get("TRUST_CANONICAL_REPO", "/home/johannes/projects/trust-platform"),
    )
    prepare_parser.add_argument("--remote-host", default="trust-builder")
    prepare_parser.add_argument("--remote-worktree", required=True)
    prepare_parser.add_argument(
        "--remote-target",
        default="/home/johannes/.cache/codex-targets/trust-platform-gate",
    )
    prepare_parser.add_argument("--fetch", action=argparse.BooleanOptionalAction, default=True)
    prepare_parser.set_defaults(handler=prepare_command)

    push_parser = subparsers.add_parser("check-push")
    push_parser.set_defaults(handler=lambda args: check_push_command(args))

    collect_parser = subparsers.add_parser("collect-failures")
    collect_parser.add_argument("--pr", required=True)
    collect_parser.add_argument("--wait", action="store_true")
    collect_parser.add_argument("--timeout", type=int, default=7200)
    collect_parser.add_argument("--interval", type=int, default=30)
    collect_parser.set_defaults(handler=collect_failures)

    merge_parser = subparsers.add_parser("check-merge")
    merge_parser.add_argument("--pr", required=True)
    merge_parser.add_argument("--execute", action="store_true")
    merge_parser.set_defaults(handler=check_merge)

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


def prepare_command(args: argparse.Namespace) -> int:
    from release_candidate_prepare import prepare

    return prepare(args)


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
    repo = repo_root(Path(args.repo))
    failures = check_push_lines(repo, sys.stdin.read().splitlines())
    if failures:
        print("Release-candidate push blocked:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    return int(args.handler(args))


if __name__ == "__main__":
    raise SystemExit(main())
