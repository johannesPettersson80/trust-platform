# Full Architecture Refactor Program Checklist

Status: Planned
Owner: Architecture/runtime/HIR team
Scope: execution program for the full software-map audit findings, SOLID/KISS cleanup, and zero-silent-bug posture.

This is the umbrella checklist. Individual execution boards own the detailed work. A single runtime-core split is not enough to satisfy the architecture goal.

## Program Rule

- [ ] `ARCHPROG-RULE-01` Do not claim "0 silent bugs" from behavior-preserving refactors alone.
- [ ] `ARCHPROG-RULE-02` Do not claim "clean SOLID/KISS" while `trust-runtime` still mixes product runtime, workbench tooling, HMI/web/control/cloud ownership, and unchecked large-file hotspots.
- [ ] `ARCHPROG-RULE-03` Every architecture claim must be backed by source-derived facts, a doctor rule, mutation/fuzz evidence, or a documented manual exception.
- [ ] `ARCHPROG-RULE-04` Each branch must state which audit finding it closes and which findings remain open.
- [ ] `ARCHPROG-RULE-05` Do not merge a refactor branch that weakens an existing behavior lock, doctor rule, or generated-map check.

## Required Execution Boards

- [ ] `ARCHPROG-BOARD-01` Full-map architecture doctor: `architecture-doctor-full-map-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-02` HIR mutation hardening: `hir-mutation-hardening-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-03` Parser recovery hardening: `parser-recovery-hardening-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-04` Runtime CLI product/workbench split: `runtime-cli-product-workbench-split-checklist.md`.
- [ ] `ARCHPROG-BOARD-05` Runtime host surface ownership: `runtime-host-surface-ownership-checklist.md`.
- [ ] `ARCHPROG-BOARD-06` Runtime core/Linux host split: `runtime-core-host-split-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-07` Dependency hygiene: `dependency-hygiene-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-08` Runtime large-file split: `runtime-large-file-split-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-09` Diagram semantic enforcement is added before diagrams are trusted as acceptance evidence.
- [ ] `ARCHPROG-BOARD-10` Runtime VM mutation hardening: `runtime-vm-mutation-hardening-execution-checklist.md`.
- [ ] `ARCHPROG-BOARD-11` Unsafe/concurrency hardening: `unsafe-concurrency-hardening-execution-checklist.md`.

## Recommended Order

### Phase A - Automation First, But Only The Needed Automation

- [ ] `ARCHPROG-A-01` Implement `architecture-doctor --full-map` enough to enforce branch-relevant boundaries.
- [ ] `ARCHPROG-A-02` Add generated/report artifact output so reviewers can inspect the software map.
- [ ] `ARCHPROG-A-03` Add forbidden-edge and forbidden-import policy loading.
- [ ] `ARCHPROG-A-04` Add size/API trend reporting with blocking thresholds for configured KISS gates; public API growth is advisory only until its baseline snapshot exists.
- [ ] `ARCHPROG-A-05` Do not spend this phase building dashboards that do not block a real architecture risk.

### Phase B - Silent-Bug Hardening Before Large Runtime Movement

- [ ] `ARCHPROG-B-01` Close HIR mutation gap for `symbol_import`.
- [ ] `ARCHPROG-B-02` Close HIR mutation gap for `type_check::const_eval`.
- [ ] `ARCHPROG-B-03` Close HIR mutation gap for aggregate initializer validation.
- [ ] `ARCHPROG-B-04` Add mutation gate with zero unexplained survivors for the focused HIR shard.
- [ ] `ARCHPROG-B-05` Fix parser recovery bounded-scanner and fuzz/property tests.

### Phase C - Runtime Boundary Policy Before Runtime Extraction

- [ ] `ARCHPROG-C-01` Classify runtime binary commands as product, UI product, conformance/benchmark, or workbench/dev.
- [ ] `ARCHPROG-C-02` Classify `web`, `hmi`, `ui`, `control`, and `runtime_cloud` ownership.
- [ ] `ARCHPROG-C-03` Add doctor rules for product/workbench command boundaries.
- [ ] `ARCHPROG-C-04` Add doctor rules for host-surface forbidden imports and approved ports.
- [ ] `ARCHPROG-C-05` Freeze "no new top-level runtime module without subsystem decision note".

### Phase D - Runtime Core/Linux Host Split

- [ ] `ARCHPROG-D-01` Run `runtime-core-host-split-execution-checklist.md` only after Phases A-C have the required rules or explicit waivers.
- [ ] `ARCHPROG-D-02` Treat this phase as behavior-preserving; no embedded product support claims.
- [ ] `ARCHPROG-D-03` Keep behavior-lock tests ahead of code movement.
- [ ] `ARCHPROG-D-04` Keep host crate responsibility shrinkage visible, not hidden behind re-exports.

### Phase E - Remaining Runtime Host Cleanup

- [ ] `ARCHPROG-E-01` Split workbench/dev command implementation after compatibility policy is decided.
- [ ] `ARCHPROG-E-02` Split HMI/web/control/cloud surfaces behind ports/adapters.
- [ ] `ARCHPROG-E-03` Add owner/split notes for every runtime Rust file over 1,000 lines.
- [ ] `ARCHPROG-E-04` Add KISS gates for module size, function size, public API growth, and top-level module growth.
- [ ] `ARCHPROG-E-05` Add runtime VM mutation gate before claiming zero silent bugs for runtime execution.
- [ ] `ARCHPROG-E-06` Add unsafe/concurrency risk register and focused Miri/sanitizer/Loom/Valgrind evidence before claiming memory/concurrency safety.

## Program Exit Criteria

- [ ] `ARCHPROG-EXIT-01` Full-map doctor runs and blocks known bad dependency/ownership patterns.
- [ ] `ARCHPROG-EXIT-02` Focused HIR mutation shard has zero unexplained survivors.
- [ ] `ARCHPROG-EXIT-03` Parser recovery has bounded scanner API plus fuzz/property coverage.
- [ ] `ARCHPROG-EXIT-04` Product runtime binary no longer owns unclassified workbench/dev commands.
- [ ] `ARCHPROG-EXIT-05` HMI/web/control/cloud ownership is enforced by ports and doctor rules.
- [ ] `ARCHPROG-EXIT-06` `trust-runtime-core` owns portable execution and blocks host-only dependencies.
- [ ] `ARCHPROG-EXIT-07` Every runtime Rust file over 1,000 lines has an owner/split note; every file over 1,500 lines has an approved split plan, completed split, or dated waiver.
- [ ] `ARCHPROG-EXIT-08` Diagrams are source-checked, not only render-fresh.
- [ ] `ARCHPROG-EXIT-09` Final report states what is fixed, what remains risky, and which gates prove each claim.
- [ ] `ARCHPROG-EXIT-10` Runtime VM mutation shard has zero unexplained survivors or a documented equivalent-mutant list.
- [ ] `ARCHPROG-EXIT-11` `trust-runtime/src` host top-level module count is at or below the configured full-map cap after CLI, host-surface, and runtime-core boards complete, or a dated waiver names the next extraction branch.
- [ ] `ARCHPROG-EXIT-12` Unsafe/concurrency register is complete and focused Miri/sanitizer/Loom/Valgrind evidence or exact blockers are attached.
