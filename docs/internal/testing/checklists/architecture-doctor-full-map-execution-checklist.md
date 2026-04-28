# Architecture Doctor Full-Map Execution Checklist

Status: In progress - `--full-map` scaffold, `SoftwareMap` data model, and initial JSON map writer landed.
Owner: Architecture automation
Scope: implement a repeatable `cargo xtask architecture-doctor --full-map` command with source-derived facts, policies, fixtures, and reports.

This checklist is a prerequisite for enforcing the runtime-core split, product/workbench split, host-surface ownership rules, KISS thresholds, API trend checks, and diagram semantic checks.

## Stop Rules

- [ ] `FULLMAP-STOP-01` Do not treat a partial/failed tool as a passing architecture check.
- [ ] `FULLMAP-STOP-02` Do not add a rule without a failing fixture or equivalent known-bad test case.
- [ ] `FULLMAP-STOP-03` Do not trust a diagram claim unless it maps to generated facts or documented manual facts.
- [ ] `FULLMAP-STOP-04` Do not make downstream checklists depend on a full-map check that is not implemented.
- [ ] `FULLMAP-STOP-05` Do not silently widen allowlists; every allowlist entry needs owner, rationale, and review date.

## Phase 0 - MVP Scope Lock

- [x] `FULLMAP-P0-001` Lock MVP command: `cargo xtask architecture-doctor --full-map`.
- [x] `FULLMAP-P0-002` Lock artifact root: `target/gate-artifacts/full-software-map-<date-or-commit>/`.
- [x] `FULLMAP-P0-003` Lock generated JSON path for source facts.
- [ ] `FULLMAP-P0-004` Lock generated Markdown report path.
- [ ] `FULLMAP-P0-005` Define MVP checks required before runtime-core extraction:
  - workspace edge policy,
  - forbidden dependency/import scanner,
  - runtime command/bin-module ownership,
  - host-surface forbidden edges,
  - runtime-core dependency fence when crate exists.
- [ ] `FULLMAP-P0-006` Defer diagram claim checking to a follow-up only if MVP notes exactly which downstream gates remain unavailable.

### Phase 0 MVP Check Aliases

These IDs are referenced by downstream checklists as hard prerequisites.

- [ ] `FULLMAP-CHECK-01` Allowed workspace edge policy exists and is loaded.
- [ ] `FULLMAP-CHECK-02` New workspace edges fail unless explicitly classified.
- [ ] `FULLMAP-CHECK-05` `trust-runtime-core` dependency fence is enforced when the crate exists.
- [ ] `FULLMAP-CHECK-06` Product/workbench runtime command, nested action, and bin-module ownership is enforced.
- [ ] `FULLMAP-CHECK-07` HMI/web/control/cloud forbidden edges are enforced.
- [ ] `FULLMAP-CHECK-08` Dependency hygiene policy status is emitted and failed tools cannot report as pass.
- [ ] `FULLMAP-CHECK-09` Unsafe/concurrency hotspot summary is emitted with owner/status fields.
- [ ] `FULLMAP-CHECK-10` KISS large-file and runtime-host module-count thresholds are enforced.

## Phase 1 - Data Model And JSON Map Writer

- [x] `FULLMAP-P1-001` Define `SoftwareMap` JSON schema or Rust structs.
- [x] `FULLMAP-P1-002` Include workspace packages, targets, target kinds, and package paths from cargo metadata.
- [x] `FULLMAP-P1-003` Include direct workspace dependency edges.
- [ ] `FULLMAP-P1-004` Include crate/module tree summaries.
- [ ] `FULLMAP-P1-005` Include Rust file line counts and largest-file list.
- [x] `FULLMAP-P1-006` Include top-level `trust-runtime` modules.
- [x] `FULLMAP-P1-007` Include `trust-runtime` CLI command variants and bin modules.
- [x] `FULLMAP-P1-007A` Include nested CLI `*Action` enums and their parent command or explicit override.
- [ ] `FULLMAP-P1-008` Include selected import edges from source scans.
- [x] `FULLMAP-P1-009` Include tool result statuses as `pass`, `finding`, `partial`, or `failed`.
- [x] `FULLMAP-P1-010` Add deterministic serialization and stable sorting.
- [x] `FULLMAP-P1-011` Add unit tests for serialization and stable ordering.

## Phase 2 - Policy Loader

- [ ] `FULLMAP-P2-001` Add an allowed workspace edge policy file.
- [ ] `FULLMAP-P2-002` Add forbidden dependency policy for `trust-runtime-core`.
- [ ] `FULLMAP-P2-003` Add runtime command/bin-module ownership policy.
- [ ] `FULLMAP-P2-003A` Add nested CLI action ownership inheritance/override policy.
- [ ] `FULLMAP-P2-004` Add host-surface ownership policy for `web`, `hmi`, `ui`, `control`, and `runtime_cloud`.
- [ ] `FULLMAP-P2-005` Add KISS thresholds:
  - no new `trust-runtime` top-level module without subsystem decision note,
  - no new Rust file over 1,000 lines,
  - no existing Rust file over 1,000 lines without owner/split note,
  - no file over 1,500 lines without an approved split plan or waiver,
  - after the runtime CLI, host-surface, and runtime-core boards complete, `trust-runtime/src` must have no more than 18 top-level host modules unless a dated architecture waiver names the next extraction branch,
  - public API growth requires explicit review once snapshots exist.
- [ ] `FULLMAP-P2-006` Add allowlist format with owner, rationale, and review date.
- [ ] `FULLMAP-P2-007` Add policy parse/validation tests.

## Phase 3 - Workspace Edge And Dependency Checks

