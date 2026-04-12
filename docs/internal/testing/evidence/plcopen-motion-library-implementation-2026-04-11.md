# PLCopen Motion Library Implementation Evidence — 2026-04-11

## Single-Axis Conformance Through Early A4

Targeted command:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core
```

Result:

```text
Running 32 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core
PASS [1/32] TEST_PROGRAM::plcopen_motion_single_axis_axis_ref_fields_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2742ms]
PASS [2/32] TEST_PROGRAM::plcopen_motion_single_axis_buffer_mode_literals_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2751ms]
PASS [3/32] TEST_PROGRAM::plcopen_motion_single_axis_direction_literals_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2771ms]
PASS [4/32] TEST_PROGRAM::plcopen_motion_single_axis_execution_mode_literals_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2767ms]
PASS [5/32] TEST_PROGRAM::plcopen_motion_single_axis_source_literals_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2738ms]
PASS [6/32] TEST_PROGRAM::plcopen_motion_single_axis_axis_status_literals_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2738ms]
PASS [7/32] TEST_PROGRAM::plcopen_motion_single_axis_core_command_fbs_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2805ms]
PASS [8/32] TEST_PROGRAM::plcopen_motion_single_axis_motion_fbs_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2803ms]
PASS [9/32] TEST_PROGRAM::plcopen_motion_single_axis_administrative_readback_fbs_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2853ms]
PASS [10/32] TEST_PROGRAM::plcopen_motion_single_axis_parameter_fbs_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2799ms]
PASS [11/32] TEST_PROGRAM::plcopen_motion_single_axis_parameter_number_constants_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2767ms]
PASS [12/32] TEST_PROGRAM::plcopen_motion_single_axis_error_constants_resolve (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2936ms]
PASS [13/32] TEST_PROGRAM::plcopen_motion_single_axis_mc_power_status_tracks_stage_not_enable (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2825ms]
PASS [14/32] TEST_PROGRAM::plcopen_motion_single_axis_read_status_and_reset_state_paths (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3072ms]
PASS [15/32] TEST_PROGRAM::plcopen_motion_single_axis_administrative_fbs_preserve_state (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2929ms]
PASS [16/32] TEST_PROGRAM::plcopen_motion_single_axis_grouped_axis_motion_rejection_and_readonly_allowance (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2853ms]
PASS [17/32] TEST_PROGRAM::plcopen_motion_single_axis_home_backend_fault_enters_errorstop (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2824ms]
PASS [18/32] TEST_PROGRAM::plcopen_motion_single_axis_read_motion_state_source_and_flags (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2822ms]
PASS [19/32] TEST_PROGRAM::plcopen_motion_single_axis_read_axis_info_and_axis_error (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2766ms]
PASS [20/32] TEST_PROGRAM::plcopen_motion_single_axis_set_position_updates_actual_and_commanded_values (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2879ms]
PASS [21/32] TEST_PROGRAM::plcopen_motion_single_axis_set_override_validates_factors_and_vel_zero_behavior (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2826ms]
PASS [22/32] TEST_PROGRAM::plcopen_motion_single_axis_actual_value_readbacks_follow_seeded_values (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2760ms]
PASS [23/32] TEST_PROGRAM::plcopen_motion_single_axis_parameter_plane_numeric_and_bool_roundtrip (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3385ms]
PASS [24/32] TEST_PROGRAM::plcopen_motion_single_axis_parameter_plane_rejections_and_mcDelayed (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2868ms]
PASS [25/32] TEST_PROGRAM::plcopen_motion_single_axis_execute_edge_and_continuous_update_semantics (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2881ms]
PASS [26/32] TEST_PROGRAM::plcopen_motion_single_axis_enable_style_valid_error_exclusivity (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2833ms]
PASS [27/32] TEST_PROGRAM::plcopen_motion_single_axis_relative_and_additive_distinction (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2923ms]
PASS [28/32] TEST_PROGRAM::plcopen_motion_single_axis_move_velocity_direction_and_state (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2855ms]
PASS [29/32] TEST_PROGRAM::plcopen_motion_single_axis_home_halt_stop_and_continuous_end_velocity_behaviors (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3199ms]
PASS [30/32] TEST_PROGRAM::plcopen_motion_single_axis_move_absolute_final_position_semantics (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2796ms]
PASS [31/32] TEST_PROGRAM::plcopen_motion_single_axis_home_from_nonstandstill_entry_state (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [2830ms]
PASS [32/32] TEST_PROGRAM::plcopen_motion_single_axis_dynamic_max_parameter_rejections (/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [3064ms]
32 passed, 0 failed, 0 errors (98729ms)
```

Evidence covered by this run:

- `A0-01`, `A0-02`, `A0-03`, `A0-03A`, `A0-05`, `A0-05A`, `A0-08`, `A0-09`, `A0-11`, `A0-12`, and `A0-13` are covered by the public-surface and constant tests in `single_axis_core/src/tests.st`.
- `A0-05B` is covered by the corresponding entries in `docs/PLCOPEN_DECISIONS.md` referenced by the current checklist.
- `A1-01`, `A1-02`, `A1-03`, `A1-03A`, `A1-04`, `A1-05`, `A1-06`, `A1-06A`, `A1-07`, `A1-08`, `A1-09`, `A1-09A`, and `A1-10` are covered by the axis-state, grouped-axis, backend-fault, and reset-path tests in `single_axis_core/src/tests.st`.
- `A2-01`, `A2-01B`, `A2-02`, `A2-03`, `A2-03A`, `A2-03B`, `A2-04`, `A2-05`, `A2-06`, `A2-07`, `A2-08`, `A2-09`, `A2-10`, `A2-11`, `A2-12`, `A2-13`, `A2-13A`, `A2-14`, `A2-15`, `A2-16`, `A2-17`, `A2-18`, `A2-19`, and `A2-20` are covered by the `MC_Power`, readback, override, actual-value, and parameter-plane tests in `single_axis_core/src/tests.st` plus the dedicated negative projects below.
- `A3-01`, `A3-03`, `A3-03A`, `A3-03B`, `A3-07`, `A3-08`, and `A3-10` are covered by `plcopen_motion_single_axis_execute_edge_and_continuous_update_semantics` and `plcopen_motion_single_axis_enable_style_valid_error_exclusivity`.
- `A4-01`, `A4-02`, `A4-03`, `A4-04`, `A4-05`, and `A4-06` are covered by `plcopen_motion_single_axis_home_halt_stop_and_continuous_end_velocity_behaviors`, `plcopen_motion_single_axis_move_absolute_final_position_semantics`, `plcopen_motion_single_axis_home_from_nonstandstill_entry_state`, `plcopen_motion_single_axis_relative_and_additive_distinction`, `plcopen_motion_single_axis_move_velocity_direction_and_state`, and the dedicated `single_axis_negative_stop_active` negative project below.
- `A5-01` is covered by `plcopen_motion_single_axis_dynamic_max_parameter_rejections`.

Negative commands:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface
```

Result:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s
Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface`
Error: /home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/main.st: error[E102]: cannot resolve type 'MC_ERROR' (at 34..42)
/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/main.st: error[E102]: cannot resolve type 'MC_PAYLOAD_REF' (at 63..77)
/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ERROR' (at 85..93)
/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PAYLOAD_REF' (at 114..128)
```

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero
```

Result:

```text
Error: invalid typed literal
```

Pinned rejected literal in fixture source:
- `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero/src/tests.st`: `MC_BUFFER_MODE#mcTransitionVelZero`

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next
```

Result:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.36s
Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next`
Error: invalid typed literal
```

Pinned rejected literal in fixture source:
- `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next/src/tests.st`: `MC_BUFFER_MODE#mcTransitionVelNext`

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label
```

Result:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.04s
Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label`
Error: invalid typed literal
```

Pinned rejected literal in fixture source:
- `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label/src/tests.st`: `MC_AXIS_STATUS#GroupStandby`

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split
```

Result:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s
Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split`
Error: /home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split/src/main.st: error[E105]: unknown parameter 'EnablePositive' (at 104..127)
/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split/src/tests.st: error[E105]: unknown parameter 'EnableNegative' (at 159..182)
```

Evidence covered by these negative runs:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active
```

Result:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.66s
Running `target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active`
Error: /home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/main.st: error[E105]: no member `Active` on type (at 156..163)
/home/johannes/projects/trust-platform/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/tests.st: error[E105]: no member `Active` on type (at 214..220)
```


- `A0-04` The classic FB surface does not expose `MC_ERROR` and does not define `MC_PAYLOAD_REF`.
- `A0-07` The public ST surface rejects legacy `mcTransitionVelZero`, legacy `mcTransitionVelNext`, and the bare diagram label `GroupStandby`.
- `A2-01A` The initial single-axis `MC_Power` profile does not expose `EnablePositive` or `EnableNegative`.
- `A4-02` The selected single-axis `MC_Stop` public signature does not expose `Active`.

