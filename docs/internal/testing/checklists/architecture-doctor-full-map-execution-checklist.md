# Architecture Doctor Full-Map Execution Checklist

Status: Done - `cargo xtask architecture-doctor --full-map` writes source-derived JSON/Markdown reports and enforces the MVP policy checks with known-bad unit tests.
Owner: Architecture automation
Scope: implement a repeatable `cargo xtask architecture-doctor --full-map` command with source-derived facts, policies, fixtures, and reports.

This checklist is a prerequisite for enforcing the runtime-core split, product/workbench split, host-surface ownership rules, KISS thresholds, API trend checks, and diagram semantic checks.
It also runs the HIR zero-silent-bug doctor as a full-map gate after the HIR semantic-kernel board closes.

## Stop Rules

- [x] `FULLMAP-STOP-01` Do not treat a partial/failed tool as a passing architecture check.
- [x] `FULLMAP-STOP-02` Do not add a rule without a failing fixture or equivalent known-bad test case.
- [x] `FULLMAP-STOP-03` Do not trust a diagram claim unless it maps to generated facts or documented manual facts.
- [x] `FULLMAP-STOP-04` Do not make downstream checklists depend on a full-map check that is not implemented.
- [x] `FULLMAP-STOP-05` Do not silently widen allowlists; every allowlist entry needs owner, rationale, and review date.

## Validation Cadence

- [x] `FULLMAP-VAL-01` Implementation-loop checks are focused: `cargo test -p xtask`, `cargo clippy -p xtask --all-targets -- -D warnings`, and `cargo run -p xtask -- architecture-doctor --full-map`.
- [x] `FULLMAP-VAL-02` Run `just fmt` and `just clippy` before committing full-map doctor implementation changes.
- [x] `FULLMAP-VAL-03` Run `just test-all` only at strategic gates: before merge, release/customer-facing readiness, after large cross-crate changes, after risky rebases, or before marking the board complete.
- [x] `FULLMAP-VAL-04` Long suites that are not affected by full-map doctor code, such as OSCAT example sweeps, are milestone evidence rather than the default edit loop.

## Phase 0 - MVP Scope Lock

- [x] `FULLMAP-P0-001` Lock MVP command: `cargo xtask architecture-doctor --full-map`.
- [x] `FULLMAP-P0-002` Lock artifact root: `target/gate-artifacts/full-software-map-<date-or-commit>/`.
- [x] `FULLMAP-P0-003` Lock generated JSON path for source facts.
- [x] `FULLMAP-P0-004` Lock generated Markdown report path.
- [x] `FULLMAP-P0-005` Define MVP checks required before runtime-core extraction:
  - workspace edge policy,
  - forbidden dependency/import scanner,
  - runtime command/bin-module ownership,
  - host-surface forbidden edges,
  - runtime-core dependency fence when crate exists.
- [x] `FULLMAP-P0-006` Defer diagram claim checking to a follow-up only if MVP notes exactly which downstream gates remain unavailable.

### Phase 0 MVP Check Aliases

These IDs are referenced by downstream checklists as hard prerequisites.

- [x] `FULLMAP-CHECK-01` Allowed workspace edge policy exists and is loaded.
- [x] `FULLMAP-CHECK-02` New workspace edges fail unless explicitly classified.
- [x] `FULLMAP-CHECK-05` `trust-runtime-core` dependency fence is enforced when the crate exists.
- [x] `FULLMAP-CHECK-06` Product/workbench runtime command, nested action, and bin-module ownership is enforced.
- [x] `FULLMAP-CHECK-07` HMI/web/control/cloud forbidden direct edges are enforced, and approved-port bypass enforcement reports `partial` until ports exist.
- [x] `FULLMAP-CHECK-08` Dependency hygiene policy status is emitted and failed tools cannot report as pass.
- [x] `FULLMAP-CHECK-09` Unsafe/concurrency hotspot summary is emitted with owner/status fields and reports a finding while hotspots remain nonzero.
- [x] `FULLMAP-CHECK-10` KISS large-file and runtime-host module-count thresholds are enforced.
- [x] `FULLMAP-HIRZSB` HIR zero-silent-bug doctor runs with `--fail` and fails full-map when HIR broad lookup, sentinel return, duplicated resolver, silent discard, allowlist, public raw API, or runtime declaration-bypass findings appear.

