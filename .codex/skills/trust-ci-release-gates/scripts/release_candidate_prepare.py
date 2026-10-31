#!/usr/bin/env python3
"""Execute the exact local and remote checks for a frozen release candidate."""

from __future__ import annotations

import shlex
import sys
import time
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Callable, Sequence

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


def remote_command(
    host: str, worktree: str, command: str, *, tool_path: str | None = None,
    cargo_home: str | None = None,
) -> list[str]:
    environment = []
    if tool_path is not None:
        environment.append(f"PATH={tool_path}")
    if cargo_home is not None:
        environment.append(f"CARGO_HOME={cargo_home}")
    if environment:
        command = f"env {shlex.join(environment)} bash -c {shlex.quote(command)}"
    return ["ssh", host, f"cd {shlex.quote(worktree)} && {command}"]


def remote_validation_commands(
    *, vscode_changed: bool, remote_target: str, pinned_environment: bool = False,
) -> list[tuple[str, str]]:
    target = validated_remote_target(remote_target)
    target_tmp = str(PurePosixPath(target) / "tmp")
    target_bin = str(PurePosixPath(target) / "bin")
    sccache_shim = str(PurePosixPath(target_bin) / "sccache")
    passthrough_source = (
        ".codex/skills/trust-ci-release-gates/scripts/compiler_passthrough.sh"
    )
    cargo_env = (
        f"CARGO_TARGET_DIR={shlex.quote(target)} "
        "CARGO_INCREMENTAL=0 RUSTC_WRAPPER=/usr/bin/env "
        "CARGO_BUILD_RUSTC_WRAPPER=/usr/bin/env"
    )
    # The toolchain for compiled programs (specification 44 §4.6), built from the candidate as CI
    # builds it: its program library's hash names the candidate's version, so the tests that
    # build and run programs need this one and no other.
    toolchain = str(PurePosixPath(target) / "trust-toolchain")
    target_env = (
        f"{cargo_env} CC=cc CXX=c++ TMPDIR={shlex.quote(target_tmp)} "
        f"TRUST_TOOLCHAIN={shlex.quote(toolchain)} "
        f"PATH={shlex.quote(target_bin)}:$PATH"
    )
    cross_target_env = (
        f"{cargo_env} TMPDIR={shlex.quote(target_tmp)} "
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
        shell_mode = "-c" if pinned_environment else "-lc"
        return (
            "bash scripts/with_cargo_target_lease.sh "
            f"{shlex.quote(target)} bash {shell_mode} {shlex.quote(command)}"
        )

    toolchain_build = leased(
        f"{target_env} bash scripts/native/build_toolchain_libraries.sh "
        f"{shlex.quote(toolchain)}"
    )

    disk_probe = (
        "set -euo pipefail; "
        f"probe=$(bash scripts/cargo_target_path.sh {shlex.quote(target)}) || exit $?; "
        'while [ ! -e "$probe" ]; do probe=$(dirname -- "$probe"); done; '
        'available_kib=$(df --output=avail -k "$probe" | tail -n 1 | tr -d " "); '
        "required_kib=83886080; "
        'if ! [[ "$available_kib" =~ ^-?[0-9]+$ ]]; then '
        'echo "cannot determine available target space" >&2; exit 1; fi; '
    )
    disk_floor_check = (
        'if [ "$available_kib" -lt "$required_kib" ]; then '
        'printf "exact candidate requires at least 80 GiB free on the target filesystem; '
        'found %s KiB\\n" "$available_kib" >&2; exit 1; fi'
    )
    disk_preflight = disk_probe + 'df -hT "$HOME" /tmp "$probe"; ' + disk_floor_check
    conditional_reclaim = (
        disk_probe
        + 'if [ "$available_kib" -ge "$required_kib" ]; then '
        + 'printf "Keeping warm Cargo target: %s (%s KiB available)\\n" '
        + f'{shlex.quote(target)} "$available_kib"; '
        + "else "
        + f"bash scripts/remove_cargo_target_if_idle.sh {shlex.quote(target)} || exit $?; "
        + f"({prepare_target}) || exit $?; "
        # The toolchain for compiled programs lives in the target, so a reclaim builds it again
        # (guard prepare 12 on 27 October 2026 ran test-all without it).
        + f"({toolchain_build}) || exit $?; "
        + disk_probe
        + disk_floor_check
        + "; fi"
    )
    commands = [
        ("remote_exact_head", ""),
        ("remote_disk_preflight", disk_preflight),
        ("remote_prepare_target", prepare_target),
        ("remote_toolchain", toolchain_build),
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
                    # Every rendered journey runs after a failure, so one run shows them all.
                    f"{vscode_target_env} TRUST_VSCODE_RENDERED_CONTINUE=1 xvfb-run -a npm test"
                ),
            )
        )
    commands.extend(
        [
            ("remote_fmt", "just fmt"),
            (
                "remote_cross_target_warnings",
                leased(
                    f"{cross_target_env} ./scripts/check_runtime_cross_target_warnings.sh "
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
                # The same documentation build as CI's Documentation job, so a doc error cannot
                # pass the guard and then fail CI (16 October 2026, PR #137).
                "remote_docs",
                leased(
                    f"{target_env} RUSTDOCFLAGS='-D warnings' "
                    "cargo doc --no-deps --all-features"
                ),
            ),
            (
                # The documentation site's build and checks, as Docs Pages runs them after a merge:
                # path filters see only a pull request's first 300 files, so 0.25.0's 10,252 ran
                # none of them and the site stopped publishing. They run in a copy of the
                # candidate's tree, since the media step copies files into it.
                "remote_docs_site",
                leased(
                    "set -euo pipefail; "
                    f"venv={shlex.quote(str(PurePosixPath(target) / 'docs-venv'))}; "
                    '[ -x "$venv/bin/python" ] || python3 -m venv "$venv"; '
                    '"$venv/bin/python" -m pip install -q -r docs/requirements.txt; '
                    f"site=$(mktemp -d {shlex.quote(target_tmp)}/docs-site.XXXXXX); "
                    "trap 'rm -rf -- \"$site\"' EXIT; "
                    'git archive HEAD | tar -x -C "$site"; cd "$site"; '
                    'export PATH="$venv/bin:$PATH"; '
                    "python scripts/generate_public_docs_media.py && "
                    "python scripts/check_public_docs_ia.py && "
                    "python scripts/check_public_docs_links.py && "
                    "python scripts/check_example_catalog_links.py && "
                    "python scripts/check_openot_database_examples.py && "
                    "mkdocs build --strict && "
                    "python scripts/check_public_docs_assets.py && "
                    "python scripts/check_public_docs_search.py"
                ),
            ),
            (
                "remote_reclaim_before_test_all",
                conditional_reclaim,
            ),
            (
                "remote_test_all",
                leased(f"{target_env} just test-all"),
            ),
            (
                "remote_mp001_parity",
                leased(f"{target_env} bash scripts/check_mp001_split_parity.sh --verify"),
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
        PurePosixPath("/mnt/HC_Volume_107089260/builder-storage/cargo-targets"),
    )
    if (
        not path.is_absolute()
        or any(part in (".", "..") for part in remote_target.split("/"))
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


# A failed prerequisite ends the run, because the stages after it cannot run (specification 22,
# "Release-candidate failure reporting"). Any other failed stage is recorded and the run goes on, so
# one run reports every failure: 0.26.0 needed six runs, each ended by one failure.
PREREQUISITE_REMOTE_STAGES = frozenset(
    {
        "remote_exact_head",
        "remote_disk_preflight",
        "remote_prepare_target",
        "remote_toolchain",
        "remote_reclaim_before_test_all",
    }
)

# The quick stages `preflight` runs: minutes instead of the hours of `prepare`.
PREFLIGHT_REMOTE_STAGES = (
    "remote_exact_head",
    "remote_disk_preflight",
    "remote_prepare_target",
    "remote_fmt",
    "remote_cross_target_warnings",
    "remote_supply_chain",
    "remote_architecture_safety",
    "remote_clippy",
    "remote_docs",
    "remote_docs_site",
)


def run_remote_stages(
    commands: Sequence[tuple[str, str]],
    run_stage: Callable[[str, str], dict[str, Any]],
) -> list[dict[str, Any]]:
    """Runs `commands` in order and returns their records; stops only after a failed prerequisite."""
    records: list[dict[str, Any]] = []
    for command_id, command in commands:
        records.append(run_stage(command_id, command))
        if records[-1]["exit_status"] != 0 and command_id in PREREQUISITE_REMOTE_STAGES:
            break
    return records


def report_preflight(records: Sequence[dict[str, Any]], log_dir: Path) -> int:
    """Prints every stage's result; writes no artifact, so no push can rely on a preflight."""
    failed = [row for row in records if row["exit_status"] != 0]
    for row in records:
        verdict = "PASSED" if row["exit_status"] == 0 else "FAILED"
        print(f"{verdict} {row['id']}: {log_dir / (row['id'] + '.log')}")
    print(f"preflight: {len(records)} stages, {len(failed)} failed; no artifact written")
    return 1 if failed else 0


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
    preflight = bool(getattr(args, "preflight", False))
    # A preflight keeps its logs apart, so it never replaces the evidence of a `prepare` of the same head.
    log_dir = guard.state_root(repo) / "logs" / head / ("preflight" if preflight else "")
    records: list[dict[str, Any]] = []

    def conclude(vscode_changed: bool) -> int:
        if preflight:
            return report_preflight(records, log_dir)
        return finish_artifact(
            repo,
            head=head,
            base_ref=args.base,
            base_sha=base_sha,
            vscode_changed=vscode_changed,
            records=records,
            log_dir=log_dir,
        )


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
        return conclude(vscode_changed)

    local_commands = local_maintenance_commands()
    for command_id, command in local_commands:
        records.append(command_record(command_id, command, cwd=repo, scope="local", log_dir=log_dir))
    if not stage_passed(records, tuple(command_id for command_id, _ in local_commands)):
        return conclude(vscode_changed)

    strict_command = strict_report_command(
        base_sha=base_sha,
        head=head,
        intent=args.intent,
    )
    records.append(
        command_record("strict_gate", strict_command, cwd=repo, scope="local", log_dir=log_dir)
    )
    if not stage_passed(records, ("strict_gate",)):
        return conclude(vscode_changed)

    remote_head_check = (
        f'test "$(git rev-parse HEAD)" = {shlex.quote(head)} && '
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"'
    )
    remote_commands = remote_validation_commands(
        vscode_changed=vscode_changed, remote_target=args.remote_target,
        pinned_environment=(
            getattr(args, "remote_tool_path", None) is not None
            or getattr(args, "remote_cargo_home", None) is not None
        ),
    )
    remote_commands[0] = ("remote_exact_head", remote_head_check)
    if preflight:
        remote_commands = [row for row in remote_commands if row[0] in PREFLIGHT_REMOTE_STAGES]
    records.extend(
        run_remote_stages(
            remote_commands,
            lambda command_id, command: command_record(
                command_id,
                remote_command(
                    args.remote_host, args.remote_worktree, command,
                    tool_path=getattr(args, "remote_tool_path", None),
                    cargo_home=getattr(args, "remote_cargo_home", None),
                ),
                cwd=repo,
                scope=args.remote_host,
                log_dir=log_dir,
            ),
        )
    )

    return conclude(vscode_changed)
