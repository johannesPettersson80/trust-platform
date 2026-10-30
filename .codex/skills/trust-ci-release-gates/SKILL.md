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

## The release flow: one day, one fix round

Run these steps in this order. Each one finds in minutes what a later step finds only in hours. A
guard run takes about two hours and a CI round about four, so every failure found there costs a
whole run. Each step names the release that taught it.

0. **Push early, bump the version last.** Develop the next release on a `work/` branch at `main`'s
   version and push it with a draft pull request from its first commit. Such a push needs no guard,
   so CI on Linux, macOS and Windows, the documentation workflows and the automatic review run on
   every push while the work goes on; fix what they find as it comes. Only the last commit bumps the
   version and moves the changelog section, so the guard and the rehearsal run once, on the finished
   release. (A version bump makes every push of the branch wait for the guard: each fix of 0.25.0,
   0.25.1 and 0.26.0 then waited about two hours for the guard and four for CI, so a release took
   days.)
1. **Scope, text and docs.** Freeze the scope. Bump the version and move `[Unreleased]` to the
   version's section; check the text with `scripts/extract_changelog_section.py`. Copy `AGENTS.md` and
   `.codex/skills/` from the primary checkout. For every changelog entry, update in the same
   candidate each page that describes that behaviour: the public docs, the README, the example and
   lesson READMEs, the specification and the screenshots. Search the docs for every command, setting,
   flag and feature the release renames or removes. Screenshots need no commit: the site publishes
   the newest ones rendered on `main`. Look at the candidate's own: the pull request's Docs Captures
   run keeps them as the artifact `docs-captures`. (The docs checks test links, assets, navigation and
   search, not whether a page still describes the product: 0.25.1's terminal GIFs showed errors, and
   its README linked pages the site did not serve yet.)
