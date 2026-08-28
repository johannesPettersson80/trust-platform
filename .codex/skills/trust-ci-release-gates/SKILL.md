---
name: trust-ci-release-gates
description: CI and release-gate hardening workflow for truST, including exact pre-push parity, cross-platform test harnesses, and artifact-based release gating.
---

# trust-ci-release-gates

Use this skill when updating CI workflows, release-gate checks, cross-platform test stability, or local pre-push verification scripts.
Use `trust-test-authoring` first for the written-specification, native-test, and
red-green/behavior-lock route. Planner, catalog, invariant, and evidence tools
are nonblocking maintenance reports; they cannot create product work or reject
an otherwise valid specification-and-test change. This skill owns CI parity,
gate wiring, artifact evidence, and release enforcement.

## Test-first rule

- For every new gate/validator behavior, bug fix, or intentional behavior change, add the smallest focused self-test or fixture first and run it to the expected behavior assertion failure.
- Implement only the minimum change, then rerun the same focused test until green. Harness, dependency, registration, timeout, and unrelated failures do not count as red evidence.
- Preserve the red and green commands/results. Release and broad CI gates do not replace this focused loop.

## Core workflow

### Release-candidate state machine

For integration, release, `main`, or any branch whose workspace version differs from
`origin/main`, use the guard bundled with this skill. Do not replace these states with serial
push-and-repair attempts.

1. Freeze one clean candidate and prepare its exact-SHA artifact:

   ```bash
   python3 .codex/skills/trust-ci-release-gates/scripts/release_candidate_guard.py \
     prepare --remote-worktree '<clean exact-SHA trust-builder worktree>'
   ```

   Preparation records complete-diff planning and catalog staleness as advisory
   maintenance. Its strict report command runs only the report-boundary smoke;
   failed artifact output labels those maintenance records `ADVISORY`, never
   `FAILED`; only required candidate-integrity commands receive the `FAILED`
   label.
   exhaustive recursive verification-tooling self-tests remain scheduled/manual
   maintenance. Required acceptance remains bootstrap parity, a clean exact
   candidate and base, diff integrity, the strict smoke, a successful
   trust-builder `just test-all` on the frozen candidate, the applicable native
   product gates, and final remote broad gates. The guard records this as
   `remote_test_all`; it never runs a full suite on the invoking workstation.
   Any commit, base
   movement, missing required command, or dirty checkout invalidates the artifact.
2. Push the frozen candidate once. The installed pre-push hook rejects a release-sensitive push
   without a passing artifact for the exact head and current base.
3. Wait for every required GitHub check. Before editing after a red candidate, collect the whole
   failure set and all failed job logs:

   ```bash
   python3 .codex/skills/trust-ci-release-gates/scripts/release_candidate_guard.py \
     collect-failures --pr '<number>' --wait
   ```

   Do not issue a corrective push from partial CI results. Repair one complete failure ledger,
   return to focused tests, refreeze, and prepare one new candidate.
4. Merge only through `check-merge --pr <number> --execute`; it requires the validated exact head,
   a clean merge state, and every check green.
5. After main CI and the annotated tag/Release workflow, run `verify-release --candidate-head
   <reviewed-head> --branch <candidate-branch>`; completion requires
   the final main SHA, annotated tag, successful Release workflow, GitHub Latest, verified assets
   and checksums, and the expected VS Code Marketplace target versions.
6. `verify-release` then runs `audit-post-merge` with that exact candidate head and branch. The
   audit must fail closed when the candidate is not contained by current main, a candidate
   worktree is dirty, or a named candidate branch no longer points at the exact reviewed head. It
   must report clean candidate worktrees and exact local/remote branches as explicit cleanup
   targets without deleting them. Remove only those reviewed targets, fetch/prune, and rerun the
   audit until it reports no stale candidate state. A release handoff is incomplete without this
   final clean result.

Stop after a second red candidate or two elapsed hours without merge readiness. Report the full
blocker ledger and obtain a new decision instead of continuing an unbounded push/wait/fix loop.

