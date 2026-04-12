# PLCopen Motion Library Post-Parity Reconciliation Evidence — 2026-04-12

> Historical note: this reconciliation pass predates the later extraction of the reusable motion-library sources into `libraries/plcopen_motion/*`. The current library source of truth is the package layout under `libraries/`; the fixture projects remain conformance consumers.

## Stacked Branch Base

- Worktree: `/home/johannes/projects/trust-platform-motion-stacked`
- Branch: `feature/plcopen-motion-stacked`
- HEAD before the reconciliation-doc commit: `c198e9c4411f387c3a9879afead4f5cacdddede9`

## Rebase Result

- `git rebase feature/codesys-twincat-parity`
- Result: clean rebase, no conflicts

## Stack-Surfaced Motion Fix

Parity's stricter FUNCTION_BLOCK-body name resolution surfaced that the carried motion fixture still depended on bare access to the internal shared-state carrier when that carrier lived inside `PROGRAM Main VAR_GLOBAL`. The stacked baseline failed with `undefined identifier 'g_trust_motion_*'` in the single-axis motion FB bodies.

Applied fix on the stacked branch:
- [plcopen_motion_single_axis_globals.st](/home/johannes/projects/trust-platform-motion-stacked/libraries/plcopen_motion/single_axis_core/src/plcopen_motion_single_axis_globals.st): now hosts the internal shared-state carrier as a file-scope `VAR_GLOBAL` block.
- [plcopen_motion_single_axis_internal_kernel.st](/home/johannes/projects/trust-platform-motion-stacked/libraries/plcopen_motion/single_axis_core/src/plcopen_motion_single_axis_internal_kernel.st): removed the temporary `VAR_EXTERNAL` experiment used during diagnosis.
- [plcopen_motion_single_axis_public_surface.st](/home/johannes/projects/trust-platform-motion-stacked/libraries/plcopen_motion/single_axis_core/src/plcopen_motion_single_axis_public_surface.st): removed the temporary `VAR_EXTERNAL` experiment used during diagnosis.

No PLCopen public-surface names changed. The fix was internal to the ST library kernel.

## Regression Sweep

### 1. Motion Positive Fixture

Command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test   --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core
```

Output tail:

```text
PASS [14/32] TEST_PROGRAM::plcopen_motion_single_axis_read_status_and_reset_state_paths (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3319ms]
PASS [15/32] TEST_PROGRAM::plcopen_motion_single_axis_administrative_fbs_preserve_state (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3192ms]
PASS [16/32] TEST_PROGRAM::plcopen_motion_single_axis_grouped_axis_motion_rejection_and_readonly_allowance (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3135ms]
PASS [17/32] TEST_PROGRAM::plcopen_motion_single_axis_home_backend_fault_enters_errorstop (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3070ms]
PASS [18/32] TEST_PROGRAM::plcopen_motion_single_axis_read_motion_state_source_and_flags (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3079ms]
PASS [19/32] TEST_PROGRAM::plcopen_motion_single_axis_read_axis_info_and_axis_error (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3028ms]
PASS [20/32] TEST_PROGRAM::plcopen_motion_single_axis_set_position_updates_actual_and_commanded_values (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3133ms]
PASS [21/32] TEST_PROGRAM::plcopen_motion_single_axis_set_override_validates_factors_and_vel_zero_behavior (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3086ms]
PASS [22/32] TEST_PROGRAM::plcopen_motion_single_axis_actual_value_readbacks_follow_seeded_values (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3006ms]
PASS [23/32] TEST_PROGRAM::plcopen_motion_single_axis_parameter_plane_numeric_and_bool_roundtrip (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3634ms]
PASS [24/32] TEST_PROGRAM::plcopen_motion_single_axis_parameter_plane_rejections_and_mcDelayed (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3128ms]
PASS [25/32] TEST_PROGRAM::plcopen_motion_single_axis_execute_edge_and_continuous_update_semantics (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3119ms]
PASS [26/32] TEST_PROGRAM::plcopen_motion_single_axis_enable_style_valid_error_exclusivity (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3039ms]
PASS [27/32] TEST_PROGRAM::plcopen_motion_single_axis_relative_and_additive_distinction (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3144ms]
PASS [28/32] TEST_PROGRAM::plcopen_motion_single_axis_move_velocity_direction_and_state (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3083ms]
PASS [29/32] TEST_PROGRAM::plcopen_motion_single_axis_home_halt_stop_and_continuous_end_velocity_behaviors (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3451ms]
PASS [30/32] TEST_PROGRAM::plcopen_motion_single_axis_move_absolute_final_position_semantics (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3049ms]
PASS [31/32] TEST_PROGRAM::plcopen_motion_single_axis_home_from_nonstandstill_entry_state (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3094ms]
PASS [32/32] TEST_PROGRAM::plcopen_motion_single_axis_dynamic_max_parameter_rejections (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3093ms]
32 passed, 0 failed, 0 errors (99162ms)
```

### 2. Motion Negative Fixtures

Command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test   --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface
```

