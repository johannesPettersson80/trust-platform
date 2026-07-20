#!/usr/bin/env python3
"""Execute the exact local and remote checks for a frozen release candidate."""

from __future__ import annotations

import shlex
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Sequence

import release_candidate_guard as guard


def remote_vscode_command() -> str:
    return (
        "cd editors/vscode && npm run lint && npm run compile && "
        "TRUST_UI_TEST_BROWSER=/usr/bin/google-chrome "
        'xvfb-run -a -s "-screen 0 1920x1080x24" npm test'
    )


def tree_manifest(root: Path) -> dict[str, str]:
    files = [root / "AGENTS.md"]
    skills = root / ".codex" / "skills"
    if skills.is_dir():
        files.extend(path for path in skills.rglob("*") if path.is_file())
    manifest: dict[str, str] = {}
    for path in files:
        if not path.is_file() or "__pycache__" in path.parts or path.suffix == ".pyc":
            continue
        manifest[path.relative_to(root).as_posix()] = guard.sha256_bytes(path.read_bytes())
    return manifest


def bootstrap_failures(repo: Path, canonical_repo: Path) -> list[str]:
    expected = tree_manifest(canonical_repo)
    actual = tree_manifest(repo)
    return [
        f"agent bootstrap mismatch: {path}"
        for path in sorted(expected.keys() | actual.keys())
        if expected.get(path) != actual.get(path)
    ]


def command_record(
    command_id: str,
    command: Sequence[str],
    *,
    cwd: Path,
    scope: str,
    log_dir: Path,
    accepted_nonzero: Callable[[int, str], bool] | None = None,
) -> dict[str, Any]:
    start = time.monotonic()
    result = guard.run(command, cwd=cwd)
    duration_ms = round((time.monotonic() - start) * 1000)
    output = result.stdout.encode("utf-8", errors="replace")
    log_path = log_dir / f"{command_id}.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_bytes(output)
    accepted_advisory = (
        result.returncode != 0
        and accepted_nonzero is not None
        and accepted_nonzero(result.returncode, result.stdout)
    )
    record = {
        "id": command_id,
        "command": shlex.join(command),
        "exit_status": 0 if accepted_advisory else result.returncode,
        "output_sha256": guard.sha256_bytes(output),
        "duration_ms": duration_ms,
        "scope": scope,
    }
    if accepted_advisory:
        record["raw_exit_status"] = result.returncode
        record["disposition"] = "accepted_planner_advisory"
    return record


def planner_exit_is_advisory(exit_status: int, output: str) -> bool:
    if exit_status == 0:
        return False
    repo = guard.repo_root(Path.cwd())
    if str(repo) not in sys.path:
        sys.path.insert(0, str(repo))
    from scripts.verification.report_gate import parse_planner_json, planner_finding_blocks

    payload = parse_planner_json(output)
    return payload is not None and not planner_finding_blocks(payload)


def planner_command(
    *, python: str, intent: str, baseline: str, paths: Sequence[str]
) -> list[str]:
    return [
        python,
        "scripts/plan_tests.py",
        "--intent",
        intent,
        "--baseline",
        baseline,
        "--changed",
        *paths,
        "--format",
        "json",
    ]


def synthetic_record(command_id: str, command: str, failures: Sequence[str]) -> dict[str, Any]:
    output = "\n".join(failures).encode("utf-8")
    return {
        "id": command_id,
        "command": command,
        "exit_status": 1 if failures else 0,
        "output_sha256": guard.sha256_bytes(output),
        "duration_ms": 0,
        "scope": "local",
    }


def remote_command(host: str, worktree: str, command: str) -> list[str]:
    return ["ssh", host, f"cd {shlex.quote(worktree)} && {command}"]


def finish_artifact(
    repo: Path,
    *,
    head: str,
    base_ref: str,
    base_sha: str,
    vscode_changed: bool,
    records: list[dict[str, Any]],
    log_dir: Path,
) -> int:
    required = set(guard.required_command_ids(vscode_changed=vscode_changed))
    recorded_required = {row["id"] for row in records if row["id"] in required}
    passed = required == recorded_required and all(
        row["exit_status"] == 0 for row in records if row["id"] in required
    )
    artifact = {
        "schema_version": 1,
        "status": "pass" if passed else "fail",
        "head": head,
        "base_ref": base_ref,
        "base_sha": base_sha,
        "changed_paths_sha256": guard.changed_paths_sha256(repo, base_sha, head),
        "created_at": datetime.now(timezone.utc).isoformat(),
        "commands": records,
    }
    path = guard.artifact_path(repo, head)
    guard.write_json(path, artifact)
    print(path)
    if not passed:
        for row in records:
            if row["exit_status"] != 0:
                print(f"FAILED {row['id']}: {log_dir / (row['id'] + '.log')}", file=sys.stderr)
        return 1
    return 0