- [ ] `FULLMAP-P3-001` Fail new workspace edges not present in allowed-edge policy.
- [ ] `FULLMAP-P3-002` Keep HIR-to-runtime dependency forbidden.
- [ ] `FULLMAP-P3-003` Classify current `trust-runtime -> trust-ide` edge as allowed, temporary, or forbidden.
- [ ] `FULLMAP-P3-004` Classify current `trust-lsp -> trust-runtime` edge as allowed, temporary, or forbidden.
- [ ] `FULLMAP-P3-005` Classify current `trust-debug -> trust-runtime` edge as allowed, temporary, or forbidden.
- [ ] `FULLMAP-P3-006` Fail forbidden `trust-runtime-core` dependencies when the crate exists.
- [ ] `FULLMAP-P3-007` Add fixture or unit test for a forbidden workspace edge.
- [ ] `FULLMAP-P3-008` Add fixture or unit test for a forbidden core dependency.

## Phase 4 - Forbidden Import Scanner

- [ ] `FULLMAP-P4-001` Implement source scanner for direct `use crate::<module>` and `crate::<module>` references in selected crates.
- [ ] `FULLMAP-P4-002` Fail `trust-runtime-core` imports of host-only modules.
- [ ] `FULLMAP-P4-003` Fail product runtime command/module imports of workbench modules.
- [ ] `FULLMAP-P4-004` Fail `control -> web` implementation imports once policy is active.
- [ ] `FULLMAP-P4-005` Fail HMI/web/control/cloud bypass imports once approved ports exist.
- [ ] `FULLMAP-P4-006` Add fixtures or unit tests for each forbidden-import rule.

## Phase 5 - Runtime Command And Bin Module Checks

- [ ] `FULLMAP-P5-001` Parse or scan `Command` enum variants from `crates/trust-runtime/src/bin/trust-runtime/cli/commands.rs`.
- [ ] `FULLMAP-P5-002` Scan top-level bin modules under `crates/trust-runtime/src/bin/trust-runtime/*.rs`.
- [ ] `FULLMAP-P5-003` Fail unclassified command variants.
- [ ] `FULLMAP-P5-004` Fail unclassified bin modules.
- [ ] `FULLMAP-P5-005` Report command-to-module mapping gaps.
- [ ] `FULLMAP-P5-006` Add fixtures or unit tests for unclassified command and unclassified module cases.
- [ ] `FULLMAP-P5-007` Parse or scan nested CLI `*Action` enums and fail unclassified action enums or explicit ownership overrides.

## Phase 6 - KISS And Public API Checks

- [ ] `FULLMAP-P6-001` Report and fail new Rust files over 1,000 lines.
- [ ] `FULLMAP-P6-002` Report and fail existing files over 1,000 lines with no owner/split note.
- [ ] `FULLMAP-P6-003` Report and fail files over 1,500 lines with no approved split plan or waiver.
- [ ] `FULLMAP-P6-004` Report top-level `trust-runtime` module count and fail net growth without subsystem decision note.
- [ ] `FULLMAP-P6-005` Capture public API snapshot when `cargo public-api` is available.
- [ ] `FULLMAP-P6-006` Report public API growth and fail unreviewed growth once baseline exists.
- [ ] `FULLMAP-P6-007` Add tests for threshold evaluation.
- [ ] `FULLMAP-P6-008` Add configurable `max_runtime_host_top_level_modules` policy and fail program-exit checks when the host exceeds the cap without waiver.

## Phase 7 - Diagram Claim Checker

- [ ] `FULLMAP-P7-001` Parse selected PlantUML component names.
- [ ] `FULLMAP-P7-002` Parse selected PlantUML dependency/control/data-flow edges.
- [ ] `FULLMAP-P7-003` Match components to crates/modules/subsystems in the software map.
- [ ] `FULLMAP-P7-004` Match edges to source-derived facts or manual-facts file.
- [ ] `FULLMAP-P7-005` Fail stale component names.
- [ ] `FULLMAP-P7-006` Fail unsupported dependency/control/data-flow claims.
- [ ] `FULLMAP-P7-007` Add known-bad diagram fixture.

## Phase 8 - Report Writer And CI Artifact

- [ ] `FULLMAP-P8-001` Write Markdown report with summary, failures, findings, partial tools, and artifact links.
- [ ] `FULLMAP-P8-002` Write machine-readable JSON summary for CI.
- [ ] `FULLMAP-P8-003` Include exact commands and tool versions where available.
- [ ] `FULLMAP-P8-004` Include remediation hints with file/path evidence.
- [ ] `FULLMAP-P8-005` Add CI artifact upload plan.

## Phase 9 - Acceptance

- [ ] `FULLMAP-ACC-01` `cargo xtask architecture-doctor --full-map` exists.
- [ ] `FULLMAP-ACC-02` Command can run locally from a clean checkout with documented tools.
- [ ] `FULLMAP-ACC-03` Known-bad dependency edge fixture/test fails.
- [ ] `FULLMAP-ACC-04` Known-bad runtime-core forbidden dependency fixture/test fails once core exists.
- [ ] `FULLMAP-ACC-05` Known-bad product/workbench command/module fixture/test fails.
- [ ] `FULLMAP-ACC-06` Known-bad host-surface forbidden import fixture/test fails.
- [ ] `FULLMAP-ACC-07` Known-bad KISS threshold fixture/test fails.
- [ ] `FULLMAP-ACC-08` Known-bad diagram claim fixture/test fails if diagram checker is in MVP.
- [ ] `FULLMAP-ACC-09` Generated report is stable enough for CI artifact comparison.
- [ ] `FULLMAP-ACC-10` Runtime split checklist no longer depends on a missing automation command.
