---
name: trust-ci-release-gates
description: CI and release-gate hardening workflow for truST, including exact pre-push parity, cross-platform test harnesses, and artifact-based release gating.
---

# trust-ci-release-gates

Use this skill when updating CI workflows, release-gate checks, cross-platform test stability, or local pre-push verification scripts.
Use `trust-test-authoring` first for planner, catalog, invariant, and red-green/behavior-lock
routing. This skill owns CI parity, gate wiring, artifact evidence, and release enforcement.

## Test-first rule

- For every new gate/validator behavior, bug fix, or intentional behavior change, add the smallest focused self-test or fixture first and run it to the expected behavior assertion failure.
- Implement only the minimum change, then rerun the same focused test until green. Harness, dependency, registration, timeout, and unrelated failures do not count as red evidence.
- Preserve the red and green commands/results. Release and broad CI gates do not replace this focused loop.

## Core workflow

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
- Run required project gates with full Rust gates on the remote builder:
  - `just fmt`
  - `just clippy`
  - `just test-all`
  - `cd editors/vscode && npm run lint && npm run compile`
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
