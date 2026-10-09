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

## Before a push

1. List every required GitHub job and its exact command shape from `.github/workflows/`. A host
   `just clippy` or `just test-all` is not cross-platform parity.
2. On the builder, run `just fmt`, `just clippy` and `just test-all`. Then run the gates for the
   areas you touched:
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
     - the capture lifecycle, when it applies.
   - Planner and catalog records are advisory.
   - Any commit, base movement or dirty checkout invalidates the artifact.
   - It needs 80 GiB free under `$HOME` on the builder.
2. **Push once.** The shared pre-push hook (`scripts/pre-push`, which `git config core.hooksPath`
   points at) rejects a release-sensitive push that has no passing artifact for the exact head and
   base. Never bypass it.
3. **Wait for every required check.** If any check fails, collect the whole failure set and every
   failed job log before editing:
   `release_candidate_guard.py collect-failures --pr <number> --wait`.
   - Fix the complete list in one batch, check with focused tests, refreeze, and prepare a new
     candidate.
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
     run and rerun the failed main jobs on the same SHA. Never create another tag.
6. **Clean up.** `verify-release` then runs `audit-post-merge`.
   - Remove only the exact clean targets it lists.
   - Fetch with prune, and rerun the audit until it reports `clean`.
   - The handoff is incomplete while candidate branches, worktrees or unique commits remain.

Stop and report the full blocker list after a second failed candidate, or after two hours without
merge readiness.

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
  - the job that writes the refresh branch never runs for pull requests.
- **Exact-candidate disk bounds**:
  - `CARGO_INCREMENTAL=0`;
  - `RUSTC_WRAPPER` and `CARGO_BUILD_RUSTC_WRAPPER` set to `/usr/bin/env`;
  - `scripts/compiler_passthrough.sh` installed on `PATH` as `sccache`;
  - `CC=cc` and `CXX=c++`;
  - honor the builder's Cargo job configuration for cold `just test-all`; do not force one job;
  - `TMPDIR` inside the task's own target.

  Between Clippy and `just test-all`, reclaim only that validated target, through
  `scripts/remove_cargo_target_if_idle.sh`.
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
