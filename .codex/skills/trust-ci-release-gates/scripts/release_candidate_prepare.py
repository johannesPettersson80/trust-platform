#!/usr/bin/env python3
"""Execute the exact local and remote checks for a frozen release candidate."""

from __future__ import annotations

import shlex
import sys
import time
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Sequence

import release_candidate_guard as guard


ADVISORY_COMMAND_IDS = frozenset({"planner", "catalog_staleness", "selftests"})


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
) -> dict[str, Any]:
    start = time.monotonic()
    result = guard.run(command, cwd=cwd)
    duration_ms = round((time.monotonic() - start) * 1000)
    output = result.stdout.encode("utf-8", errors="replace")
    log_path = log_dir / f"{command_id}.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_bytes(output)
    return {
        "id": command_id,
        "command": shlex.join(command),
        "exit_status": result.returncode,
        "output_sha256": guard.sha256_bytes(output),
        "duration_ms": duration_ms,
        "scope": scope,
    }


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


def remote_validation_commands(
    *, vscode_changed: bool, remote_target: str
) -> list[tuple[str, str]]:
    target = validated_remote_target(remote_target)
    target_tmp = str(PurePosixPath(target) / "tmp")
    target_bin = str(PurePosixPath(target) / "bin")
    sccache_shim = str(PurePosixPath(target_bin) / "sccache")
    passthrough_source = (
        ".codex/skills/trust-ci-release-gates/scripts/compiler_passthrough.sh"
    )
    target_env = (
        f"CARGO_TARGET_DIR={shlex.quote(target)} "
        "CARGO_INCREMENTAL=0 RUSTC_WRAPPER=/usr/bin/env "
        "CARGO_BUILD_RUSTC_WRAPPER=/usr/bin/env "
        f"CC=cc CXX=c++ TMPDIR={shlex.quote(target_tmp)} "
        f"PATH={shlex.quote(target_bin)}:$PATH"
    )
    vscode_target_env = target_env.replace(
        f"TMPDIR={shlex.quote(target_tmp)}", 'TMPDIR="$vscode_tmp"'
    )
    prepare_target = (
        f"mkdir -p -- {shlex.quote(target_tmp)} {shlex.quote(target_bin)} && "
        f"install -m 755 {passthrough_source} {shlex.quote(sccache_shim)}"
    )
    def leased(command: str) -> str:
        return (
            "bash scripts/with_cargo_target_lease.sh "
            f"{shlex.quote(target)} bash -lc {shlex.quote(command)}"
        )

    disk_preflight = (
        'available_kib=$(df --output=avail -k "$HOME" | tail -n 1 | tr -d " "); '
        "required_kib=83886080; "
        'df -hT "$HOME" /tmp; '
        'if [ "$available_kib" -lt "$required_kib" ]; then '
        'printf "exact candidate requires at least 80 GiB free under $HOME; '
        'found %s KiB\\n" "$available_kib" >&2; exit 1; fi'
    )
    commands = [
        ("remote_exact_head", ""),
        ("remote_disk_preflight", disk_preflight),
        ("remote_prepare_target", prepare_target),
    ]
    if vscode_changed:
        commands.append(
            (
                "remote_docs_capture_lifecycle",
                "python3 -m unittest scripts.tests.test_capture_lifecycle -v",
            )
        )
        commands.append(
            (
                "remote_vscode",
                leased(
                    "vscode_tmp=$(mktemp -d /tmp/trust-vscode-candidate.XXXXXX) && "
                    "trap 'rm -rf -- \"$vscode_tmp\"' EXIT && "
                    "cd editors/vscode && npm ci && npm run lint && npm run compile && "
                    f"{vscode_target_env} xvfb-run -a npm test"
                ),
            )
        )
    commands.extend(
        [
            ("remote_fmt", "just fmt"),
            (
                "remote_cross_target_warnings",
                leased(
                    f"{target_env} ./scripts/check_runtime_cross_target_warnings.sh "
                    "--install-missing --require-cross"
                ),
            ),
            (
                "remote_supply_chain",
                leased(f"{target_env} bash scripts/supply_chain_gate.sh"),
            ),
            (
                "remote_architecture_safety",
                leased(f"{target_env} bash scripts/architecture_safety_gate.sh"),
            ),
            (
                "remote_clippy",
                leased(
                    f"{target_env} cargo clippy --all-targets --all-features -- -D warnings"
                ),
            ),
            (
                "remote_reclaim_before_test_all",
                "bash scripts/remove_cargo_target_if_idle.sh "
                f"{shlex.quote(target)} && {prepare_target}",
            ),
            (
                "remote_test_all",
                leased(f"{target_env} CARGO_BUILD_JOBS=1 just test-all"),
            ),
            ("remote_clean_after", 'test -z "$(git status --porcelain=v1 --untracked-files=all)"'),
        ]
    )
    return commands


