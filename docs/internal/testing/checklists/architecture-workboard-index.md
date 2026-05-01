# Architecture Workboard Index

Status: Active navigation guard
Last verified: 2026-05-01 after starting BOARD-10 runtime VM mutation command locking.
Owner: Architecture/runtime/HIR team
Scope: reset-safe pointer to the right architecture checklists so future sessions do not swap boards.

Unchecked `ARCHIDX-*` rows are recurring guard checks for each resume, not architecture board tasks to close.

## Source Of Truth

- [ ] `ARCHIDX-RULE-01` Start architecture-program work from `full-architecture-refactor-program-checklist.md`.
- [ ] `ARCHIDX-RULE-02` Treat each board's dedicated checklist as the owner of detailed tasks and exit evidence.
- [ ] `ARCHIDX-RULE-03` Do not use ignored or untracked local files as board authority. `docs/internal/masterPlan.md` currently exists locally but is ignored by `.gitignore`; for this architecture program it is not the sequencing source of truth.
- [ ] `ARCHIDX-RULE-04` Before accepting an external board map, re-check the current branch with `git status --short --branch` and the tracked checklist rows.
- [ ] `ARCHIDX-RULE-05` Do not tick an umbrella board row until the dedicated board status, exit criteria, merge evidence, and release/tag gate match.

## Current Architecture Program Map

The umbrella checklist is `full-architecture-refactor-program-checklist.md`. If this snapshot drifts, the tracked umbrella row and tracked dedicated board status win.

- [x] `ARCHPROG-BOARD-01` Full-map architecture doctor: `architecture-doctor-full-map-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-02` HIR mutation hardening: `hir-mutation-hardening-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-03` Parser recovery hardening: `parser-recovery-hardening-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-04` Runtime CLI product/workbench split: `runtime-cli-product-workbench-split-checklist.md`. Status: complete; `trust-dev` owns agent/commit/docs/test, `trust-runtime` keeps deprecated forwarding aliases, and shared helpers have explicit infrastructure rationales.
- [x] `ARCHPROG-BOARD-05` Runtime host surface ownership: `runtime-host-surface-ownership-checklist.md`.
- [x] `ARCHPROG-BOARD-06` Runtime core/Linux host split: `runtime-core-host-split-execution-checklist.md`. Status: closed in the current tracked branch.
- [x] `ARCHPROG-BOARD-07` Dependency hygiene: `dependency-hygiene-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-08` Runtime large-file split: `runtime-large-file-split-execution-checklist.md`. Status: complete; `FULLMAP-CHECK-10` reports zero remaining runtime `src` or runtime `tests` files over 1,000 lines after all measured BOARD-08 hotspots were split.
- [x] `ARCHPROG-BOARD-09` Diagram semantic enforcement: covered by the full-map doctor work.
- [ ] `ARCHPROG-BOARD-10` Runtime VM mutation hardening: `runtime-vm-mutation-hardening-execution-checklist.md`. Status: in progress; Phase 0 command lock captured with `scripts/runtime_vm_mutation_shards.sh`.
- [ ] `ARCHPROG-BOARD-11` Unsafe/concurrency hardening: `unsafe-concurrency-hardening-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-12` HIR zero-silent-bug refactor: `hir-zero-silent-bug-refactor-checklist.md`.

## Work Order

- [x] `ARCHIDX-NEXT-01` BOARD-04 is complete in the tracked checklist; do not resume it unless a new explicit scope reopens `runtime-cli-product-workbench-split-checklist.md`.
- [x] `ARCHIDX-NEXT-02` After the `v0.24.12` BOARD-04 release gate, use the umbrella checklist to pick the next open architecture-program row. Evidence: BOARD-08 was selected from the tracked umbrella checklist after BOARD-04 completion.
- [x] `ARCHIDX-NEXT-05` Continue the active BOARD-08 work in `runtime-large-file-split-execution-checklist.md`; completed `RTLARGE-HOT-07` (`crates/trust-runtime/src/bin/trust-dev/agent.rs`) by splitting harness helpers into `trust-dev/agent/harness.rs`.
- [x] `ARCHIDX-NEXT-06` Continue active BOARD-08 work in `runtime-large-file-split-execution-checklist.md`; completed `RTLARGE-HOT-09` (`crates/trust-runtime/src/runtime/vm/register_ir/lower.rs`) by extracting decoder, fusion, and verifier helpers.
- [x] `ARCHIDX-NEXT-07` Continue active BOARD-08 work in `runtime-large-file-split-execution-checklist.md`; completed `RTLARGE-HOT-08` (`crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09.rs`) by splitting scenario groups while preserving all test names.
- [x] `ARCHIDX-NEXT-08` Continue active BOARD-08 work in `runtime-large-file-split-execution-checklist.md`; completed `RTLARGE-HOT-12` (`crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs`) by splitting state/cache, compile lowering, and compiled execution into child modules.
- [x] `ARCHIDX-NEXT-09` Continue with `ARCHPROG-BOARD-10` Runtime VM mutation hardening (`runtime-vm-mutation-hardening-execution-checklist.md`) unless the user redirects. Evidence: Phase 0 command lock started with `scripts/runtime_vm_mutation_shards.sh --list`.
- [ ] `ARCHIDX-NEXT-10` Continue BOARD-10 Phase 1 baseline mutation runs from `runtime-vm-mutation-hardening-execution-checklist.md`.
- [ ] `ARCHIDX-NEXT-03` Do not restart BOARD-06 or BOARD-12 unless the tracked dedicated checklist is reopened with a new explicit scope.
- [ ] `ARCHIDX-NEXT-04` Keep non-program boards secondary unless the user explicitly redirects away from the architecture-program path.

## Reset Procedure

- [ ] `ARCHIDX-RESET-01` Run `git status --short --branch`.
- [ ] `ARCHIDX-RESET-02` Confirm the active branch and whether it contains the last recorded release/merge commit.
- [ ] `ARCHIDX-RESET-03` Read this file and `full-architecture-refactor-program-checklist.md` before reading any dedicated board.
- [ ] `ARCHIDX-RESET-04` Read only the dedicated checklist for the next open board unless a dependency row points elsewhere.
- [ ] `ARCHIDX-RESET-05` If a version bump has been merged, confirm the tag, release workflow, and GitHub release before moving to the next board.
- [ ] `ARCHIDX-RESET-06` If a local ignored checklist conflicts with tracked checklists, ignore the local file and record the conflict before continuing.

## Validation Cadence Reminder

- [ ] `ARCHIDX-VAL-01` Use focused tests and doctor checks during implementation.
- [ ] `ARCHIDX-VAL-02` Reserve `just test-all` for merge/release readiness, board-completion gates, large cross-crate refactors, or rebases that touch shared APIs.
- [ ] `ARCHIDX-VAL-03` Measure before optimizing test speed or process count.
- [ ] `ARCHIDX-VAL-04` Write a failing behavior-lock test before fixing a behavior bug.
