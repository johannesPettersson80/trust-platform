# PLCopen Motion Library Guide

This guide describes the PLCopen Motion library profile that truST currently
ships.

## Scope

truST currently ships these PLCopen Motion subsets:

- Part 1 single-axis classic FBs
- Part 1 synchronization core (`cam` / `gear`)
- Part 4 coordinated-motion core subset
- Part 5 homing toolkit core subset

Authoritative profile sources:

- `docs/internal/references/PLCopenMotion/plcopen_motion_library_spec_for_truST_v0_1.md`
- `docs/internal/references/PLCopenMotion/plcopen_motion_compliance_matrix.yaml`
- `docs/PLCOPEN_DECISIONS.md`
- `docs/PLCOPEN_DEVIATIONS.md`

## Library Packages

The reusable PLCopen motion library source of truth lives under:

- `libraries/plcopen_motion/single_axis_core`
- `libraries/plcopen_motion/synchronization`
- `libraries/plcopen_motion/coordinated_motion`
- `libraries/plcopen_motion/homing`

Each package is a normal truST Structured Text package with its own
`trust-lsp.toml` and `src/`.

All current fixture projects under
`crates/trust-runtime/tests/fixtures/plcopen_motion/*` consume those packages
through `[dependencies]`; they are conformance consumers only, not the library
location users should copy.

## Supported Public Surface

### Part 1 Single-Axis

Public types and enums:

- `AXIS_REF`
- `MC_BUFFER_MODE`
- `MC_DIRECTION`
- `MC_EXECUTION_MODE`
- `MC_SOURCE`
- `MC_AXIS_STATUS`
- `MC_Constants`

Implemented classic FBs:

- `MC_Power`
- `MC_Home`
- `MC_Stop`
- `MC_Halt`
- `MC_MoveAbsolute`
- `MC_MoveRelative`
- `MC_MoveAdditive`
- `MC_MoveVelocity`
- `MC_MoveContinuousAbsolute`
- `MC_MoveContinuousRelative`
- `MC_SetPosition`
- `MC_SetOverride`
- `MC_ReadActualPosition`
- `MC_ReadActualVelocity`
- `MC_ReadActualTorque`
- `MC_ReadStatus`
- `MC_ReadMotionState`
- `MC_ReadAxisInfo`
- `MC_ReadAxisError`
- `MC_Reset`
- `MC_ReadParameter`
- `MC_ReadBoolParameter`
- `MC_WriteParameter`
- `MC_WriteBoolParameter`

`MC_Constants()` publishes the standardized parameter-number names and public
`mcERR_*` constants. Call the FB once before reading its members.

### Part 1 Synchronization

Public types and enums:

- `MC_START_MODE`
- `MC_SYNC_MODE`
- `MC_CAM_ID`
- `MC_CAM_REF`

Implemented FBs:

- `MC_CamTableSelect`
- `MC_CamIn`
- `MC_CamOut`
- `MC_GearIn`
- `MC_GearOut`
- `MC_GearInPos`

### Part 4 Coordinated Motion Core

Public types and enums:

- `AXES_GROUP_REF`
- `AXIS_ID`
- `AXES_GROUP_ID`
- `IDENT_IN_GROUP_REF`
- `MC_COMMAND_ID`
- `MC_GROUP_PARAMETER`
- `MC_TRANSITION_PARAMETER`
- `MC_KIN_REF`
- `MC_COORD_SYSTEM`
- `MC_DYNAMICS_MODE`
- `MC_TRANSITION_MODE`
- `MC_TRANSITION_VELOCITY`
- `MC_TRANSITION_REFERENCE`
- `MC_ORIENTATION_MODE`
- `MC_COMMAND_STATE`
- `MC_GROUP_STATUS`
- `MC_CART_REF`
- `MC_COORD_REF`
- `MC_CONFIG_DATA`
- `MC_TURN_INFO`
- `MC_CART_POS_REF`
- `MC_AXES_POS_REF`
- `MC_POS_REF`
- `MC_DISTANCE_REF`
- `MC_SWLIMIT`
- `MC_GROUP_SWLIMITS`

Implemented FBs:

