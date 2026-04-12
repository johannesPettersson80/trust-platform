# PLCopen Motion Library Implementation Evidence — 2026-04-12

- Branch: `feature/plcopen-motion-stacked`
- Base HEAD SHA: `e6f1eaffb374d9a9c8cd7b22b5bcc57aed281794`
- Evidence timestamp: `2026-04-12T08:42:56+02:00`
- Scope: ST-only PLCopen motion library implementation through the currently shipped Part 1/4/5 profile slices plus release gates and doc sync.
- Library source of truth: `libraries/plcopen_motion/*`; fixture projects under `crates/trust-runtime/tests/fixtures/plcopen_motion/*` consume those packages via `[dependencies]`.
- Validation note: the targeted fixture commands below were rerun after the library-package extraction; the broad full-workspace gates at the end were not rerun in this relocation-focused pass.

## Phase A Single-Axis Suite

Full targeted single-axis command:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core --timeout 15
```

Observed result:

```text
Running 50 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core
...
PASS [46/50] TEST_PROGRAM::plcopen_motion_single_axis_software_limits_clamp_position_target_commands (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [9120ms]
PASS [47/50] TEST_PROGRAM::plcopen_motion_single_axis_move_absolute_final_position_semantics (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [8311ms]
PASS [48/50] TEST_PROGRAM::plcopen_motion_single_axis_home_from_nonstandstill_entry_state (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [8323ms]
PASS [49/50] TEST_PROGRAM::plcopen_motion_single_axis_end_to_end_conformance_scenario (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [9581ms]
PASS [50/50] TEST_PROGRAM::plcopen_motion_single_axis_dynamic_max_parameter_rejections (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [8326ms]
50 passed, 0 failed, 0 errors (423027ms)
```

Representative focused commands:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core --timeout 15 --filter software_limits_clamp_position_target_commands
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core --timeout 15 --filter end_to_end_conformance_scenario
```

Observed results:

```text
Running 1 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core
PASS [1/1] TEST_PROGRAM::plcopen_motion_single_axis_software_limits_clamp_position_target_commands (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [9313ms]
1 passed, 0 failed, 0 errors (9313ms)

Running 1 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core
PASS [1/1] TEST_PROGRAM::plcopen_motion_single_axis_end_to_end_conformance_scenario (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st) [9829ms]
1 passed, 0 failed, 0 errors (9829ms)
```

Negative/deferred guard commands:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface
```

Observed results:

```text
Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/main.st: error[E102]: cannot resolve type 'MC_ERROR' (at 34..42)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/main.st: error[E102]: cannot resolve type 'MC_PAYLOAD_REF' (at 63..77)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ERROR' (at 85..93)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PAYLOAD_REF' (at 114..128)

Error: invalid typed literal

Error: invalid typed literal

Error: invalid typed literal

Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split/src/main.st: error[E105]: unknown parameter 'EnablePositive' (at 104..127)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split/src/tests.st: error[E105]: unknown parameter 'EnableNegative' (at 159..182)

Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/main.st: error[E105]: no member 'Active' on type (at 156..163)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/tests.st: error[E105]: no member 'Active' on type (at 214..220)

Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_MoveSuperimposed' (at 98..117)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_HaltSuperimposed' (at 142..161)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_TorqueControl' (at 183..199)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PositionProfile' (at 223..241)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_VelocityProfile' (at 265..283)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_AccelerationProfile' (at 311..333)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadDigitalInput' (at 358..377)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadDigitalOutput' (at 403..423)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_WriteDigitalOutput' (at 450..471)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_DigitalCamSwitch' (at 496..515)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_TouchProbe' (at 534..547)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_AbortTrigger' (at 568..583)
```

Coverage notes:

- `A0` is backed by the public-surface resolution tests, the `MC_Constants()` resolution tests, and the classic negative surface projects above.
- `A1` through `A5` are backed by the full 50-test single-axis run, especially the queue/abort/ownership tests, parameter-roundtrip tests, software-limit clamp tests, and the end-to-end conformance scenario.
- `A6` is backed by `single_axis_negative_deferred_public_surface` plus the compliance-matrix `Deferred` rows for the absent-path Phase A FBs.

## Phase B Synchronization Suite

Full targeted synchronization command:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization --timeout 15
```

Observed result:

```text
Running 14 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization
...
PASS [11/14] TEST_PROGRAM::plcopen_motion_synchronization_gearout_requires_synchronized_motion (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1844ms]
PASS [12/14] TEST_PROGRAM::plcopen_motion_synchronization_buffered_command_only_becomes_active_on_promotion (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [2127ms]
PASS [13/14] TEST_PROGRAM::plcopen_motion_synchronization_aborting_command_flushes_active_and_queued_sync_commands (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [2106ms]
PASS [14/14] TEST_PROGRAM::plcopen_motion_synchronization_cam_and_gear_end_to_end_scenario (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [2254ms]
14 passed, 0 failed, 0 errors (29009ms)
```

Focused commands:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization --timeout 15 --filter cam_
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization --timeout 15 --filter gear
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization_negative_deferred_public_surface
```

Observed results:

```text
Running 4 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization
PASS [1/4] TEST_PROGRAM::plcopen_motion_synchronization_cam_activation_timing_is_deterministic (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1971ms]
PASS [2/4] TEST_PROGRAM::plcopen_motion_synchronization_cam_table_select_prepares_camtableid_and_rejects_delayed (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1827ms]
PASS [3/4] TEST_PROGRAM::plcopen_motion_synchronization_camin_missing_selected_cam_errors (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1727ms]
PASS [4/4] TEST_PROGRAM::plcopen_motion_synchronization_cam_and_gear_end_to_end_scenario (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [2073ms]
4 passed, 0 failed, 0 errors (7600ms)

Running 4 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization
PASS [1/4] TEST_PROGRAM::plcopen_motion_synchronization_gearin_tracks_master_ratio_and_ingear (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1888ms]
PASS [2/4] TEST_PROGRAM::plcopen_motion_synchronization_gearinpos_startsync_then_insync (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1899ms]
PASS [3/4] TEST_PROGRAM::plcopen_motion_synchronization_gearout_requires_synchronized_motion (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [1717ms]
PASS [4/4] TEST_PROGRAM::plcopen_motion_synchronization_cam_and_gear_end_to_end_scenario (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st) [2069ms]
4 passed, 0 failed, 0 errors (7575ms)

Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PhasingAbsolute' (at 104..122)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PhasingRelative' (at 146..164)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_CombineAxes' (at 184..198)
```

Coverage notes:

- `B0` through `B3` are backed by the deterministic master/slave fixture helpers, the full 14-test synchronization run, and the focused cam/gear subsets.
- `B4` is backed by `synchronization_negative_deferred_public_surface` plus the compliance-matrix `Deferred` rows for `MC_PhasingAbsolute`, `MC_PhasingRelative`, and `MC_CombineAxes`.

## Phase C Coordinated-Motion Core Suite

Full targeted coordinated-motion command:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion --timeout 15
```

Observed result:

```text
Running 19 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion
...
PASS [16/19] TEST_PROGRAM::plcopen_motion_coordinated_motion_linear_and_direct_motion_fbs (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st) [6297ms]
PASS [17/19] TEST_PROGRAM::plcopen_motion_coordinated_motion_group_home_stop_halt_wait_and_override_fbs (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st) [6415ms]
PASS [18/19] TEST_PROGRAM::plcopen_motion_coordinated_motion_dynamics_and_command_info_fbs (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st) [6064ms]
PASS [19/19] TEST_PROGRAM::plcopen_motion_coordinated_motion_unsupported_transition_and_legacy_blending_are_rejected (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st) [5745ms]
19 passed, 0 failed, 0 errors (115762ms)
```

Deferred guard command:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface
```

Observed result:

```text
Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_MoveCircularAbsolute' (at 112..135)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_MoveCircularRelative' (at 164..187)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_CIRC_MODE' (at 204..216)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_CIRC_PATHCHOICE' (at 239..257)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_PathSelect' (at 276..289)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_MovePath' (at 306..317)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_GroupInterrupt' (at 340..357)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_GroupContinue' (at 379..395)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadDHParameters' (at 420..439)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadJointInfo' (at 461..477)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_GroupJog' (at 494..505)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_GroupJogVector' (at 528..545)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_GroupWriteJoggingDynamics' (at 579..607)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_GroupReadJoggingDynamics' (at 640..667)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_WriteToolData' (at 689..705)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadToolData' (at 726..741)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_SelectTool' (at 760..773)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadTool' (at 790..801)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_TOOL_SOURCE' (at 820..834)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_WritePayloadData' (at 859..878)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadPayloadData' (at 902..920)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_SelectPayload' (at 942..958)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadPayload' (at 978..992)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_ReadRigidBodyDynamic' (at 1021..1044)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_WriteRigidBodyDynamic' (at 1074..1098)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_SetDynCoordTransform' (at 1127..1150)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_TrackConveyorBelt' (at 1176..1196)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_SyncAxisToGroup' (at 1220..1238)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_SyncGroupToAxis' (at 1262..1280)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_TrackRotaryTable' (at 1305..1324)
```

Coverage notes:

- `C0` through `C3` are backed by the full 19-test coordinated-motion run, which covers public type resolution, membership/admin/readback, transforms, linear/direct motion, group home/stop/halt/wait/override, dynamics/reference planes, command-info retention, and the supported transition subset.
- `C4` and `C5` are backed by `coordinated_motion_negative_deferred_public_surface`, including the deferred type guards for `MC_CIRC_MODE`, `MC_CIRC_PATHCHOICE`, and `MC_TOOL_SOURCE`, plus the compliance-matrix `Deferred` rows for the absent-path optional C.1 and later coordinated-motion features.

## Phase D Homing Suite

Full targeted homing command:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/homing --timeout 15
```

Observed result:

```text
Running 10 ST test(s) in crates/trust-runtime/tests/fixtures/plcopen_motion/homing
...
PASS [7/10] TEST_PROGRAM::plcopen_motion_homing_reference_pulse_behavior (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st) [5535ms]
PASS [8/10] TEST_PROGRAM::plcopen_motion_homing_distance_coded_and_limit_errors (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st) [5835ms]
PASS [9/10] TEST_PROGRAM::plcopen_motion_homing_direct_absolute_finish_and_generic_home_behaviors (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st) [6506ms]
PASS [10/10] TEST_PROGRAM::plcopen_motion_homing_multi_step_sequence_finishes_in_work_area (/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st) [5825ms]
10 passed, 0 failed, 0 errors (59179ms)
```

Deferred guard command:

```bash
target/debug/trust-runtime test --project crates/trust-runtime/tests/fixtures/plcopen_motion/homing_negative_deferred_public_surface
```

Observed result:

```text
Error: /home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_StepReferenceFlyingSwitch' (at 105..133)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_StepReferenceFlyingRefPulse' (at 169..199)
/home/johannes/projects/trust-platform-motion-stacked/crates/trust-runtime/tests/fixtures/plcopen_motion/homing_negative_deferred_public_surface/src/tests.st: error[E102]: cannot resolve type 'MC_AbortPassiveHoming' (at 226..247)
```

Coverage notes:

- `D0` through `D2` are backed by the 10-test homing suite, including seeded signal setup, the selected step FBs, direct/absolute/finish homing, generic `MC_Home` regression coverage, and a multi-step homing sequence.
- `D3` is backed by `homing_negative_deferred_public_surface` plus the compliance-matrix `Deferred` rows for the absent-path passive/flying homing names.

## Runtime Vertical Checks

Commands:

```bash
cargo test -p trust-runtime --test api_smoke
cargo test -p trust-runtime --test complete_program
cargo test -p trust-runtime --test debug_control
cargo test -p trust-runtime --test runtime_reliability
```

Observed results:

```text
api_smoke: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
complete_program: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
debug_control: test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
runtime_reliability: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Diagram And Doc Sync

Commands:

```bash
./scripts/render_diagrams.sh
python scripts/check_diagram_drift.py --update
python scripts/check_diagram_drift.py
```

Observed results:

```text
render_diagrams: ok
check_diagram_drift --update: ok
check_diagram_drift: ok
```

Notes:

- The motion architecture diagram lives at `docs/diagrams/architecture/plcopen-motion-library.puml` and the rendered output at `docs/diagrams/generated/plcopen-motion-library.svg`.
- `docs/internal/testing/checklists/architecture-improvements.md` was updated and marked `Status: Done` after the diagram render + manifest refresh.

## Full Workspace Gates

Historical milestone commands (not rerun after the later library-package extraction):

```bash
just fmt
just clippy
just test
just test-all
cargo test -p trust-runtime plcopen_motion
```

Observed results:

```text
just fmt: ok
just clippy: ok (Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 11s)
just test: ok (Summary [   2.451s] 295 tests run: 295 passed, 0 skipped)
just test-all: not rerun in the later package-relocation pass
cargo test -p trust-runtime plcopen_motion: not rerun in the later package-relocation pass
```
