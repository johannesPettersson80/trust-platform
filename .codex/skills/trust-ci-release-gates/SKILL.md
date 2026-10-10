---
name: trust-ci-release-gates
description: Prepares and ships truST changes - changelog and version updates, CI parity, pre-push checks, the exact-SHA release-candidate guard, merge, tag and release verification. Use for commit or push preparation, version bumps, CI workflow or gate-script changes, release candidates, merges, tags, GitHub releases and VS Code Marketplace checks.
---

# Changes, CI and releases

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

Before a push, read the diff for this: a change that adds or grows logic in TypeScript does not
ship.

## Changelog and versions

- **Changelog**: user-visible changes get an entry in `CHANGELOG.md` under `## [Unreleased]`, in
  `### Added`, `### Changed` or `### Fixed`. User-visible means runtime behaviour, CLI flags and
  output, the standard library, tutorials and docs, and test harness behaviour. Purely internal
  changes need no entry.
- **Version**: a release-notable change bumps `[workspace.package].version` in `Cargo.toml`, unless
  the user says not to.
- **VS Code versions**: when the version changes, or when the extension's behaviour changes, set
  `editors/vscode/package.json` and the root version entries of `editors/vscode/package-lock.json`
  to the same version.
- **Docs**: keep tutorials, examples, specifications and coverage documents in line with what ships.

## Plan the final validation

Before implementation is frozen, read the current workflows and guard scripts. In the existing task
record, map each required job to its command, toolchain, targets/features, evidence and prerequisites.
Distinguish the pinned/MSRV compiler from floating CI stable; record the actual versions used. Keep
native Windows/macOS proof separate from Linux cross-compilation. Review new tests and their imports,
feature gates and registrations as part of the full diff before spending the batch.
For a related candidate, read the preceding failure ledger and its corrected commands,
including compiler environment, temporary-path and platform prerequisites; carry those
corrections into the new command map before freezing.

Choose the release path before scheduling commands. Once implementation is complete, perform the
single final formatting and required generated-file preparation on the builder with the guard's exact
toolchain and configuration, including touched `include!` fragments. This is the preparation phase of the
planned batch, not a per-edit check or another full test run. Copy back and review its complete diff
before the authorized commit. The exact-SHA guard then validates that clean committed source;
its formatting step must leave the tree unchanged. Never restore formatter output during a running
guard to make the final cleanliness check pass.

If an exact-SHA guard is required, commit only when authorized, then use `prepare` for the checks it
already runs. Do not run `just test-all` once
before the commit and again inside prepare merely to satisfy two descriptions of the same gate.
Required scope batches remain distinct when explicitly authorized; their evidence cannot substitute
for the exact-SHA artifact. A commit, rebase or source change requires evidence for the new candidate.

## Before a push

1. Confirm the required job/command map against `.github/workflows/` and the guard. A host
   `just clippy` or `just test-all` is not cross-platform parity.
2. The following checks must pass on the builder in the authorized batch. Count commands covered
   by `prepare` once; add only required checks it does not cover:
   `just fmt`, `just clippy`, `just test-all`, and applicable area checks below:
   - **`trust-lsp` tests or dependency/config resolution**: `./scripts/prepush_ci_gate.sh`. It
     includes the test-path hygiene check and the Windows GNU check of trust-lsp's tests.
   - **Runtime networking, mesh or TLS**:
     `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets` and
     `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8`.
   - **Runtime CI contracts** (`crates/trust-runtime/tests/ci_cicd_contract.rs` or
     `tests/fixtures/ci/**`):
     `cargo test -p trust-runtime --test ci_cicd_contract --test config_schema_command --test registry_command`.
   - **Any `trust-runtime` change**:
     `./scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross`.
   - **Docs Captures paths** (`editors/vscode/**`, `scripts/captures/**`, capture assets or the
     capture workflow): `python3 -m unittest scripts.tests.test_capture_lifecycle -v`. The VS Code
     `npm test` does not replace it.
   - **VS Code**: `npm run lint && npm run compile && npm test` in `editors/vscode`.
   - **Module splits, large-file waivers or `xtask/config/full_map_policy.json`**:
     `cargo run -p xtask -- architecture-doctor --full-map`.
