---
name: trust-remote-builder
description: Runs truST builds and tests on the shared trust-builder machine. Use when compiling, running cargo, just or npm gates, VS Code extension tests or Playwright on the builder, syncing local work there, managing shared cargo targets and disk space, or reporting remote proof.
---

# trust-builder

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

Logic is proven on the builder by `cargo` runs. VS Code and Playwright runs prove only wiring and
rendering.

`trust-builder` is an SSH alias (user `johannes`) for a shared Hetzner CPU machine. It runs:
- cargo, `just` and npm;
- the VS Code extension tests;
- the release gates;
- headless browser tests.

Two limits:
- It has no GPU, so final WebGL or WebGPU visual proof needs real hardware.
- It has no GitHub push credentials, so it fetches and validates only. Authorized pushes use the
  local checkout.

Use the local Raspberry Pi for editing, git and lightweight inspection. Every Cargo, just and npm
build/test runs through SSH on the builder. Follow AGENTS.md's implementation-first batch cadence;
this skill does not authorize intermediate checks or retries.

## Getting the code there

- **`main`**: the builder's copy is `~/projects/trust-platform`. Fetch through that checkout's
  configured `origin`; check `git remote -v` first, and do not swap in the workstation's SSH URL.
- **Uncommitted or feature work**: rsync the local checkout to its own builder copy:

  ```bash
  rsync -a --delete --exclude /target/ --exclude /fuzz/target/ --exclude '**/node_modules/' \
    --exclude /.venv-docs/ /path/to/local/checkout/ trust-builder:~/projects/<copy>/
  ```

  - Sync one folder to one folder, with matching trailing slashes. `--delete` aimed at a parent
    directory has wiped sibling folders before.
  - If the builder copy has changes of its own, stop and report them instead of overwriting.
  - `--delete` also removes files that exist only on the builder: formatted sources, built bundles
    and evidence. Copy those back before the next sync.
  - A file restored by rsync can keep an old mtime, so cargo may reuse a stale build. `touch` it.
- **Verification evidence batches** (many audit or evidence reports): regenerate them in a clean,
  detached builder worktree. Validate every report there, then copy back only the validated reports.

## Shared cargo targets

Several checkouts build into `~/.cache/codex-targets/<name>` at the same time.
An isolated task target can also live under the mounted volume's
`/mnt/HC_Volume_107089260/builder-storage/cargo-targets/` root. The shared path
policy checks mount state, ownership and canonical paths; no symlink workaround.
Use the same lease/removal scripts and preserve unrelated targets.

- Run every cargo-producing command through the lease:

  ```bash
  ssh trust-builder 'cd ~/projects/<copy> && T=$HOME/.cache/codex-targets/trust-platform-gate && \
    scripts/with_cargo_target_lease.sh "$T" env CARGO_TARGET_DIR="$T" just test-all'
  ```

- Delete a target only with `scripts/remove_cargo_target_if_idle.sh TARGET`. Exit 75 means another
  gate holds the target, so keep it. The lease lives in `~/.cache/codex-target-leases`, outside the
  target, so recreating a target cannot bypass it.
- Never glob-delete `~/.cache/codex-targets/*`. A single empty `lsof` sample does not prove that a
  target is idle.

## Disk

Run this preflight before broad gates:

```bash
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

- Also inspect `df` for the selected target and task `TMPDIR`; the example above only covers the
  home filesystem and `/tmp`. Confirm the intended volume is mounted before using it.
- **Capacity**:
  - allow about 25 GB for Clippy, tests or npm output on the filesystem holding that output;
  - the release guard requires 80 GiB on the selected target filesystem for cold `just test-all`;
  - inspect active task-owned builds and coordinate overlapping large runs on the same filesystem.
    A target lease protects against deletion, not disk consumption by other targets;
  - record actual free space and known concurrent growth. If headroom is insufficient, postpone the
    new run or use an approved target root with capacity. Do not lower guard thresholds or delete
    another session's outputs to make a preflight pass.
- **Cleaning up**: delete only generated outputs, never a source worktree. Examples are an isolated
  copy's `target/`, `fuzz/target/` and `~/.cache/sccache`. Use the idle-target script for shared
  targets.
- **Runtime test binaries**: `cargo test --all` builds about 290 runtime test binaries of up to
  450 MB each, so it can fill the disk. When space is short, run the runtime tests in batches that
  delete their binaries.
- **Out of space**: these errors are infrastructure failures, not test results:
  `No space left on device`, `Disk quota exceeded`, `mold: failed to write` and
  `couldn't create a temp dir`. Then:
  1. stop the gate's leftover processes by PID;
  2. rerun the preflight;
  3. clean up;
  4. retain the failure ledger; run again only within the user's current retry authorization.
  Report the original failure as infrastructure failure, not a failed behavior assertion.

## Running tests there

- **`TMPDIR`**: `/tmp` is a quota tmpfs that other jobs fill. If you see odd Chrome or Xvfb failures
  ("Missing X server", canvases missing edges), `/tmp` is probably full; `echo x > /tmp/.q` tests
  it. Run tests with `TMPDIR` under home, for example `TMPDIR=$HOME/tmp-<task>`.
- **VS Code tests need an X display**:
  - start Xvfb by PID: `Xvfb :<n> -screen 0 1920x1080x24 -nolisten tcp &`;
  - set `DISPLAY=:<n>`, and kill Xvfb by PID afterwards;
  - `TRUST_VSCODE_TEST_GREP` selects suites by title (a regular expression);
  - `ST_LSP_TEST_SERVER=<path>/trust-lsp` reuses a built language server.
- **Killing processes**: kill by PID or by your own process group, never with `pkill -f` or
  `killall`.
- **CPU contention**: never run two timing-sensitive suites at once on the builder. Check active
  workloads and available memory before choosing build parallelism; honor the builder's Cargo job
  configuration unless the task needs a documented override. More Cargo jobs do not speed up a
  single-threaded test, so inspect the active phase before attributing a delay to CPU limits.

## CI parity

Source parity is not CI parity. Before you call a result CI-equivalent:
- **Toolchain**: use the workflow's exact toolchain and command. When CI floats on `stable`, refresh
  it just before the final gate. Record `rustc -Vv` and `cargo -V`.
- **Clippy**: run the CI shape, `cargo clippy --all-targets --all-features -- -D warnings`.
- **`trust-runtime` candidates**: also run
  `./scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross`.
- **Windows**: the builder does not prove Windows console encoding, locale, TCP close or filesystem
  behaviour. For those, the Windows GitHub job is authoritative.

## Reporting proof

For a new evidence batch, check `hostname` and `pwd` inside the same SSH command, and record:
- the exact command, inside `ssh trust-builder '…'`;
- the remote path and its revision, or the local tree it was synced from;
- the result with counts and duration;
- the toolchain versions, for CI-equivalent claims;
- every gate that did not run, and why.

Keep these states apart:
- the local checkout;
- the builder copy;
- the GitHub branch;
- the published release.
