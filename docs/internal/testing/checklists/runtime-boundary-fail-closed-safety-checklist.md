# Runtime Boundary Fail-Closed Safety Checklist

Status: Complete
Owner: runtime/harness-control
Scope: prevent external runtime observation/control surfaces from silently defaulting, inventing, or skipping values for unresolved names.

Contract: `docs/internal/architecture/runtime-boundary-fail-closed-contract.md`.

## Stop Rules

- [x] `RUNTIMEBOUND-STOP-01` Do not return `ok=true` for an unresolved external watch/control name. Evidence: `HarnessWatchSnapshot` now stores per-entry `BoundaryEntry` values and `trust-harness` protocol v2 emits `status:"error"` for unresolved watch paths.
- [x] `RUNTIMEBOUND-STOP-02` Do not silently create globals from boundary writes. Evidence: `TestHarness::try_set_input` delegates to `boundary::resolve_write`; `set_input_typo_returns_boundary_error_not_silent_global_create` passes.
- [x] `RUNTIMEBOUND-STOP-03` Do not silently bind undeclared direct I/O names. Evidence: `bind_direct_typo_returns_boundary_error_not_silent_binding` passes.
- [x] `RUNTIMEBOUND-STOP-04` Do not silently skip top-level `PROGRAM` declarations when a `CONFIGURATION` exists. Evidence: `configuration_without_program_instance_errors_by_default` passes.
- [x] `RUNTIMEBOUND-STOP-05` Do not treat debug/DAP stale-frame or unresolved-eval paths as successful reads. Evidence: `debug_boundary_requests_fail_closed_for_stale_or_missing_names` and `debug_boundary_io_snapshot_poison_is_an_error` pass.
- [x] `RUNTIMEBOUND-STOP-06` Do not drop web/HMI serialization failures without a structured error frame or HTTP error response. Evidence: `hmi_ws::tests::hmi_control_error_payload_is_structured` passes and `FULLMAP-RUNTIMEBOUND` scans HMI/Web fallback sites.

## Phase 0 - Contract And Guardrail

- [x] `RUNTIMEBOUND-P0-001` Add runtime boundary fail-closed contract. Evidence: `docs/internal/architecture/runtime-boundary-fail-closed-contract.md`.
- [x] `RUNTIMEBOUND-P0-002` Add narrow allowlist file. Evidence: `docs/internal/architecture/runtime-boundary-fail-closed-allowlist.toml`.
- [x] `RUNTIMEBOUND-P0-003` Add runtime boundary fail-closed gate script. Evidence: `scripts/runtime_boundary_fail_closed_ast_grep_gate.sh`.
- [x] `RUNTIMEBOUND-P0-004` Surface the gate through architecture doctor. Evidence: `FULLMAP-RUNTIMEBOUND` in `xtask/src/full_map.rs`.
- [x] `RUNTIMEBOUND-P0-005` Wire the gate into CI architecture safety. Evidence: `.github/workflows/ci.yml`.

## Phase 1 - Boundary Error And Resolver

- [x] `RUNTIMEBOUND-P1-001` Add `BoundaryError` taxonomy with stable codes. Evidence: `crates/trust-runtime/src/host/boundary/error.rs`.
- [x] `RUNTIMEBOUND-P1-002` Add `BoundaryEntry` envelope model. Evidence: `crates/trust-runtime/src/host/boundary/protocol_envelope.rs`.
- [x] `RUNTIMEBOUND-P1-003` Add shared read/write/bind resolver. Evidence: `crates/trust-runtime/src/host/boundary/resolver.rs`.
- [x] `RUNTIMEBOUND-P1-004` Reuse helper-eval for dotted/indexed reads/writes instead of duplicating path evaluation. Evidence: resolver calls `eval_storage_expr_with_stdlib` and `write_storage_lvalue`.
- [x] `RUNTIMEBOUND-P1-005` Add focused resolver tests. Evidence: `cargo test -p trust-runtime --test boundary_resolver` passed.

## Phase 2 - Harness Fail-Closed Migration

- [x] `RUNTIMEBOUND-P2-001` Remove silent `set_input` global creation. Evidence: `set_input_typo_returns_boundary_error_not_silent_global_create` passed.
- [x] `RUNTIMEBOUND-P2-002` Remove silent `bind_direct` fallback binding. Evidence: `bind_direct_typo_returns_boundary_error_not_silent_binding` passed.
- [x] `RUNTIMEBOUND-P2-003` Keep declared null-like values distinct from missing names. Evidence: `declared_null_like_values_are_not_missing_name_errors` passed.
- [x] `RUNTIMEBOUND-P2-004` Convert watch snapshots to per-entry success/error envelopes. Evidence: `watch_snapshot_uses_per_entry_error_for_unknown_paths` passed.

## Phase 3 - CONFIGURATION Program Binding

