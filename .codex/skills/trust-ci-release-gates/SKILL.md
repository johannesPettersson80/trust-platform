---
name: trust-ci-release-gates
description: Ships truST changes the professional way - small pull requests to main, CI as the only gate, Codex review, the merge queue, changelog and versions, version-bump releases and release verification (the exact-SHA guard only until CI covers it). Use for commits, pushes, pull requests, CI workflow or gate-script changes, version bumps, merges, GitHub releases and VS Code Marketplace checks.
---

# Changes, CI and releases

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

Before a push, read the diff for this: a change that adds or grows logic in TypeScript does not
merge.

## The workflow (owner, 31 October 2026: "i want the most professional best workflow")

Work reaches `main` in small pull requests, and CI is the only gate. Every check runs as early as it
can, so a finding costs a fix in its own pull request, never another release round.

1. **One lane, one small pull request.** Branch from `main` (`work/<topic>`) and keep the change to
   a few hours of work. Push it with its pull request from the first commit, ready for review (a
   draft gets no automatic review). Codex reviews it when it opens; after later pushes ask with
   `@codex review`. Fix its findings in the same pull request.
2. **Before each push**: the compile check of the changed crates (AGENTS.md) and the focused tests
   of what changed and of the crates and test files that use it, on the builder. Read the diff for
   logic in TypeScript. Nothing else locally: `just test-all`, the VS Code suites, the
   documentation checks and the release rehearsal run in CI on every push.
3. **CI is the gate.** A pull request runs CI on Linux, macOS and Windows, every workflow whose
   change detection matches the whole diff, and for a version change the release rehearsal. Wait
   for all of them with one waiter. A red check: read its log, fix the cause in the same pull
   request, push. A failure without a code cause is a flake: record it with its log on the
   checklist and fix it as its own item; re-run a job only when its log shows an infrastructure
   cause, and say so in the pull request.
4. **Merge** when every required check is green and Codex has no open finding, through the merge
   queue, which tests the change on top of the current `main`. `main` is always releasable.
5. **Release** with a pull request that only bumps the version and moves the changelog section
   (Releasing, below). It merges like any other.
6. **Release blockers** are regressions and severe bugs: safety, data loss, a broken user journey.
   Any other bug found on the way goes on the checklist for the next release.
7. **Documentation with the change.** For every changelog entry, update in the same pull request
   each page that describes that behaviour: the public docs, the README, the example and lesson
   READMEs, the specification and the screenshots. Search the docs for every command, setting,
   flag and feature the change renames or removes. Screenshots need no commit: the site publishes
   the newest ones rendered on `main`; the pull request's Docs Captures run keeps its own as the
   artifact `docs-captures`.
8. **Platform probes.** When a change touches timing, sockets, paths, processes, linking or shell
   scripts, its pull request's CI on macOS and Windows is the probe; read those logs, not only the
   Linux ones.

### Until the guard is retired (checklist W3 and W4)

CI does not yet run every stage of the release guard (`scripts/release_candidate_guard.py`), nor the
acceptance journeys on the built packages. Until it does:

- a push of a version change still needs the guard's exact-SHA artifact (`prepare`, Release
  candidates below); the pre-push hook enforces it, and it is never bypassed;
- before the bump, get Codex's review on the finished pull request and run the acceptance journeys
  (the walkthrough, Identify from Auto, Try-it, F23) on the package of a dispatched rehearsal
  (`gh workflow run release.yml --ref <branch>`), so the guard runs once;
- remove the guard's target before a guard on a new version: a version bump rebuilds every workspace
  crate beside the old ones, and a warm target filled the volume on 15 and 31 October.

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

## The area gates CI runs

When one of these fails in CI, reproduce it on the builder with the same command:

- **`trust-lsp` tests or dependency/config resolution**: `./scripts/prepush_ci_gate.sh`. It includes
  the test-path hygiene check and the Windows GNU check of trust-lsp's tests.
- **Runtime networking, mesh or TLS**: `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime
  --all-targets` and `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8`.
- **Runtime CI contracts** (`crates/trust-runtime/tests/ci_cicd_contract.rs` or `tests/fixtures/ci/**`):
  `cargo test -p trust-runtime --test ci_cicd_contract --test config_schema_command --test registry_command`.
- **Any `trust-runtime` change**: `./scripts/check_runtime_cross_target_warnings.sh --install-missing
  --require-cross`.