Output tail:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface`
Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/main.st: error[E102]: cannot resolve type 'MC_ERROR' (at 34..42)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/main.st: error[E102]: cannot resolve type 'MC_PAYLOAD_REF' (at 63..77)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ERROR' (at 85..93)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PAYLOAD_REF' (at 114..128)
```

Command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test   --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero
```

Output tail:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero`
Error: invalid typed literal
```

Pinned rejected literal:
- [tests.st](/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero/src/tests.st)
  `MC_BUFFER_MODE#mcTransitionVelZero`

Command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test   --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next
```

Output tail:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.44s
     Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next`
Error: invalid typed literal
```

Pinned rejected literal:
- [tests.st](/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next/src/tests.st)
  `MC_BUFFER_MODE#mcTransitionVelNext`

Command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test   --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label
```

Output tail:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label`
Error: invalid typed literal
```

Pinned rejected literal:
- [tests.st](/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label/src/tests.st)
  `MC_AXIS_STATUS#GroupStandby`

Command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test   --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active
```

Output tail:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.43s
     Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active`
Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/main.st: error[E105]: no member 'Active' on type (at 156..163)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/tests.st: error[E105]: no member 'Active' on type (at 214..220)
```

### 3. Runtime Verticals

Command:

```bash
cargo test -p trust-runtime --test api_smoke
```

Output tail:

```text
   Compiling trust-runtime v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 17.31s
     Running tests/api_smoke.rs (target/debug/deps/api_smoke-ecf61ae761507174)

running 3 tests
test loads_runtime ... ok
test runtime_execution_backend_defaults_and_lazy_vm_materialization ... ok
test runtime_metrics_snapshot_tracks_vm_backend_selection ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Command:

```bash
cargo test -p trust-runtime --test complete_program
```

Output tail:

```text
   Compiling trust-runtime v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.86s
     Running tests/complete_program.rs (target/debug/deps/complete_program-34516c1dfda2a0db)

running 1 test
test complete_program_compiles_without_errors ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Command:

```bash
cargo test -p trust-runtime --test debug_control
```

Output tail:

```text
test breakpoint_emits_stop_event ... ok
test debug_hook_fires_once_per_statement ... ok
test frame_location_tracks_current_frame ... ok
test conditional_breakpoint_skips_when_false ... ok
test logpoint_emits_output_without_pausing ... ok
test breakpoint_pauses_execution ... ok
test resolve_breakpoint_next_statement ... ok
test resolve_breakpoint_prefers_inner_statement ... ok
test conditional_breakpoint_pauses_when_true ... ok
test runtime_resolves_breakpoint_position_to_statement_start ... ok
test runtime_resolves_breakpoint_using_index ... ok
test pause_preserves_task_order ... ok
test statement_locations_use_first_token_in_if_block ... ok
test hit_count_breakpoint_pauses_on_threshold ... ok
test watch_changes_reported_between_stops ... ok
test step_once_pauses_again ... ok
test step_out_pauses_after_return ... ok
test step_over_pauses_at_same_depth ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
```

Command:

```bash
cargo test -p trust-runtime --test runtime_reliability
```

Output tail:

```text
   Compiling trust-runtime v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.73s
     Running tests/runtime_reliability.rs (target/debug/deps/runtime_reliability-a6f3db871dc618da)

running 4 tests
test e2e_retain_roundtrip_restart ... ok
test watchdog_faults_resource_on_overrun ... ok
test retain_power_loss_does_not_persist_unsaved ... ok
test e2e_startup_io_restart ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### 4. Runtime-Side Motion Filter

