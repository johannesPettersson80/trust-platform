# Verification Program Reconciliation and MQTT Release Checklist

Status: recovery and reconciliation required before the next push.

Created: 2026-08-28.

## Proven Starting State

- [x] `RECOVER-001` Record the canonical checkout. Canonical agent source and
  current checkout are both `/home/johannes/projects/trust-platform`; the
  checkout is on `plc-verification-program` at
  `e4a4dec94bdb75922eb5b8e22d308a9454247add`. `AGENTS.md` and the complete
  `.codex/skills` tree match the canonical source.
- [x] `RECOVER-002` Confirm that the verification program was integrated.
  PR #107 merged at `41d94f25509604c94fba4ddc4b1d455e8aafcd81` and PR #109
  merged at `a3df2254c03030a2a7c0c8e2653294628012162c`. The latter is
  tagged and published as `v0.24.62`.
- [x] `RECOVER-003` Confirm that the primary checkout is stale, not an
  authoritative unpushed continuation. Its HEAD is dated 2026-08-20; the
  Phase 18 candidate is dated 2026-08-24. The local and integration histories
  are not directly ancestral, so the reported 615 commits cannot be replayed
  or discarded from commit counts alone.
- [x] `RECOVER-004` Record the current MQTT state. PR #114 merged to `main` at
  `5bca03b85cf5228c1027f52bab1cb5d39739f498` and was released as
  `v0.24.63`. PR #115 remains open at
  `1826c747c452db0ffad55609ceeacbb5b840576a`.
- [x] `RECOVER-005` Preserve the current dirty-work inventory. Thirteen tracked
  files were modified before this checklist was created. They mix
  release-candidate tooling, MQTT configuration/spec/test work, changelog, and
  version files. No reset, checkout, cleanup, or branch deletion is permitted
  before the ownership audit below.

## Lane A: Reconcile and Retire the Stale Verification Checkout

- [x] `RECON-001` Capture a recovery bundle containing the branch refs, staged
  and unstaged patches, untracked-file inventory, worktree inventory, and the
  exact `origin/main` SHA. Store it outside the repository and verify it can be
  read before changing the checkout.
  Evidence: verified bundle, binary patches, and dirty-file archives are under
  `/home/johannes/.cache/trust-platform-recovery-20260828-issue111/`. Seven
  obsolete merged or superseded worktrees have been removed after their exact
  heads or content lineage were verified; all uncertain and active work remains.
- [x] `RECON-002` Compare content, not ancestry, between the local branch,
  PR #107, PR #109, and current `origin/main`. Classify every differing path as:
  `already_integrated`, `superseded`, `unique_product_change`,
  `unique_spec_or_native_test`, `private_agent_tooling`, or `unrelated_wip`.
  Evidence: PR #109's released series maps the old integration product commit
  to its corrected first commit and fourteen subsequent CI repairs. The merged
  Phase 18 board records the complete path-level deletion/product/spec/test
  audit. Retired catalog history was archived rather than replayed.
- [x] `RECON-003` Audit each of the thirteen pre-existing dirty files against
  current `origin/main` and PR #115. Three files already match `origin/main`
  byte-for-byte: the release-gate skill, `release_candidate_guard.py`, and
  `release_candidate_prepare.py`. Do not recommit those copies. Resolve the
  remaining files by content and owner.
  Evidence: current-main copies were discarded; the contradictory MQTT
  mapping-rejection experiment was archived; MQTT support remains in PR #115;
  and only the release-asset fix plus cleanup enforcement moved to this fresh
  main-based branch.
- [x] `RECON-004` Do not revive retired verification metadata as product work.
  A retained product change must have an observable contract in an owning
  specification and a native executable assertion at the closest boundary.
- [x] `RECON-005` For each genuinely unique defect, create a fresh branch from
  current `origin/main`, manually bootstrap `AGENTS.md` and `.codex/skills`,
  then complete one honest specification, focused red, minimal fix, focused
  green slice. Do not cherry-pick the 615-commit history wholesale.
- [x] `RECON-006` If no unique product/spec/test change survives the audit,
  record that conclusion with path-level evidence and mark the stale branch as
  superseded by PR #109. If changes survive, merge them through their own
  focused candidate before the MQTT release.
  Evidence: no verification-campaign product change survived. Two release-gate
  behaviors survived and are isolated here: flat GitHub asset reconciliation
  and post-merge candidate-state audit.