def validated_remote_target(remote_target: str) -> str:
    path = PurePosixPath(remote_target)
    generated_roots = (
        PurePosixPath("/home/johannes/.cache/codex-targets"),
        PurePosixPath("/tmp"),
    )
    if (
        not path.is_absolute()
        or ".." in path.parts
        or not any(path != root and path.is_relative_to(root) for root in generated_roots)
    ):
        raise ValueError(f"remote target is not a safe generated-output path: {remote_target}")
    return str(path)


def local_maintenance_commands() -> tuple[tuple[str, list[str]], ...]:
    return (
        ("catalog_staleness", [sys.executable, "scripts/check_test_catalog_staleness.py"]),
    )


def local_candidate_validation_commands() -> tuple[tuple[str, list[str]], ...]:
    """Keep full-suite execution on the configured remote builder."""
    return ()


def strict_report_command(*, base_sha: str, head: str, intent: str) -> list[str]:
    return [
        sys.executable,
        "scripts/verification_report_gate.py",
        "--base",
        base_sha,
        "--head",
        head,
        "--intent",
        intent,
        "--strict",
        "--smoke",
        "--out-dir",
        "target/gate-artifacts/verification-release-candidate",
    ]


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
                label = (
                    "ADVISORY" if row["id"] in ADVISORY_COMMAND_IDS else "FAILED"
                )
                print(
                    f"{label} {row['id']}: {log_dir / (row['id'] + '.log')}",
                    file=sys.stderr,
                )
        return 1
    return 0


def stage_passed(records: Sequence[dict[str, Any]], command_ids: Sequence[str]) -> bool:
    required_ids = tuple(
        command_id for command_id in command_ids if command_id not in ADVISORY_COMMAND_IDS
    )
    selected = [row for row in records if row["id"] in required_ids]
    return len(selected) == len(required_ids) and all(
        row["exit_status"] == 0 for row in selected
    )


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
    planner = [
        sys.executable,
        "scripts/plan_tests.py",
        "--intent",
        args.intent,
        "--baseline",
        base_sha,
        "--changed",
        *paths,
    ]
    records.append(command_record("planner", planner, cwd=repo, scope="local", log_dir=log_dir))
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

    local_commands = local_maintenance_commands()
    for command_id, command in local_commands:
        records.append(command_record(command_id, command, cwd=repo, scope="local", log_dir=log_dir))
    if not stage_passed(records, tuple(command_id for command_id, _ in local_commands)):
        return finish_artifact(
            repo,
            head=head,
            base_ref=args.base,
            base_sha=base_sha,
            vscode_changed=vscode_changed,
            records=records,
            log_dir=log_dir,
        )

    strict_command = strict_report_command(
        base_sha=base_sha,
        head=head,
        intent=args.intent,
    )
    records.append(
        command_record("strict_gate", strict_command, cwd=repo, scope="local", log_dir=log_dir)
    )
    if not stage_passed(records, ("strict_gate",)):
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
    remote_commands = remote_validation_commands(
        vscode_changed=vscode_changed, remote_target=args.remote_target
    )
    remote_commands[0] = ("remote_exact_head", remote_head_check)
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