Command:

```bash
cargo test -p trust-runtime plcopen_motion
```

Output tail:

```text

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

     Running tests/web_ide_integration.rs (target/debug/deps/web_ide_integration-4bfe6f94447b9c28)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out; finished in 0.00s

     Running tests/web_io_config_integration.rs (target/debug/deps/web_io_config_integration-241417c9d349a347)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 48 filtered out; finished in 0.00s

     Running tests/web_tls_integration.rs (target/debug/deps/web_tls_integration-07deece48e052c5a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

Note:
- The cargo filter compiled broadly across `trust-runtime` test targets before converging on filtered test binaries. No motion-specific failure surfaced on the stacked state.

## Full Workspace Gates

Command:

```bash
just fmt
```

Result:

```text
cargo fmt
```

Command:

```bash
just clippy
```

Result:

```text
cargo clippy --all-targets --all-features
    Checking trust-syntax v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-syntax)
    Checking zenoh v1.7.2
    Checking trust-hir v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-hir)
    Checking trust-ide v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-ide)
    Checking trust-wasm-analysis v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-wasm-analysis)
    Checking trust-runtime v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime)
    Checking trust-debug v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-debug)
    Checking trust-lsp v0.11.0 (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-lsp)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 09s
```

Command:

```bash
just test
```

Result:

```text
        PASS [   0.011s] (278/295) trust-runtime web::runtime_cloud_policy::tests::wan_profile_denies_write_without_matching_rule
        PASS [   0.011s] (279/295) trust-runtime web::runtime_cloud_policy::tests::wan_allowlist_parser_fuzz_smoke_budget
        PASS [   0.015s] (280/295) trust-runtime web::runtime_cloud_routes::control_proxy::tests::proxy_action_type_uses_status_read_for_viewer
        PASS [   0.013s] (281/295) trust-runtime web::runtime_cloud_state::links::tests::apply_preferences_adds_t0_overlay_edge_without_removing_mesh
        PASS [   0.014s] (282/295) trust-runtime web::runtime_cloud_routes::control_proxy::tests::proxy_control_payload_keeps_request_shape
        PASS [   0.011s] (283/295) trust-runtime web::runtime_cloud_state::links::tests::compute_host_groups_deterministic_ordering
        PASS [   0.012s] (284/295) trust-runtime web::runtime_cloud_state::links::tests::compute_host_groups_empty_discovery
        PASS [   0.018s] (285/295) trust-runtime web::runtime_cloud_state::links::tests::apply_preferences_overrides_channel_for_extended_transports
        PASS [   0.008s] (286/295) trust-runtime web::runtime_cloud_state::links::tests::compute_host_groups_two_same_host
        PASS [   0.009s] (287/295) trust-runtime web::runtime_cloud_state::links::tests::compute_host_groups_three_mixed
        PASS [   0.009s] (288/295) trust-runtime web::runtime_cloud_state::links::tests::link_transport_preference_roundtrips_from_state
        PASS [   0.009s] (289/295) trust-runtime web::runtime_cloud_state::links::tests::same_host_check_prefers_host_group_when_present
        PASS [   0.009s] (290/295) trust-runtime web::runtime_cloud_state::links::tests::same_host_check_uses_discovery_address_overlap
        PASS [   0.008s] (291/295) trust-runtime web::runtime_cloud_state::links::tests::seed_link_transport_preferences_applies_and_removes_toml_actor_entries
        PASS [   0.007s] (292/295) trust-runtime web::runtime_cloud_state::links::tests::topology_feature_flags_plant_only_host_containers
        PASS [   0.007s] (293/295) trust-runtime web::runtime_cloud_state::links::tests::topology_feature_flags_dev_all_enabled
        PASS [   0.007s] (294/295) trust-runtime web::runtime_cloud_state::rollouts::tests::runtime_cloud_rollout_applying_timeout_transitions_to_failed
        PASS [   1.572s] (295/295) trust-runtime mesh::tests::mesh_tls_publish_applies_updates
────────────
     Summary [   2.491s] 295 tests run: 295 passed, 0 skipped