1. Preflight the repository transport before the first push:
   - Record `git remote get-url origin`, `git remote get-url --push origin`, and
     `gh auth status --hostname github.com` without exposing credentials.
   - A GitHub HTTPS OAuth token without the `workflow` scope cannot push changes under
     `.github/workflows/**`. Keep fetches on the configured HTTPS origin, set an authenticated SSH
     push URL when needed, and prove it first with
     `git ls-remote --exit-code "$(git remote get-url --push origin)" HEAD`.
   - Treat a rejected push as a missed preflight, not as the authentication test.
2. Keep local and CI guardrails aligned:
   - local: `./scripts/prepush_ci_gate.sh`
   - CI: `.github/workflows/ci.yml` gates
   - Before calling proof CI-equivalent, match the workflow's exact Rust toolchain and command.
     If CI floats on `stable`, refresh `stable` immediately before final validation and record
     `rustc +stable -Vv`; do not rely on an older builder `stable`.
   - Run the full CI Clippy shape when Clippy is required:
     `cargo clippy --all-targets --all-features -- -D warnings`.
3. Preserve required pre-push checks for `trust-lsp`:
   - `./scripts/check_test_path_hygiene.sh`
   - `cargo fmt --all --check`
   - `cargo clippy -p trust-hir -p trust-lsp -- -D warnings`
   - `cargo test -p trust-lsp --bin trust-lsp`
   - `cargo check -p trust-lsp --tests --target x86_64-pc-windows-gnu`
4. Preserve required runtime reliability checks when mesh/TLS/runtime networking is touched:
   - `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`
   - `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8`
5. Preserve CI contract stability for `trust-runtime`:
   - `cargo test -p trust-runtime --test ci_cicd_contract`
   - `cargo test -p trust-runtime --test config_schema_command`
   - `cargo test -p trust-runtime --test registry_command`
   - On Windows CI, prefer deterministic execution for this suite (`-- --test-threads=1`) when validating matrix reliability.
   - Keep CI fixtures cross-platform (`crates/trust-runtime/tests/fixtures/ci/**`):
     - No hardcoded `unix://` endpoints in shared fixtures.
     - Prefer `tcp://127.0.0.1:0` for parser/validation-only CI contracts.
   - Ensure fixture temp dirs are collision-proof under parallel tests (avoid timestamp-only IDs).
   - In bash with `set -u`, avoid expanding empty arrays (`"${arr[@]}"`); prefer branch-local command functions.
6. When adding new CI gates, ensure release gate aggregation still matches:
   - `.github/workflows/ci.yml`
   - `scripts/generate_release_gate_report.py`
   - For module splits, large-file waiver changes, or `xtask/config/full_map_policy.json` edits,
     run `cargo run -p xtask -- architecture-doctor --full-map` before push.
7. Keep generated docs captures as candidate evidence:
   - Relevant capture, VS Code, tutorial, and docs-asset paths must trigger `Docs Captures` on
     `pull_request` as well as on `main`, with identical path filters.
   - The capture/validation job must have read-only contents permission on pull requests.
   - Put refresh-branch and PR creation in a separate write-enabled job that never runs for
     `pull_request` events.
   - Require the relevant release PR's capture run to pass on the exact candidate SHA before merge.
8. Preserve version/release evidence gate on `main`/`master`:
   - Keep `.github/workflows/ci.yml` job `version-release-guard`.
   - Keep `scripts/check_version_release_evidence.py` in sync with release workflow triggers.
   - Gate requirement: when `[workspace.package].version` changes, require matching `vX.Y.Z` tag, successful `.github/workflows/release.yml` run for that tag, and a published GitHub release.
   - Size the guard's completion budget above the slowest cold release-matrix path with margin.
   - If the guard expires only while the matching Release run is still healthy, wait for that run
     to finish and rerun failed main jobs on the same SHA. Do not create another version or tag.
