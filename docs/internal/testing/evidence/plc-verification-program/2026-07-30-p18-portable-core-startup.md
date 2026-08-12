# Phase 18 portable runtime-core and startup authority evidence

Date: 2026-07-30

Scope: focused `VERIF-P18-003`, `VERIF-P18-005`, `VERIF-P18-006`, and
`VERIF-P18-004` progress only. This evidence does not close those rows
repository-wide and does not claim mutation, adequacy, environment, or broad
candidate proof.

## Authority and implemented proof

The portable runtime-core batches established or refined authority for:

- elementary and compound default construction;
- recursive value construction and declared-type matching;
- watchdog configuration parsing, inert defaults, and fault identity;
- civil-date conversion, profile resolution, and signed tick conversion;
- LValue index, field, and dereference projections;
- stable runtime error identities and conversions;
- numeric-kind widening, initializer catalog behavior, new task state, and
  declared/runtime `SIZEOF` behavior.

The corresponding product authority is in
`docs/specs/10-runtime-semantics.md`, with focused reviewed invariants under
`verification/invariants/bytecode_vm/` and
`verification/invariants/runtime_safety/`. Focused tests retire 68 live
`spec_gap` code facts across those batches.

The runtime-startup batch then expanded
`docs/specs/11-runtime-engine.md` and
`RT_RUN_STARTUP_POLICY_001` to own:

- legacy-source, project-bundle, and IDE bootstrap loading;
- accepted bundle bytecode and runtime-policy overrides;
- disabled ADS and OPC UA startup as transport-free no-ops;
- fail-closed enabled protocol startup when required sidecar configuration is
  absent;
- canonical structured-log severity threshold and level identity;
- bounded structured SIGINT/SIGTERM shutdown evidence.

Five new tests implement that missing startup proof:

- `runtime_load_modes_compile_legacy_sources_and_ide_bootstrap`
- `accepted_bundle_overrides_apply_runtime_policies_and_valid_bytecode`
- `ads_runtime_start_is_disabled_noop_and_missing_config_is_fail_closed`
- `opcua_runtime_start_is_disabled_noop_and_missing_config_is_fail_closed`
- `runtime_logger_threshold_uses_canonical_severity_order`

Two existing child-process tests were strengthened to parse every emitted JSON
record and require the timestamp, canonical level, event, data object, exact
debug exit message, and stopped status:

- `sigint_requests_bounded_graceful_stop_in_child_runtime`
- `sigterm_requests_bounded_graceful_stop_in_child_runtime`

No startup production defect was exposed. One test-authoring compile error used
a nonexistent `FaultPolicy` variant; it was corrected before execution and was
not counted as an expected-red result.

The datetime batch did expose two production defects through focused
assertions: extreme civil years could overflow intermediate `i64` arithmetic,
and resolutions that could not represent an integral day were accepted. Civil
conversion now uses checked `i128` intermediates and profile conversion rejects
zero, negative, over-day, and non-dividing resolutions.

## Focused remote-builder proof

All compilation, execution, and coverage evidence below ran on
`scena-rust-builder` in the detached validation checkout
`/home/johannes/projects/trust-platform-p18-hir-b2ff0fbd`.

- Four default-feature startup unit tests passed 4/4.
- The OPC UA startup test passed 1/1 with `opcua-wire`.
- The two strengthened signal-smoke tests passed 2/2.
- The final exact startup trace executed and observed 8/8 selected tests. It
  combined the five new tests with the existing project-settings,
  project-dependency, and simulation-plan locks.
- The automatic semantic and confirmation review catalogued all five new test
  identities. Three already-catalogued tests retained their existing reviewed
  authority.
- The reviewed trace-to-authority transaction moved all 17 selected startup
  code facts from `spec_gap` to `specified`.

The canonical focused commands are recorded in
`verification/test-catalog.toml`; the startup trace is retained at
`target/gate-artifacts/verification/p18-startup-core-trace.json` on the
builder.

## Post-batch accounting

The clean post-batch report at
`target/gate-artifacts/verification/p18-startup-core-final-report/` is bound to
the detached validation commit and reports:

- 17,232 live code facts;
- 3,096 `specified`;
- 14,101 remaining `spec_gap`;
- all 17 startup-selected facts `specified`;
- 4,440 catalog-mapped tests out of the 4,457-fact frozen test denominator;
- 17 remaining `reviewed_nonmapping` test facts.

The report retains two pre-existing findings: both code facts mapped to
`RT_RETAIN_SNAPSHOT_ROUND_TRIP_001` still lack a linked mapped test in that
invariant. They remain explicit debt for the next authority batch.

No broad `just fmt`, `just clippy`, `just test`, or `just test-all` gate ran.
Those gates remain reserved for the exact frozen `VERIF-P18-016` candidate.