- [ ] `RECON-007` Run the verification/release-tooling focused self-tests for
  every retained tooling change. Run the strict report smoke and ensure it
  remains advisory about historical metadata and authoritative only about
  direct specification/native-test integrity.
  - Focused red/green evidence: the flat GitHub asset assertion failed on an
    isolated current-`main` checkout because the staged manifest path did not
    resolve to the published basename, then passed with the retained fix. The
    post-merge command, explicit cleanup-target projection, dirty-worktree
    blocking, remote-ref refresh, CI registration, required release identity,
    and release-to-cleanup chaining each produced an assertion-level red before
    its minimum implementation and passed afterward. Cold, uncached Clippy then
    exposed compile-time runtime-test binary discovery that cached builds had
    hidden; the registered portability assertion rejected all 23 affected
    integration tests before they moved to execution-time Cargo environment
    discovery. A separate release-guard assertion proved that advisory planner
    and catalog records were incorrectly printed as `FAILED` before the output
    classified them as `ADVISORY`. Exact cold Clippy on checkpoint `2d3c5383`
    then proved that the first portability scanner was too narrow: it matched
    only `trust-runtime` binary names under the runtime crate and missed 10
    `trust-dev` and `trust-harness` compile-time lookups. The broadened native
    assertion failed with all 10 paths, then passed after every workspace
    integration test moved to execution-time lookup. Focused all-target,
    all-feature Clippy passed for both `trust-dev` and `trust-runtime`, the
    three-test `trust-dev` CLI suite passed, and all 15 `trust-harness` command
    tests passed. The replacement cold `test-all` was stopped before a quota
    error when its isolated target exceeded 55 GiB and only 11 GiB remained;
    no test assertion failed. The documented 60 GiB start target was therefore
    insufficient. A focused release-guard assertion then failed because no
    disk-preflight command existed and passed after exact preparation required
    80 GiB before target creation and recorded that check in its SHA-bound
    artifact.
    The generated command was also executed directly on `trust-builder` with
    60 GiB available: it returned status 1 before target creation and reported
    the required 80 GiB floor.
  - Current focused green: 22 release-candidate guard tests, 10 post-merge
    cleanup tests, 2 runtime binary-path contract tests, and 31 workflow,
    gate-inventory, report-input, and reviewed-source digest tests. Version
    alignment and diagram drift checks also pass. Exact-commit strict smoke is
    pending the checkpoint commit.
- [ ] `RECON-008` Freeze any retained reconciliation candidate and run the
  applicable exact-SHA remote builder gates, including `just fmt`, `just
  clippy`, and `just test-all`. Prepare the release-candidate artifact before
  the single push.
- [ ] `RECON-009` Merge only the exact green SHA through the release-candidate
  guard. If the reconciliation has no retained change, close this lane without
  creating an empty PR or release.
- [x] `RECON-010` Add a test-first post-merge audit command to the release
  guard. It must identify the candidate's merged local/remote branch and
  worktree, fail closed on dirty or unique content, and emit explicit safe
  cleanup targets. It must never delete an unrelated or unresolved worktree
  automatically. Make successful post-merge audit evidence part of release
  verification and handoff completion.
  Evidence: the focused cleanup module contains 10 green native tests covering
  clean, dirty, moved, missing, and unmerged candidate states, refreshed remote
  refs, exact release identity, and release-to-audit chaining. The actual
  post-merge audit remains a mandatory Lane A completion step after release.

## Lane B: Complete MQTT Issue #111 Separately

- [ ] `MQTT-001` Keep PR #115 frozen while Lane A is being reconciled. Do not
  mix stale verification-branch files or release-gate recovery into the MQTT
  diff.
- [ ] `MQTT-002` Collect every human and automated review finding on PR #115
  before editing. Validate each finding against the written runtime contract
  and native assertions, then create one complete repair ledger.
- [ ] `MQTT-003` Rebase or rebuild the MQTT candidate from current
  `origin/main` after Lane A closes. Manually bootstrap and verify the agent
  files before editing or testing.
- [ ] `MQTT-004` Preserve the supported `io.params.mappings` contract. Do not
  replace mapping support with silent ignore or blanket rejection. Confirm the
  exact schema, duplicate/conflict rules, default raw-topic compatibility, and
  validation errors in `docs/specs/11-runtime-engine.md` before code changes.
- [ ] `MQTT-005` Prove `MainInstance.Green`, `MainInstance.Yellow`, and
  `MainInstance.Red` resolution, including Boolean and enumeration-backed
  values. Prove that mapping direction `write` means PLC-to-broker and that
  `read` means broker-to-PLC in the shipped implementation.
- [ ] `MQTT-006` Preserve an honest focused assertion red for every newly
  confirmed behavior defect. A compile error, harness failure, timeout, or
  unavailable broker is not valid red evidence. Apply only the minimum
  production change and rerun the same assertion green.