9. For VS Code extension CI changes, validate Linux headless setup (`xvfb`) remains intact.
10. Keep gate scripts fail-fast, deterministic, and shellcheck-friendly (`set -euo pipefail`).

## Path/Test hygiene rule

Prevent known Windows-only regressions in `trust-lsp` tests:

- Do not serialize git paths in TOML fixtures via raw `to_string_lossy()` backslash output.
- Do not compare dependency source paths by direct raw `PathBuf` equality in workspace symbol tests.
- Keep normalization helpers in place in test files when cross-platform path handling is needed.

## Cross-platform harness rule

- Decode child-process streams with an explicit encoding and non-throwing error policy; also make
  writes safe for strict Windows console encodings. Self-test invalid bytes and non-ASCII output.
- Make reader failures visible and ensure they cannot stop pipe draining while the child runs.
- In raw HTTP/TCP test servers, consume complete headers and any declared request body before
  replying or closing so Windows cannot turn unread request data into a reset.

## Validation

- In trust-platform checkouts on a Raspberry Pi or other slow local host, do not run broad local Rust/runtime gates as the default proof path.
- Use the remote builder for full validation first, especially `just test-all`.
- Ask before starting expensive local commands such as workspace `cargo test`, `cargo test -p trust-runtime ...`, local `just test`, local `just clippy`, or local `just test-all`.
- When the user explicitly requires local full-suite proof before a new release
  candidate, run local `just test-all` after the strict smoke and before any
  remote validation; the release-candidate guard enforces and records it.
- Run required project gates with full Rust gates on the remote builder:
  - `just fmt`
  - `just clippy`
  - `just test-all`
  - `cd editors/vscode && npm run lint && npm run compile`
- Keep exact-candidate disk use bounded: set `CARGO_INCREMENTAL=0` and set both
  `RUSTC_WRAPPER` and `CARGO_BUILD_RUSTC_WRAPPER` to the pass-through
  `/usr/bin/env` wrapper for the candidate's Rust-producing VS Code, Clippy, and
  test commands. An empty value does not override the repository Cargo config;
  the explicit pass-through matches uncached CI behavior. Cargo can still
  propagate the repository-configured `sccache` name into native build scripts,
  so install the skill's reviewed `compiler_passthrough.sh` as a task-owned
  executable named `sccache` and prepend it to `PATH`. That pass-through must
  cover both Rust and native compiler invocations without writing cache
  artifacts. Pin `CC=cc` and
  `CXX=c++` as the native compiler identities. Set
  `CARGO_BUILD_JOBS=1` for the cold `just test-all` run to bound concurrent
  linker space, and set `TMPDIR` to a directory inside the validated task-owned
  target so a constrained system `/tmp` cannot interrupt the proof.
  Reclaim only that validated `CARGO_TARGET_DIR` between Clippy and
  `just test-all`. Never apply that cleanup to a repository, home directory,
  shared cache, unrelated target, or unresolved path.
- Before creating the exact-candidate target, fail closed unless the builder has
  at least 80 GiB available under `$HOME`. Report both `$HOME` and `/tmp`
  filesystem state in that preflight. The floor must be enforced by the
  release-candidate guard, not left as a remembered manual step.
- Use `scripts/verification_report_gate.py --strict --smoke` for pull requests
  and exact-candidate preparation. Run the exhaustive recursive verification
  tooling and its historical self-tests only from the scheduled/manual
  maintenance workflow; they are not native product proof.
- Execute workflow-specific local checks where possible:
  - workflow lint/check
  - script smoke runs
  - artifact existence checks
- For bug fixes, additionally name focused tests so `scripts/check_regression_test_first.py` recognizes them
  (`test_*.py`, `tests.py`, or a `test`/`tests` directory), then run the guard with the candidate
  base and head. When proving an existing failing command, put a non-empty
  `Regression-test-first:` marker in the commit or PR body.
- For runtime-impacting gates, also verify on the remote builder unless the user explicitly approves local execution:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