2. **Pre-flight: every cheap check, all failures at once.** Stage the candidate in the guard
   worktree and run `release_candidate_guard.py preflight` with the arguments of `prepare`. It runs the
   guard's quick stages with the guard's environment, goes on after a failure and reports every
   failure with its log: formatting, cross-target warnings, the supply chain, the architecture
   doctor (files over 1,000 lines, unsafe registers), Clippy, the documentation build and the docs
   site with its checks. Also run the MP-001 baselines and every generator whose output is committed
   (diagrams, HIR baselines, the runtime's `hmi-drawing.js` and `hmi-display.js`), with the tree
   unchanged afterwards.

   Fix all of them, then repeat the pre-flight. (0.26.0 needed six guard runs, one for each of these:
   a blank line, a file over 1,000 lines, a search rank, a stale baseline, a stale bundle, a test.)
3. **Every test the change reaches.** Run the tests of each changed crate and of the crates and test
   files that use what changed, not only the new tests. (F2's focused runs missed
   `control_studio_pid_group`; the guard's `test-all` found it two hours later.)
4. **Review before the guard.** Review the release diff, new features first, for bugs and for logic
   in TypeScript, and fix what it finds. (The automatic review found 0.25.1's Bash 3.2 bug and
   0.26.0's endpoint bug only after the push.)
5. **The user's journeys on a packaged build.** Build the VSIX as the Release builds it (linux-x64
   on the builder; linux-arm64 for the Pi with `cross`). Install it in a fresh profile with an empty
   data folder and no Rust. Run the release's main journeys with the acceptance harness, every step
   asserted, and review the screenshots. (0.26.0's guard and CI passed while Connect on "This
   project's runtime" failed for every user: no test pressed that button.)
6. **Platform probes.** When the change touches timing, sockets, paths, processes, linking or shell
   scripts, run the affected tests on the macOS and Windows runners from a temporary `probe/<topic>`
   branch at `main`'s version while the guard runs: twenty minutes instead of a four-hour round.
   (0.25.1: Windows linking, restart timing and a reused temporary folder; macOS's instruction cache
   and Bash 3.2.)
7. **The guard, alone on its volume.** Keep check targets on the system disk and remove idle targets
   from the build volume first, so the 80 GiB floor holds (30 October's guard failed it on idle
   targets). Stage the candidate by bundle, then run `prepare`. It goes on after a failing stage and
   reports every failure of the run.
8. **Push, then wait once.** Push and request `@codex review`. The pull request starts everything
   itself: CI, each workflow whose change detection matches the whole diff, and, for a version
   change, the Release rehearsal on the head. Wait for all of them with one waiter. (0.25.0's docs
   workflows never ran on its 10,252-file pull request, and its release jobs failed only after the
   tag.)
9. **One fix round.** Collect every failure and every review finding (`collect-failures`), fix the
   whole set, and run steps 2 to 8 again for the fix, pre-flight first.
10. **Merge, release, verify, clean up.** Run `check-merge --execute`. `main`'s CI then runs on the
    merge commit, and the Release workflow follows it by itself: it tags that commit, publishes the
    GitHub release and the Marketplace packages and verifies them. Nobody pushes a tag. Watch that
    run, then run `verify-release` with `TMPDIR` on a large disk (the assets are 2.7 GB). Finally,
    remove the guard's target and worktrees.

## Changelog and versions

- **Changelog**: user-visible changes get an entry in `CHANGELOG.md` under `## [Unreleased]`, in
  `### Added`, `### Changed` or `### Fixed`. User-visible means runtime behaviour, CLI flags and
  output, the standard library, tutorials and docs, and test harness behaviour. Purely internal
  changes need no entry.
- **Release text**: the version bump renames `## [Unreleased]` to `## [<version>] - <date>` and puts a
  new, empty `## [Unreleased]` above it. The Release workflow publishes that version's section as the
  release's text (`scripts/extract_changelog_section.py`), which refuses a missing or empty section
  and a text over GitHub's 125,000 characters; GitHub would cut it without a word.
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
   `just fmt`, `just clippy`, the documentation build CI runs
   (`RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features`, which `prepare` includes),
   `just test-all`, and applicable area checks below:
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
     capture workflow):
     `python3 -m unittest scripts.tests.test_capture_lifecycle scripts.tests.test_docs_captures_artifact scripts.tests.test_docs_captures_workflow -v`.
     The VS Code `npm test` does not replace it.
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

   For isolated, pinned builder tools, append
   `--remote-tool-path '<complete remote PATH>' --remote-cargo-home '<private Cargo home>'`.
   These optional values apply to every recorded remote command, including initial
   VS Code installation/compilation and formatting before the internal target prefixes.
   With either value supplied, leased commands use a non-login shell so profile
   initialization cannot replace the pinned environment; default commands retain
   their existing login-shell behavior.
   Record the exact values in the candidate command map; preserve the native/cross
   compiler environments and leave shared machine tools unchanged.

   - It records the required commands:
     - parity of `AGENTS.md` and `.codex/skills/` with the primary checkout;
     - a clean candidate and base, and diff integrity;
     - `bash scripts/supply_chain_gate.sh` and `bash scripts/architecture_safety_gate.sh`;
     - the cross-target warnings;
     - `just test-all` on the builder;
     - MP-001 discovery/snapshot parity, reusing the native test binaries;
     - the documentation site's build and checks, as Docs Pages runs them before it deploys (in a
       copy of the candidate's tree);
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
4. **A version change is rehearsed on its pull request.** The Release workflow's plan rehearses
   the head of a pull request that changes the workspace version: every build, test and Publish
   script runs and nothing is published, and the run keeps its release text and evidence as the
   artifact `release-rehearsal-v<version>`. A fork's pull request is not rehearsed on the project's
   runners; rehearse its head with `gh workflow run release.yml --ref <branch>`. 0.25.0's release
   jobs failed only after its tag, and each fix then needed a new version.
5. **Merge** only with `release_candidate_guard.py check-merge --pr <number> --execute`. Besides a
   clean merge state and green checks on the exact head, it requires for a version change a passed
   Release rehearsal on that head that kept `release-rehearsal-v<version>`.
6. **Release a version change.** The merge releases it: when `main`'s CI passes on the merge
   commit, the Release workflow tags that commit `v<version>`, creates the GitHub release with the
   version's changelog section, publishes every target to the Marketplace and verifies the tag, the
   release, Latest and the Marketplace listing of every target (specification 24, "Release after
   main CI"). Never push a release tag by hand.
   - When `main`'s CI fails on the merge commit, nothing is released: find the cause, and when it is
     a flake that a retry may cover under the current authorization, re-run CI's failed jobs; their
     success starts the release.
   - When a release job fails, fix the cause; re-running the failed jobs of that run continues the
     release on the same commit (a tag on that commit, an existing release and a published target
     are accepted). A fix that needs a new commit releases from that commit once its CI passes,
     while the version has no tag yet.
   - Then run
     `release_candidate_guard.py verify-release --candidate-head <reviewed-head> --branch <candidate-branch>`.
     It checks that the tag is on `main` and contains the candidate, that the Release run named by
     the release's provenance passed, GitHub Latest, the assets against their checksums, and the
     Marketplace version for darwin-arm64, darwin-x64, linux-arm64, linux-x64 and win32-x64.
7. **Clean up.** `verify-release` then runs `audit-post-merge`.
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
- **Release workflow**: `scripts/release_plan.py` decides what a run of `release.yml` does, and every
  job checks out the plan's commit. `scripts/check_version_release_evidence.py` verifies a release at
  the end of the run; keep its Marketplace wait inside the publish job's limit. The CI job
  `version-release-guard` checks only that the versions agree and waits for nothing.
- **Change detection**: a workflow whose checks matter only for some paths starts on every pull
  request and asks `scripts/ci_changes.py` in a `changes` job, with its path list in `.github/paths/`;
  GitHub's own path filters compare only the first 300 changed files. A workflow that checks pull
  requests this way ends with a `conclusion` job that needs every check, so a skipped check reads as
  passed and a failed one as failed.
- **Docs Captures**:
  - its `changes` job uses the same path list on pull requests and on `main`;
  - no job of it writes to the repository. Its artifact `docs-captures` holds only the screenshots
    the captures wrote. Before the Docs Pages deploy builds the site, it lays over the images the
    artifact of the newest passing run of `main` that a push, the schedule or a dispatch started
    (specification 22, "Docs Captures");
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

  Between Clippy, the documentation build and `just test-all`, recheck available space on the validated
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