- `MC_AddAxisToGroup`
- `MC_RemoveAxisFromGroup`
- `MC_UngroupAllAxes`
- `MC_GroupReadConfiguration`
- `MC_ReadAxisGroupInfo`
- `MC_GroupEnable`
- `MC_GroupDisable`
- `MC_GroupPower`
- `MC_GroupReadStatus`
- `MC_GroupReadError`
- `MC_GroupReset`
- `MC_GroupReadPosition`
- `MC_GroupReadVelocity`
- `MC_GroupReadAcceleration`
- `MC_GroupReadMotionState`
- `MC_GroupReadParameter`
- `MC_GroupWriteParameter`
- `MC_GroupReadSWLimits`
- `MC_GroupWriteSWLimits`
- `MC_SetKinTransform`
- `MC_SetCartesianTransform`
- `MC_SetCoordinateTransform`
- `MC_ReadKinTransform`
- `MC_ReadCartesianTransform`
- `MC_ReadCoordinateTransform`
- `MC_GroupSetPosition`
- `MC_MoveLinearAbsolute`
- `MC_MoveLinearRelative`
- `MC_MoveDirectAbsolute`
- `MC_MoveDirectRelative`
- `MC_GroupHome`
- `MC_GroupStop`
- `MC_GroupHalt`
- `MC_GroupWaitTime`
- `MC_GroupSetOverride`
- `MC_TransformPosition`
- `MC_GroupReadCommandInfo`
- `MC_GroupWriteReferenceDynamics`
- `MC_GroupReadReferenceDynamics`
- `MC_GroupWriteDefaultDynamics`
- `MC_GroupReadDefaultDynamics`

### Part 5 Homing Core

Public types:

- `MC_HOME_DIRECTION`
- `MC_SWITCH_MODE`
- `MC_REF_SIGNAL_REF`

Implemented FBs:

- `MC_StepAbsoluteSwitch`
- `MC_StepLimitSwitch`
- `MC_StepBlock`
- `MC_StepReferencePulse`
- `MC_StepDistanceCoded`
- `MC_HomeDirect`
- `MC_HomeAbsolute`
- `MC_FinishHoming`

## Deferred Or Not Shipped

These names are intentionally deferred in the current shipped profile:

- Deferred single-axis FBs such as `MC_MoveSuperimposed`, `MC_TorqueControl`, and the digital I/O / touch-probe subset
- Deferred synchronization FBs `MC_PhasingAbsolute`, `MC_PhasingRelative`, and `MC_CombineAxes`
- Optional coordinated-motion tracking/synchronization names such as `MC_SetDynCoordTransform`, `MC_TrackConveyorBelt`, `MC_SyncAxisToGroup`, and `MC_SyncGroupToAxis`
- Deferred coordinated-motion path/circular/jog/tool/payload and kinematic-introspection names
- Deferred homing passive/flying names `MC_StepReferenceFlyingSwitch`, `MC_StepReferenceFlyingRefPulse`, and `MC_AbortPassiveHoming`
- OO facade
- Part 6 fluid power

Deferred names follow the compliance-matrix path recorded in
`plcopen_motion_compliance_matrix.yaml`; they are not implied to be shipped.

## Reference Example And Benchmark

The user-facing motion example lives under:

- `examples/plcopen_motion_single_axis_demo`

That example consumes `libraries/plcopen_motion/single_axis_core` through
`[dependencies]`, exposes watched globals for runtime inspection, and is the
reference project for end-to-end runtime benchmarking.

Focused benchmark entrypoints:

- `trust-runtime bench project --project examples/plcopen_motion_single_axis_demo ...`
- `./scripts/runtime_motion_example_bench_gate.sh`
- `./scripts/runtime_motion_benchmark_breakdown.sh`

Use the demo + gate when you need one shipped end-to-end answer. Use the
breakdown pack under `examples/plcopen_motion_single_axis_benchmarks` when you
need to separate runtime floor, `MC_Constants()`, status/readback overhead,
inactive command upkeep, and single-command execution cost on the same
hardware. `trust-runtime bench project` now emits a VM profile block for
VM-backed workloads so the benchmark artifacts also show register-executor
fallbacks, hot blocks, and lowering-cache behavior.

## Validation

The reusable library packages above are locked by ST conformance suites under:

- `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core`
- `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization`
- `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion`
- `crates/trust-runtime/tests/fixtures/plcopen_motion/homing`

Those fixture projects are test consumers only. Deferred-name guard coverage
lives in the matching `*_negative_*` projects.