## Phase 1 - Data Model And JSON Map Writer

- [x] `FULLMAP-P1-001` Define `SoftwareMap` JSON schema or Rust structs.
- [x] `FULLMAP-P1-002` Include workspace packages, targets, target kinds, and package paths from cargo metadata.
- [x] `FULLMAP-P1-003` Include direct workspace dependency edges.
- [x] `FULLMAP-P1-004` Include crate/module tree summaries.
- [x] `FULLMAP-P1-005` Include Rust file line counts and largest-file list.
- [x] `FULLMAP-P1-006` Include top-level `trust-runtime` modules.
- [x] `FULLMAP-P1-007` Include `trust-runtime` CLI command variants and bin modules.
- [x] `FULLMAP-P1-007A` Include nested CLI `*Action` enums and their parent command or explicit override.
- [x] `FULLMAP-P1-008` Include selected import edges from source scans.
- [x] `FULLMAP-P1-009` Include tool result statuses as `pass`, `finding`, `partial`, or `failed`.
- [x] `FULLMAP-P1-010` Add deterministic serialization and stable sorting.
- [x] `FULLMAP-P1-011` Add unit tests for serialization and stable ordering.

## Phase 2 - Policy Loader

- [x] `FULLMAP-P2-001` Add an allowed workspace edge policy file.
- [x] `FULLMAP-P2-002` Add forbidden dependency policy for `trust-runtime-core`.
- [x] `FULLMAP-P2-003` Add runtime command/bin-module ownership policy.
- [x] `FULLMAP-P2-003A` Add nested CLI action ownership inheritance/override policy.
- [x] `FULLMAP-P2-004` Add host-surface ownership policy for `web`, `hmi`, `ui`, `control`, and `runtime_cloud`.
- [x] `FULLMAP-P2-005` Add KISS thresholds:
  - no new `trust-runtime` top-level module without subsystem decision note,
  - no new Rust file over 1,000 lines,
  - no existing Rust file over 1,000 lines without owner/split note,
  - no file over 1,500 lines without an approved split plan or waiver,
  - after the runtime CLI, host-surface, and runtime-core boards complete, `trust-runtime/src` must have no more than 18 top-level host modules unless a dated architecture waiver names the next extraction branch,
  - public API growth requires explicit review once snapshots exist.
- [x] `FULLMAP-P2-006` Add allowlist format with owner, rationale, and review date.
- [x] `FULLMAP-P2-007` Add policy parse/validation tests.

## Phase 3 - Workspace Edge And Dependency Checks

- [x] `FULLMAP-P3-001` Fail new workspace edges not present in allowed-edge policy.
- [x] `FULLMAP-P3-002` Keep HIR-to-runtime dependency forbidden.
- [x] `FULLMAP-P3-003` Classify current `trust-runtime -> trust-ide` edge as allowed, temporary, or forbidden.
- [x] `FULLMAP-P3-004` Classify current `trust-lsp -> trust-runtime` edge as allowed, temporary, or forbidden.
- [x] `FULLMAP-P3-005` Classify current `trust-debug -> trust-runtime` edge as allowed, temporary, or forbidden.
- [x] `FULLMAP-P3-006` Fail forbidden `trust-runtime-core` dependencies when the crate exists.
- [x] `FULLMAP-P3-007` Add fixture or unit test for a forbidden workspace edge.
- [x] `FULLMAP-P3-008` Add fixture or unit test for a forbidden core dependency.

## Phase 4 - Forbidden Import Scanner

- [x] `FULLMAP-P4-001` Implement source scanner for direct `use crate::<module>` and `crate::<module>` references in selected crates.
- [x] `FULLMAP-P4-002` Fail `trust-runtime-core` imports of host-only modules.
- [x] `FULLMAP-P4-003` Fail product runtime command/module imports of workbench modules.
- [x] `FULLMAP-P4-004` Fail `control -> web` implementation imports once policy is active.
- [x] `FULLMAP-P4-005` Fail HMI/web/control/cloud bypass imports once approved ports exist.
- [x] `FULLMAP-P4-006` Add fixtures or unit tests for each forbidden-import rule.

## Phase 5 - Runtime Command And Bin Module Checks