```

Command:

```bash
just test-all
```

Result:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests trust_runtime

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests trust_syntax

running 1 test
test crates/trust-syntax/src/lib.rs - (line 20) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s

   Doc-tests trust_wasm_analysis

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Checklist And Docs Reconciliation

Updated after the green stacked sweep:
- [plcopen-motion-library-implementation-checklist.md](/home/johannes/projects/trust-platform-motion-stacked/docs/internal/testing/checklists/plcopen-motion-library-implementation-checklist.md): removed the stale blocker framing and replaced it with the landed parity-runtime-base note.
- [plcopen_motion_compliance_matrix.yaml](/home/johannes/projects/trust-platform-motion-stacked/docs/internal/references/PLCopenMotion/plcopen_motion_compliance_matrix.yaml): kept the row set aligned and updated the metadata note from current A2 wording to current early A4 wording.

A0-11 audit result:
- Verified rows already exist for `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveVelocity`, `MC_MoveContinuousAbsolute`, `MC_MoveContinuousRelative`, `MC_SetPosition`, `MC_SetOverride`, `MC_ReadActualPosition`, `MC_ReadActualVelocity`, `MC_ReadActualTorque`, `MC_ReadAxisError`, `MC_ReadParameter`, `MC_ReadBoolParameter`, `MC_WriteParameter`, and `MC_WriteBoolParameter`.
- No additional row additions were required in this reconciliation pass.

Evidence grep result:
- `rg -n "blocker|VAR_STAT|VAR_GLOBAL" docs/internal/testing/evidence`
- Exit code `1`
- No stale blocker/runtime-prerequisite phrasing remained in the historical evidence set.

## Candidate Shape Migrations

Identify only; do not apply in this pass.

### VAR_STAT Candidates In Motion FBs

The current single-axis motion FBs keep per-instance persistent state in plain `VAR` fields. With parity runtime support landed, these are candidates to migrate to `VAR_STAT` later.

- `MC_Home` in [plcopen_motion_single_axis_public_surface.st](/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/plcopen_motion_single_axis_public_surface.st): `LastExecute` line 112, `RequestedPosition` line 113. Reason: rising-edge capture and retained command target.
- `MC_Stop`: `LastExecute` line 187. Reason: execute-edge retention.
- `MC_Halt`: `LastExecute` line 257. Reason: execute-edge retention.
- `MC_MoveAbsolute`: `LastExecute` line 325, `TargetPosition` line 326. Reason: retained execute-edge state and captured target.
- `MC_MoveRelative`: `LastExecute` line 408, `TargetPosition` line 409. Reason: retained execute-edge state and captured target.
- `MC_MoveAdditive`: `LastExecute` line 489, `TargetPosition` line 490. Reason: retained execute-edge state and captured target.
- `MC_MoveVelocity`: `LastExecute` line 570, `RequestedVelocity` line 571. Reason: retained execute-edge state and captured signed velocity.
- `MC_MoveContinuousAbsolute`: `LastExecute` line 655, `RequestedEndVelocity` line 656. Reason: retained execute-edge state and captured end-velocity request.
- `MC_MoveContinuousRelative`: `LastExecute` line 732, `TargetPosition` line 733, `RequestedEndVelocity` line 734. Reason: retained execute-edge state and captured continuous relative target/end velocity.
- `MC_SetPosition`: `LastExecute` line 804. Reason: execute-edge retention.
- `MC_Reset`: `LastExecute` line 1201. Reason: execute-edge retention.
- `MC_WriteParameter`: `LastExecute` line 1358. Reason: execute-edge retention.
- `MC_WriteBoolParameter`: `LastExecute` line 1446. Reason: execute-edge retention.

### Shared-State Carrier Shape

Current kernel shape:
- [plcopen_motion_single_axis_globals.st](/home/johannes/projects/trust-platform-motion-stacked/libraries/plcopen_motion/single_axis_core/src/plcopen_motion_single_axis_globals.st) uses a file-scope `VAR_GLOBAL` block as the internal shared axis-state carrier.

Possible later alternatives now available on the parity base:
- Move the carrier into a dedicated `gvl_motion.st` file with a bare top-level `VAR_GLOBAL` block.
- Move the carrier into a `NAMESPACE ... VAR_GLOBAL ... END_VAR END_NAMESPACE` wrapper if qualified vendor-parity names are preferred for internal organization.

### Redundant VAR_EXTERNAL Blocks

- None remain in the motion ST code after the stacked-state reconciliation fix.