def stage_passed(records: Sequence[dict[str, Any]], command_ids: Sequence[str]) -> bool:
    selected = [row for row in records if row["id"] in command_ids]
    return len(selected) == len(command_ids) and all(row["exit_status"] == 0 for row in selected)


def prepare(args: Any) -> int:
    repo = guard.repo_root(Path(args.repo))
    canonical_repo = guard.repo_root(Path(args.canonical_repo))
    if args.fetch:
        fetch = guard.run(["git", "-C", str(repo), "fetch", "origin", "main"])
        if fetch.returncode != 0:
            print(fetch.stdout, file=sys.stderr)
            return 2
    head = guard.git(repo, "rev-parse", "HEAD").strip()
    base_sha = guard.git(repo, "rev-parse", args.base).strip()
    paths = guard.changed_paths(repo, base_sha, head)
    log_dir = guard.state_root(repo) / "logs" / head
    records: list[dict[str, Any]] = []

    bootstrap = bootstrap_failures(repo, canonical_repo)
    records.append(synthetic_record("bootstrap", "compare canonical AGENTS.md and skills", bootstrap))
    dirty = guard.git(repo, "status", "--porcelain=v1", "--untracked-files=all").splitlines()
    records.append(synthetic_record("clean", "git status --porcelain", dirty))
    ancestor = guard.run(["git", "-C", str(repo), "merge-base", "--is-ancestor", base_sha, head])
    records.append(
        synthetic_record(
            "base_ancestor",
            f"git merge-base --is-ancestor {base_sha} {head}",
            [] if ancestor.returncode == 0 else ["base is not an ancestor of candidate"],
        )
    )
    records.append(
        command_record(
            "diff_check",
            ["git", "diff", "--check", f"{base_sha}...{head}"],
            cwd=repo,
            scope="local",
            log_dir=log_dir,
        )
    )
    planner = planner_command(
        python=sys.executable,
        intent=args.intent,
        baseline=base_sha,
        paths=paths,
    )
    records.append(
        command_record(
            "planner",
            planner,
            cwd=repo,
            scope="local",
            log_dir=log_dir,
            accepted_nonzero=planner_exit_is_advisory,
        )
    )
    vscode_changed = any(
        path == "editors/vscode" or path.startswith("editors/vscode/") for path in paths
    )
    cheap_ids = ("bootstrap", "clean", "base_ancestor", "diff_check", "planner")
    if not stage_passed(records, cheap_ids):
        return finish_artifact(
            repo,
            head=head,
            base_ref=args.base,
            base_sha=base_sha,
            vscode_changed=vscode_changed,
            records=records,
            log_dir=log_dir,
        )

    remote_head_check = (
        f'test "$(git rev-parse HEAD)" = {shlex.quote(head)} && '
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"'
    )
    strict_command = shlex.join(
        [
            "python3",
            "scripts/verification_report_gate.py",
            "--base",
            base_sha,
            "--head",
            head,
            "--intent",
            args.intent,
            "--strict",
            "--out-dir",
            "target/gate-artifacts/verification-release-candidate",
        ]
    )
    remote_commands = [
        ("remote_exact_head", remote_head_check),
        ("catalog_staleness", "python3 scripts/check_test_catalog_staleness.py"),
        ("selftests", "python3 scripts/check_verification_tooling_selftests.py"),
        ("strict_gate", strict_command),
    ]
    if vscode_changed:
        remote_commands.append(("remote_vscode", remote_vscode_command()))
    remote_commands.extend(
        [
            ("remote_fmt", "just fmt"),
            ("remote_clippy", f"CARGO_TARGET_DIR={shlex.quote(args.remote_target)} just clippy"),
            ("remote_test_all", f"CARGO_TARGET_DIR={shlex.quote(args.remote_target)} just test-all"),
        ]
    )
    remote_commands.append(
        ("remote_clean_after", 'test -z "$(git status --porcelain=v1 --untracked-files=all)"')
    )
    for command_id, command in remote_commands:
        records.append(
            command_record(
                command_id,
                remote_command(args.remote_host, args.remote_worktree, command),
                cwd=repo,
                scope=args.remote_host,
                log_dir=log_dir,
            )
        )
        if records[-1]["exit_status"] != 0:
            break

    return finish_artifact(
        repo,
        head=head,
        base_ref=args.base,
        base_sha=base_sha,
        vscode_changed=vscode_changed,
        records=records,
        log_dir=log_dir,
    )