- [x] `FULLMAP-P5-001` Parse or scan `Command` enum variants from `crates/trust-runtime/src/bin/trust-runtime/cli/commands.rs`.
- [x] `FULLMAP-P5-002` Scan top-level bin modules under `crates/trust-runtime/src/bin/trust-runtime/*.rs`.
- [x] `FULLMAP-P5-003` Fail unclassified command variants.
- [x] `FULLMAP-P5-004` Fail unclassified bin modules.
- [x] `FULLMAP-P5-005` Report command-to-module mapping gaps and named route metadata for commands intentionally dispatched through another module.
- [x] `FULLMAP-P5-006` Add fixtures or unit tests for unclassified command and unclassified module cases.
- [x] `FULLMAP-P5-007` Parse or scan nested CLI `*Action` enums and fail unclassified action enums or explicit ownership overrides.
- [x] `FULLMAP-P5-008` Verify policy-declared command route handlers against source symbols so stale route rationale cannot pass silently.

## Phase 6 - KISS And Public API Checks

- [x] `FULLMAP-P6-001` Report and fail new Rust files over 1,000 lines.
- [x] `FULLMAP-P6-002` Report and fail existing files over 1,000 lines with no owner/split note.
- [x] `FULLMAP-P6-003` Report and fail files over 1,500 lines with no approved split plan or waiver.
- [x] `FULLMAP-P6-004` Report top-level `trust-runtime` module count and fail net growth without subsystem decision note.
- [x] `FULLMAP-P6-005` Capture public API snapshot when `cargo public-api` is available.
- [x] `FULLMAP-P6-006` Report public API growth and fail unreviewed growth once baseline exists.
- [x] `FULLMAP-P6-007` Add tests for threshold evaluation.
- [x] `FULLMAP-P6-008` Add configurable `max_runtime_host_top_level_modules` policy and fail program-exit checks when the host exceeds the cap without waiver.

## Phase 7 - Diagram Claim Checker

- [x] `FULLMAP-P7-001` Parse selected PlantUML component names.
- [x] `FULLMAP-P7-002` Parse selected PlantUML dependency/control/data-flow edges.
- [x] `FULLMAP-P7-003` Match components to crates/modules/subsystems in the software map.
- [x] `FULLMAP-P7-004` Match edges to source-derived facts or manual-facts file.
- [x] `FULLMAP-P7-005` Fail stale component names.
- [x] `FULLMAP-P7-006` Fail unsupported dependency/control/data-flow claims.
- [x] `FULLMAP-P7-007` Add known-bad diagram fixture.

## Phase 8 - Report Writer And CI Artifact

- [x] `FULLMAP-P8-001` Write Markdown report with summary, failures, findings, partial tools, and artifact links.
- [x] `FULLMAP-P8-002` Write machine-readable JSON summary for CI.
- [x] `FULLMAP-P8-003` Include exact commands and tool versions where available.
- [x] `FULLMAP-P8-004` Include remediation hints with file/path evidence.
- [x] `FULLMAP-P8-005` Add CI artifact upload plan.

CI artifact plan: CI should run `cargo xtask architecture-doctor --full-map` and upload `target/gate-artifacts/full-software-map-*/software-map.json`, `full-map-report.json`, and `full-map-report.md` as architecture-doctor artifacts.

## Phase 9 - Acceptance

- [x] `FULLMAP-ACC-01` `cargo xtask architecture-doctor --full-map` exists.
- [x] `FULLMAP-ACC-02` Command can run locally from a clean checkout with documented tools.
- [x] `FULLMAP-ACC-03` Known-bad dependency edge fixture/test fails.
- [x] `FULLMAP-ACC-04` Known-bad runtime-core forbidden dependency fixture/test fails once core exists.
- [x] `FULLMAP-ACC-05` Known-bad product/workbench command/module fixture/test fails.
- [x] `FULLMAP-ACC-06` Known-bad host-surface forbidden import fixture/test fails.
- [x] `FULLMAP-ACC-07` Known-bad KISS threshold fixture/test fails.
- [x] `FULLMAP-ACC-08` Known-bad diagram claim fixture/test fails if diagram checker is in MVP.
- [x] `FULLMAP-ACC-09` Generated report is stable enough for CI artifact comparison.
- [x] `FULLMAP-ACC-10` Runtime split checklist no longer depends on a missing automation command.