- [x] `RUNTIMEBOUND-P3-001` Fail load when `CONFIGURATION` exists and a declared `PROGRAM` is not bound or explicitly extra-instanced. Evidence: `configuration_without_program_instance_errors_by_default` passed.
- [x] `RUNTIMEBOUND-P3-002` Preserve test-builder opt-in with `CompileSession::with_extra_program_instances`. Evidence: `explicit_extra_program_instance_keeps_test_builder_opt_in` passed.

## Phase 4 - JSON Protocol

- [x] `RUNTIMEBOUND-P4-001` Default `trust-harness` to protocol v2 with per-watch-entry envelopes. Evidence: `trust_harness_cycle_dt_ms_advances_virtual_time` checks `protocol_version: 2` and v2 envelope values.
- [x] `RUNTIMEBOUND-P4-002` Preserve legacy watch shape behind explicit protocol v1. Evidence: `trust_harness_protocol_version_1_keeps_legacy_watch_shape` passed.
- [x] `RUNTIMEBOUND-P4-003` Migrate `trust-dev` agent harness watch output to envelopes. Evidence: `agent_serve_supports_harness_execute_for_pou_and_project_fixtures` and `agent_serve_supports_runtime_project_commands_and_harness_loop` passed after rebuilding `trust-dev`.

## Phase 5 - Debug/Control Boundary

- [x] `RUNTIMEBOUND-P5-001` Add stale-frame and unresolved-eval fail-closed tests for `debug.scopes`, `debug.variables`, and `debug.evaluate`. Evidence: `cargo test -p trust-runtime control::tests::debug_boundary --lib -- --nocapture` passed.
- [x] `RUNTIMEBOUND-P5-002` Replace stale-frame fallback in `debug_handlers_variables.rs` with structured control errors where applicable. Evidence: stale frame ids, stale variable handles, stale locals/instances/references, and poisoned I/O snapshot locks now return `ControlResponse::error`.
- [x] `RUNTIMEBOUND-P5-003` Decide whether debug watch `.ok()` collapse in `host/debug/control/hook.rs` needs an error-bearing watch model. Evidence: classified out of this board because `DebugControl` watch state only drives an internal changed flag and does not serialize values to an external protocol; runtime-internal watch/error modeling belongs to a separate debugger-state board if needed.

## Phase 6 - Web/HMI Boundary

- [x] `RUNTIMEBOUND-P6-001` Add tests for HMI WebSocket/control serialization failure behavior. Evidence: `hmi_ws::tests::hmi_control_error_payload_is_structured` passes.
- [x] `RUNTIMEBOUND-P6-002` Replace silent `serde_json::to_value(...).ok()?` in HMI WebSocket polling with a structured error path. Evidence: `hmi_control_result` now returns `Result`; WebSocket polling sends `{"type":"error","code":"control_request_failed",...}` instead of dropping the event.
- [x] `RUNTIMEBOUND-P6-003` Audit web/config UI fallback serialization patterns and classify transport-only fallbacks versus external observation/control errors. Evidence: `control.rs`, `/hmi/export.json`, and runtime-cloud local control proxy serialization are explicit error responses; config-UI live fallbacks remain transport-state projection paths that already emit `ok:false` or `RuntimeError`.

## Validation

- [x] `RUNTIMEBOUND-VAL-01` `cargo test -p trust-runtime --test boundary_resolver --test harness_fail_closed --test build_unbound_program --test protocol_envelope` passed.
- [x] `RUNTIMEBOUND-VAL-02` `cargo test -p trust-runtime --test trust_harness_command` passed.
- [x] `RUNTIMEBOUND-VAL-03` `cargo build -p trust-dev` passed.
- [x] `RUNTIMEBOUND-VAL-04` `cargo test -p trust-runtime --test agent_command -- harness_execute --nocapture` passed.
- [x] `RUNTIMEBOUND-VAL-05` `cargo test -p trust-runtime --test agent_command -- harness_loop --nocapture` passed.
- [x] `RUNTIMEBOUND-VAL-06` `scripts/runtime_boundary_fail_closed_ast_grep_gate.sh` passes. Evidence: wrote `target/gate-artifacts/runtime-boundary-fail-closed-7e24aebd7/runtime-boundary-summary.txt` with no findings.
- [x] `RUNTIMEBOUND-VAL-07` `cargo run -p xtask -- architecture-doctor --full-map` passes. Evidence: `FULLMAP-RUNTIMEBOUND` passed; `FULLMAP-RUNTIMEVM-MUT` remained the expected clean-tree partial from missing local mutation artifacts.
- [x] `RUNTIMEBOUND-VAL-08` Final release-readiness gates pass under the staged cadence rule. Evidence: `just fmt`, `just clippy`, and `just test-all` passed on 2026-05-03 after updating the trust-dev and PLCopen tests for the new fail-closed CONFIGURATION rule.
