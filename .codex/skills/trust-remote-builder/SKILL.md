---
name: trust-remote-builder
description: Use when compiling trust-platform, running just/cargo/npm/VS Code gates, synchronizing local work to the shared Hetzner CPU builder, or reporting remote build/test proof.
---

# Trust Remote Builder

Use `trust-test-authoring` before remote proof work to establish the exact cataloged command,
required suite, case/evidence binding, and red-green or behavior-lock posture. The builder executes
that contract; it does not decide the oracle or proof level.

## Builder Contract

Use the shared Hetzner CPU builder for heavy Rust and Node gates.

- SSH alias: `trust-builder`
- Remote repo path: `/home/johannes/projects/trust-platform`
- Remote user: `johannes`
- Purpose: `just`, cargo, npm, VS Code extension tests, release gates, and CPU/headless
  proof.
- Not purpose: final GPU/WebGL/WebGPU visual proof when real hardware acceleration matters.

Do not store private SSH key material, cloud credentials, or provider tokens in the repo.

## Sync Rule

Before a remote gate, make the remote checkout match the exact work being validated.

- Inspect `git remote -v` in the remote checkout before fetching.
- Fetch through that checkout's configured `origin` (normally HTTPS), for example
  `git fetch origin <branch>`. Do not replace it with a hardcoded SSH URL copied from the local
  workstation; the builder does not necessarily have the workstation's GitHub SSH credentials.
- Treat the builder as fetch-and-validation-only unless the user explicitly authorizes a remote
  push. A local SSH push URL is not evidence that the builder can push.
- After fetching, verify the exact remote HEAD being tested and repeat the mandatory `AGENTS.md`
  and `.codex/skills/**` bootstrap verification before building or testing.

## Toolchain Parity Rule

Source parity alone is not CI parity. Before calling a remote result CI-equivalent:

1. Inspect whether the workflow pins Rust or floats on `stable`.
2. Install or refresh the same toolchain immediately before the final gate.
3. Record `rustc -Vv` and `cargo -V` with the proof.
4. Run the workflow's exact command and flags, especially full-workspace Clippy.

The Linux builder does not prove Windows console encoding, locale, TCP-close, or filesystem
behavior. Preserve portable harness tests and treat the Windows GitHub job as authoritative for
those surfaces.

## Disk Preflight Rule

Before running any broad remote gate (`just clippy`, `just test`, `just test-all`, `npm test`, or
large `cargo test`), check disk first and clean generated junk before the run. The builder is a
shared finite machine; do not require unrealistic free-space numbers, and do not create a fresh
cold `target/` tree for every isolated validation copy.

All paths in this section are paths on the remote machine reached through `ssh trust-builder`, not
paths on the local workstation.

Practical minimums:

- For `just test-all` or large native-dependency changes (ADS, OPC UA/OpenSSL, EtherCAT,
  WebGPU/Scena), aim for at least 60G free on `trust-builder:/home/johannes` and at least 3G free
  on `trust-builder:/tmp` after cleanup.
- For `just clippy`, `just test`, VS Code `npm test`, or focused runtime gates, aim for at least
  25G free on `trust-builder:/home/johannes`.
- If the builder has less than that after safe cleanup, do not invent a higher threshold. Report the
  real free space and either run a narrower gate or ask before deleting large unrelated generated
  targets.

Run this preflight before syncing or testing:

```bash
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

If space is below the minimum, clean stale build outputs before running tests. Prefer deleting
only generated caches and targets, not source checkouts:

```bash
ssh trust-builder 'rm -rf "$HOME/projects/<isolated-validation-copy>/target"'
ssh trust-builder 'rm -rf "$HOME/projects/"*/fuzz/target'
ssh trust-builder 'rm -rf "$HOME/.cache/sccache" "$HOME/.cache/codex-targets/"*'
ssh trust-builder 'df -hT /home/johannes /tmp'
```

For isolated validation copies, avoid repeated cold rebuilds by using one warmed target directory
on the remote builder when practical:

```bash
ssh trust-builder 'mkdir -p "$HOME/.cache/codex-targets/trust-platform-gate"'
ssh trust-builder 'cd "$HOME/projects/<isolated-validation-copy>" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" just test-all'
```

Use broad `*/target` cleanup only when the user asked for cleanup or the stale target directories
are clearly validation/build artifacts. Never remove source worktrees or non-generated files as part
of disk cleanup.

If a gate fails with `No space left on device`, `Disk quota exceeded`, `mold: failed to write`, or
`couldn't create a temp dir`, stop. Do not immediately rerun the same gate. First:

1. Kill any still-running cargo/rustc/linker processes from that gate.
2. Re-run the disk preflight.
3. Clean stale generated targets/caches until the minimum is met.
4. Re-run the gate only after reporting that the previous failure was infrastructure, not a test
   assertion.

For local uncommitted work:

```bash
git -C /home/johannes/projects/trust-platform status --short --branch
ssh trust-builder 'git -C "$HOME/projects/trust-platform" status --short --branch'
rsync -az --delete \
  --exclude '/target/' \
  --exclude '/fuzz/target/' \
  --exclude '**/node_modules/' \
  --exclude '/.venv-docs/' \
  /home/johannes/projects/trust-platform/ \
  trust-builder:~/projects/trust-platform/
```

If the remote has unrelated dirty changes, stop and report them instead of overwriting them.

## Gate Commands

Run project gates through SSH:

```bash
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test-all'
```

Useful focused setup and smoke commands:

```bash
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just check'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test-fast'
ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && npm ci'
ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && npm test'
```

## Reporting Proof

Report:

- command run
- remote host alias and repo path
- pass/fail status and timing when available
- Rust and Cargo versions for CI-equivalent proof
- remote git status and HEAD when relevant
- any gate not run and the concrete reason

Do not confuse local checkout state, remote builder checkout state, GitHub branch state, and
published release state.