3. Check the push transport once, before the first push:
   - Record `git remote get-url origin`, `git remote get-url --push origin` and
     `gh auth status --hostname github.com`. Never print credentials.
   - An HTTPS OAuth token without the `workflow` scope cannot push `.github/workflows/**`. In that
     case, set the SSH push URL:
     `git config remote.origin.pushurl git@github.com:johannesPettersson80/trust-platform.git`.
   - Prove the SSH push URL works:
     `git ls-remote --exit-code "$(git remote get-url --push origin)" HEAD`.

## Release candidates

These pushes go through the guard in this skill's `scripts/`:
- integration, release and `main` pushes;
- any branch whose workspace version differs from `origin/main`.

1. **Prepare.** Freeze one clean candidate and prepare its exact-SHA artifact in a clean builder
   worktree:

   ```bash
   python3 .codex/skills/trust-ci-release-gates/scripts/release_candidate_guard.py \
     prepare --remote-worktree '<clean exact-SHA trust-builder worktree>'
   ```

   - It records the required commands:
     - parity of `AGENTS.md` and `.codex/skills/` with the primary checkout;
     - a clean candidate and base, and diff integrity;
     - `bash scripts/supply_chain_gate.sh` and `bash scripts/architecture_safety_gate.sh`;
     - the cross-target warnings;
     - `just test-all` on the builder;
     - MP-001 discovery/snapshot parity, reusing the native test binaries;
     - the capture lifecycle, when it applies.
   - Planner and catalog records are advisory.
   - Any commit, base movement or dirty checkout invalidates the artifact.
   - It needs 80 GiB free on the selected target filesystem on the builder.
2. **Push once.** The shared pre-push hook (`scripts/pre-push`, which `git config core.hooksPath`
   points at) rejects a release-sensitive push that has no passing artifact for the exact head and
   base. Never bypass it.
3. **Wait for every required check.** If any check fails, collect the whole failure set and every
   failed job log before editing:
   `release_candidate_guard.py collect-failures --pr <number> --wait`.
   - Keep the source frozen while results are being collected. Within an authorized batch,
     continue independent checks when safe; record dependent checks as unrun when prerequisites
     fail. Do not bypass a guard's failure or alter its control flow ad hoc.
   - Fix the complete known set, including defects in new tests, and review the full correction.
     Refreeze and run the consolidated correction batch only under the user's current authorization.
     Do not automatically rerun, or ask again when that authorization already exists.
   - Inspect native and VS Code job logs and `*-retry-rescued.txt` artifacts even
     when GitHub marks the job green. A passing automatic retry does not close
     the first failure: retain it, identify its cause and obtain the applicable
     correction/validation authorization before merge.
   - Never push corrections from partial results.
4. **Merge** only with `release_candidate_guard.py check-merge --pr <number> --execute`.
5. **Release a version change.**
   - Wait for main CI.
   - Push the annotated tag `v<version>` from the same main SHA.
   - Run
     `release_candidate_guard.py verify-release --candidate-head <reviewed-head> --branch <candidate-branch>`.
     It checks:
     - the tag;
     - the Release workflow;
     - GitHub Latest;
     - the assets and checksums;
     - the Marketplace version for darwin-arm64, darwin-x64, linux-arm64, linux-x64 and win32-x64.
   - If the main version guard expired only because the Release run was still going, wait for that
     run, then rerun the failed main jobs on the same SHA only under the current retry
     authorization. Never create another tag.
6. **Clean up.** `verify-release` then runs `audit-post-merge`.
   - Remove only the exact clean targets it lists.
   - Fetch with prune, and rerun the audit until it reports `clean`.
   - The handoff is incomplete while candidate branches, worktrees or unique commits remain.

After a second failed candidate, or two hours without merge readiness, report the complete blocker
list, elapsed time and next action. Continue only within the current scope and retry authorization;
this report is not a new permission gate when the user already authorized continued corrections.

Use precise readiness claims:

- **Scope verified**: the scope's required evidence passes for its recorded source identity.
- **Guard passed / push-ready**: the required exact-SHA artifact passes and push prerequisites hold.
- **Merge-ready**: current-head required GitHub checks and reviews pass and `check-merge` allows it.
- **Released**: `verify-release` confirms the tag, workflow, assets and Marketplace requirements.

A passing earlier scope or old-head CI cannot establish a later state. Report the active stage,
actual elapsed time and remaining stages; do not describe unpushed corrections as GitHub results.

