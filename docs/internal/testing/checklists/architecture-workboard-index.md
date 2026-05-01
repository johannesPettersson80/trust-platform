# Architecture Workboard Index

Status: Active navigation guard
Last verified: 2026-05-01 on `main` at `f6e8895e0` / release `v0.24.9`.
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
- [ ] `ARCHPROG-BOARD-04` Runtime CLI product/workbench split: `runtime-cli-product-workbench-split-checklist.md`. Status: active next board; Phase 1 inventory captured.
- [x] `ARCHPROG-BOARD-05` Runtime host surface ownership: `runtime-host-surface-ownership-checklist.md`.
- [x] `ARCHPROG-BOARD-06` Runtime core/Linux host split: `runtime-core-host-split-execution-checklist.md`. Status: closed in the current tracked branch.
- [x] `ARCHPROG-BOARD-07` Dependency hygiene: `dependency-hygiene-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-08` Runtime large-file split: `runtime-large-file-split-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-09` Diagram semantic enforcement: covered by the full-map doctor work.
- [ ] `ARCHPROG-BOARD-10` Runtime VM mutation hardening: `runtime-vm-mutation-hardening-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-11` Unsafe/concurrency hardening: `unsafe-concurrency-hardening-execution-checklist.md`.
- [x] `ARCHPROG-BOARD-12` HIR zero-silent-bug refactor: `hir-zero-silent-bug-refactor-checklist.md`.

## Work Order

- [ ] `ARCHIDX-NEXT-01` Continue `ARCHPROG-BOARD-04` first: `runtime-cli-product-workbench-split-checklist.md`.
- [ ] `ARCHIDX-NEXT-02` After BOARD-04, use the umbrella checklist to pick the next open architecture-program row.
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