- **Docs Captures paths** (`editors/vscode/**`, `scripts/captures/**`, capture assets or the capture
  workflow): `python3 -m unittest scripts.tests.test_capture_lifecycle
  scripts.tests.test_docs_captures_artifact scripts.tests.test_docs_captures_workflow -v`.
- **VS Code**: `npm run lint && npm run compile && npm test` in `editors/vscode`.
- **Module splits, large-file waivers or `xtask/config/full_map_policy.json`**:
  `cargo run -p xtask -- architecture-doctor --full-map`.

## Push transport

Check it once, before the first push:

- Record `git remote get-url origin`, `git remote get-url --push origin` and `gh auth status
  --hostname github.com`. Never print credentials.
- An HTTPS OAuth token without the `workflow` scope cannot push `.github/workflows/**`. In that
  case, set the SSH push URL:
  `git config remote.origin.pushurl git@github.com:johannesPettersson80/trust-platform.git`.
- Prove it works: `git ls-remote --exit-code "$(git remote get-url --push origin)" HEAD`.

## Releasing

1. **The release pull request** bumps `[workspace.package].version` and the VS Code versions, and
   moves `[Unreleased]` to the version's section; check the text with
   `scripts/extract_changelog_section.py`. It carries nothing else.
2. **Its rehearsal.** The Release workflow rehearses the head of a pull request that changes the
   workspace version: every build, test and Publish script runs and nothing is published, and the
   run keeps its release text and evidence as the artifact `release-rehearsal-v<version>`. A fork's
   pull request is not rehearsed on the project's runners; rehearse its head with
   `gh workflow run release.yml --ref <branch>`.
3. **Merge** it when CI, the rehearsal and Codex are green (while the guard is in use:
   `release_candidate_guard.py check-merge --pr <number> --execute`, which also requires the
   rehearsal that kept `release-rehearsal-v<version>`).
4. **The release follows `main`'s CI.** When `main`'s CI passes on the merge commit, the Release
   workflow tags that commit `v<version>`, creates the GitHub release with the version's changelog
   section, publishes every target to the Marketplace and verifies the tag, the release, Latest and
   the Marketplace listing of every target (specification 24, "Release after main CI"). Never push a
   release tag by hand.
   - When `main`'s CI fails on the merge commit, nothing is released: find the cause; a flake's
     failed jobs may be re-run, and their success starts the release.
   - When a release job fails, fix the cause; re-running the failed jobs continues the release on the
     same commit. A fix that needs a new commit releases from that commit once its CI passes, while
     the version has no tag yet.
5. **Verify**: `release_candidate_guard.py verify-release --candidate-head <head> --branch <branch>`
   with `TMPDIR` on a large disk (the assets are 2.7 GB). It checks the tag on `main`, the Release
   run named by the release's provenance, GitHub Latest, the assets against their checksums, and the
   Marketplace version for darwin-arm64, darwin-x64, linux-arm64, linux-x64 and win32-x64.
6. **Clean up**: `verify-release` runs `audit-post-merge`; remove only the targets it lists, fetch
   with prune and rerun the audit until it reports `clean`.

Readiness words: **verified locally** (compile check and focused tests), **CI green** (every required
check on the exact head), **merge-ready** (CI green, Codex without open findings, the merge queue
accepts it), **released** (`verify-release` passes). An earlier head's results prove nothing about a
later one.

In PR text, `Closes`, `Fixes` and `Resolves` close an issue on merge; `Addresses` does not. Check
that the intended issues are closed after the merge.

### Release candidates (while the guard is in use)

`release_candidate_guard.py prepare --remote-worktree '<clean exact-SHA trust-builder worktree>'`
records the guard's stages for one clean commit: rule-file parity with the primary checkout, a clean
candidate and base, the supply chain and architecture safety gates, the cross-target warnings, the
VS Code suites, `just test-all`, MP-001 parity, the documentation site and, when it applies, the
capture lifecycle. Planner and catalog records are advisory. Any commit, base movement or dirty
checkout invalidates the artifact. It needs 80 GiB free on the selected target filesystem; stage the
candidate by bundle in the guard worktree, and remove the guard's target after every guard.
`preflight` runs the quick stages and reports every failure at once. `collect-failures --pr
<number> --wait` gathers every failed job log of a pull request.

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