In PR text, `Closes`, `Fixes` and `Resolves` close an issue on merge; `Addresses` does not. Check
that the intended issues are closed after the merge.

## Changing CI or gate scripts

- **Shared scripts**: local runs and CI call the same scripts: `scripts/prepush_ci_gate.sh`,
  `scripts/supply_chain_gate.sh` and `scripts/architecture_safety_gate.sh`. Do not copy their
  commands into workflows.
- **New gates**: a new gate also updates `scripts/generate_release_gate_report.py`.
- **Version guard**: keep the `version-release-guard` job in `.github/workflows/ci.yml` and
  `scripts/check_version_release_evidence.py` in line with the release workflow's triggers. Give it a
  time budget above the slowest cold release path.
- **Docs Captures**:
  - it runs on pull requests with the same path filters as on `main`;
  - its validation job has read-only contents permission;
  - the job that writes the refresh branch never runs for pull requests;
  - a newly linked public specification needs its snippet page, navigation and
    index/search entries. Include the workflow's strict MkDocs build and public
    docs checks in the candidate batch; capture-lifecycle tests do not cover
    publication or cross-page links.
- **Target storage**: use a strict descendant of `~/.cache/codex-targets/`, `/tmp/`, or
  `/mnt/HC_Volume_107089260/builder-storage/cargo-targets/`. The shared path policy
  requires the volume mount, canonical paths and current-user ownership; leases
  and idle-only removal apply unchanged. Keep the 80 GiB floor on the target's
  actual filesystem. Do not substitute a symlink to bypass target-root policy.
- **Exact-candidate disk bounds**:
  - `CARGO_INCREMENTAL=0`;
  - `RUSTC_WRAPPER` and `CARGO_BUILD_RUSTC_WRAPPER` set to `/usr/bin/env`;
  - `scripts/compiler_passthrough.sh` installed on `PATH` as `sccache`;
  - `CC=cc` and `CXX=c++` for native builds only;
  - cross-compilation commands use `env -u CC -u CXX` so native overrides cannot
    select a host compiler for the target; reuse the guard's separate native and
    cross-target environments in supplemental commands;
  - honor the builder's Cargo job configuration for cold `just test-all`; do not force one job;
  - `TMPDIR` inside the task's own target.

  Between Clippy and `just test-all`, recheck available space on the validated
  target filesystem. Preserve the warm target when the existing 80 GiB floor is
  met. Otherwise reclaim only that target through
  `scripts/remove_cargo_target_if_idle.sh`, prepare its directories again and
  recheck the same floor. A busy target, unreadable space measurement or still
  insufficient space fails the stage; never delete an active target or lower the floor.
- **Script style**: gate scripts fail fast and pass shellcheck (`set -euo pipefail`). With `set -u`,
  do not expand arrays that may be empty.
- **Regression tests for gate tooling**: name the focused test so that
  `scripts/check_regression_test_first.py` finds it (`test_*.py`, `tests.py`, or a `test` or `tests`
  directory). When it proves an existing failing command, add a `Regression-test-first:` line to the
  commit or PR.
- **Verification report**: pull requests and candidates run
  `scripts/verification_report_gate.py --strict --smoke`. The exhaustive verification tooling runs
  only in the scheduled workflow.
- **Missing tools**: after fixing a missing tool in one workflow, search `.github/workflows/` for the
  same tool, because `ci.yml` and `release.yml` drift apart.
- **VS Code CI**: keep `xvfb` in the VS Code CI job.

## Cross-platform test hygiene

- **Fixtures**:
  - never hardcode a `unix://` endpoint in a shared fixture; use `tcp://127.0.0.1:0`;
  - never write raw `to_string_lossy()` paths into TOML fixtures, because Windows backslashes break
    them;
  - compare paths through the normalization helpers;
  - make temporary directories collision-proof, not timestamp-based.
- **Windows**: run `ci_cicd_contract` with `-- --test-threads=1`.
- **Child processes**: decode their output with an explicit encoding and a non-throwing error policy.
  Keep draining pipes even when a reader fails.
- **Raw HTTP/TCP test servers**: read the whole request before replying or closing. Otherwise Windows
  turns the unread data into a reset.
- **Sockets**: call `set_nonblocking(false)` on every stream accepted from a nonblocking listener.
  Windows inherits the flag; Linux does not.