- [ ] `MQTT-007` Run a real Mosquitto vertical with the traffic-light tutorial.
  Assert continuing scan counts and timestamped publications on all configured
  `traffic/north/*` topics. Also assert the backward-compatible raw input and
  output topics when mappings are absent. Unit-only driver tests do not satisfy
  this row.
- [ ] `MQTT-008` Run malformed configuration, unknown tag, duplicate topic,
  wrong direction, payload coercion, enum identity, reconnect, shutdown, and
  worker-liveness native tests. Distinguish configuration failure, tag mapping
  failure, scheduler failure, and MQTT worker failure in assertions and logs.
- [ ] `MQTT-009` Update the changelog and bump the release version from current
  `v0.24.63` to the next approved patch version. Synchronize `Cargo.toml`,
  `Cargo.lock`, VS Code `package.json`, and root package-lock versions.
- [ ] `MQTT-010` Run all required runtime vertical tests remotely: `api_smoke`,
  `debug_control`, `complete_program`, and `runtime_reliability`, plus the
  runtime networking warning-deny check and eight-iteration mesh/TLS stability
  gate.
- [ ] `MQTT-011` Inventory every job and command in the current GitHub
  workflows. Produce a job-to-command matrix and execute every Linux-runnable
  command on the exact candidate at `trust-builder`, including formatting,
  full-feature Clippy, all tests, conformance, architecture, supply chain,
  documentation, captures, VS Code, MSRV, release builds, and cross-target
  compile checks.
- [ ] `MQTT-012` Keep platform claims honest. The Linux builder can cross-check
  Windows compilation and portable harness contracts, but it cannot execute a
  Windows or macOS runner. GitHub's Windows and macOS jobs remain authoritative
  for native execution. Do not merge until those exact-SHA jobs are green.
- [ ] `MQTT-013` Run the remote disk/toolchain/bootstrap preflight, freeze one
  clean SHA, prepare the exact-SHA release-candidate artifact, and push once.
  Any edit invalidates the artifact.
- [ ] `MQTT-014` Wait for all GitHub checks and automated reviews. If anything
  fails, collect the complete failure ledger and logs before editing, repair
  the whole batch, rerun focused tests and all affected parity gates, then
  prepare at most one replacement candidate.
- [ ] `MQTT-015` Merge only through `release_candidate_guard.py check-merge
  --pr 115 --execute`, bound to the reviewed green SHA.
- [ ] `MQTT-016` From the resulting `main` SHA, create and push the annotated
  release tag, monitor main CI and the Release workflow to completion, verify
  GitHub Latest, assets and checksums, and Marketplace propagation, then run
  `release_candidate_guard.py verify-release --candidate-head <reviewed-head>
  --branch fix/mqtt-review-followups`.
- [ ] `MQTT-017` Confirm Issue #111 is closed and post the prepared natural
  reply without naming the reporter and without em dashes.

## Mandatory Cleanup After Every Merge to Main

- [ ] `CLEAN-001` Fetch `origin/main` and record the PR number, validated head
  SHA, merge SHA, resulting main SHA, version, tag, and release URL.
- [ ] `CLEAN-002` Verify the merged content and required release artifacts are
  actually present on `origin/main`; a green PR alone is not release proof.
- [ ] `CLEAN-003` Inventory every local and remote task branch and worktree.
  For each one, prove whether it contains unique commits, staged changes,
  unstaged changes, or untracked files. Path names and commit counts are not
  proof of ownership or uniqueness.
- [ ] `CLEAN-004` Save and verify a recovery patch/bundle for any uncertain or
  still-owned work. Remove only clean, merged, task-owned worktrees and branches
  whose content is proven recoverable. Report exactly what was removed.
- [ ] `CLEAN-005` Return the primary checkout to clean current `main`. Begin the
  next task from a fresh branch based on that exact `origin/main`, then manually
  copy and verify `AGENTS.md` and `.codex/skills` as required by bootstrap.
- [ ] `CLEAN-006` Confirm there is no stale open PR, orphan candidate branch,
  obsolete validation worktree, or release-candidate artifact that could be
  mistaken for current work.
- [ ] `CLEAN-007` Do not mark the merge complete in a handoff until rows
  `CLEAN-001` through `CLEAN-006` are either complete or have an explicit named
  owner and blocker.

## SOLID, KISS, and DRY Acceptance

- [x] Verification reconciliation and MQTT behavior remain separate commits,
  candidates, reviews, and release evidence.
- [ ] MQTT transport, mapping resolution, value coercion, and scheduler
  ownership remain separate responsibilities with no duplicated mapping
  policy.
- [x] No new validator is appended to an already-large multipurpose tooling
  file; new gate behavior has a focused self-test.
- [x] Every retained product behavior has one owning specification and a native
  executable test. Planning and proof metadata cannot substitute for either.
