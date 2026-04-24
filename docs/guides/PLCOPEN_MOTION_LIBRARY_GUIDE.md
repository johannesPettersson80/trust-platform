# PLCopen Motion Library Guide

This guide is the user-facing reference manual for the PLCopen Motion packages shipped with truST. It follows the same documentation intent users expect from library manuals such as OSCAT: each public type and each public function block is documented as a named API surface, not just listed in a coverage table.

If you want a worked project first, start with the reference example in
`examples/plcopen_motion_single_axis_demo`. That project explains how the
single-axis demo is wired and why the scan loop is written the way it is.

## Package Layout

- `libraries/plcopen_motion/single_axis_core`: single-axis motion, readbacks, and parameter access
- `libraries/plcopen_motion/synchronization`: cam and gear synchronization blocks
- `libraries/plcopen_motion/coordinated_motion`: group handles, transforms, and coordinated-motion blocks
- `libraries/plcopen_motion/homing`: advanced homing-step blocks

## Dependency Setup

Add the package you need to your project `trust-lsp.toml`. A minimal single-axis project typically starts with:

```toml
[project]
vendor_profile = "codesys"
include_paths = ["src"]
stdlib = "iec"

[dependencies]
PLCopenMotionSingleAxis = { path = "../../libraries/plcopen_motion/single_axis_core", version = "0.1.0" }
```

## Library Usage Rules

1. Keep one shared `AXIS_REF` per axis and one shared `AXES_GROUP_REF` per group.
2. Instantiate each PLCopen function block once and call it every scan.
3. Drive commands with `Execute`/`Enable`; do not expect one-shot FB calls to retain behavior if you stop calling them.
4. Call `MC_Constants()` once during initialization before you use `PN_*` or `mcERR_*` members.
5. Use the readback blocks continuously when your state machine depends on live status, position, or error state.

## How To Read The Function Block Interfaces

- `VAR_IN_OUT`: caller-owned handles or payloads passed by reference, such as `Axis`, `Master`, `Slave`, `AxesGroup`, or `CamTable`.
- `VAR_INPUT`: ordinary input parameters copied into the function block each scan, such as `Execute`, `Enable`, `Position`, `Velocity`, or `BufferMode`.
- `VAR_OUTPUT`: status and result values produced by the function block, such as `Done`, `Busy`, `Active`, `Error`, `ErrorID`, and readback values.

## Single-axis core

Classic PLCopen Part 1 axis-control blocks. These are the normal starting point for one axis with power, homing, moves, stops, readbacks, and parameter access.

### Public Data Types

### `AXIS_REF`
Public handle for one motion axis. You keep one of these per axis and pass it by `VAR_IN_OUT` into the axis-related function blocks.
Fields:

- `AxisId : UDINT`: Stable public identifier for the axis in your application.
- `InternalIndex : UINT`: Runtime registry slot used by the library implementation.

### `MC_BUFFER_MODE`
Controls whether a command aborts the current motion, waits in a queue, or blends with adjacent motion commands.
Values:

- `mcAborting`: Abort the currently active command and start the new command immediately.
- `mcBuffered`: Queue the new command behind the active command.
- `mcBlendingLow`: Blend using the lower transition velocity policy.
- `mcBlendingPrevious`: Blend using the previous command velocity.
- `mcBlendingNext`: Blend using the next command velocity.
- `mcBlendingHigh`: Blend using the higher transition velocity policy.

### `MC_DIRECTION`
Selects how directional motion commands choose travel direction.
Values:

- `mcPositiveDirection`: Travel in the positive direction.
- `mcShortestWay`: Choose the shortest path supported by the axis model.
- `mcNegativeDirection`: Travel in the negative direction.
- `mcCurrentDirection`: Continue using the current motion direction.

### `MC_EXECUTION_MODE`
Controls how parameter, transform, and set-position style commands are applied.
Values:

- `mcImmediately`: Apply immediately when accepted.
- `mcDelayed`: Delayed execution mode. The current shipped profile rejects unsupported delayed paths.
- `mcQueued`: Queue behind already active/accepted work where supported.

### `MC_SOURCE`
Chooses whether a readback or synchronization calculation uses commanded, set, or actual values.
Values:

- `mcCommandedValue`: Use the commanded/planned value.
- `mcSetValue`: Use the setpoint value.
- `mcActualValue`: Use the measured/actual value.

### `MC_AXIS_STATUS`
Public axis-state enum used by the library state machine and exposed by status FBs.
Values:

- `mcErrorStop`: Axis is in error stop.
- `mcDisabled`: Axis is disabled.
- `mcStandstill`: Axis is powered and not moving.
- `mcHoming`: Axis is performing a homing action.
- `mcStopping`: Axis is stopping.
- `mcDiscreteMotion`: Axis is in discrete point-to-point motion.
- `mcContinuousMotion`: Axis is in continuous/velocity motion.
- `mcSynchronizedMotion`: Axis is currently synchronized to another motion source.

### Function Block Reference

### `MC_Power`
Enable or remove power-stage permission for one axis.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Status : BOOL`: Current achieved power/enabled state.
- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Call every scan. `Enable` is level-sensitive; the block is not a one-shot command.

### `MC_Home`
Run the basic PLCopen homing command and declare the homed position when the sequence completes.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Position : REAL`: Target or reference position value.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Typical sequence is `MC_Power` -> `MC_Home` -> wait for `Done` -> move commands.

### `MC_Stop`
Force an immediate stop sequence for one axis.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use for emergency or immediate stop handling. It does not rely on buffered blending behavior.

### `MC_Halt`
Request a halt while preserving buffered-motion semantics.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when you want a controlled halt that still participates in the PLCopen buffering model.

### `MC_MoveAbsolute`
Move one axis to an absolute target position.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `Position : REAL`: Target or reference position value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `Direction : MC_DIRECTION`: Requested travel direction policy.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Keep calling the block every scan after the rising edge; monitor `Busy`, `Active`, and `Done`.

### `MC_MoveRelative`
Move one axis by a relative distance.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `Distance : REAL`: Relative distance value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use buffered mode when you want this command to queue behind another motion command.

### `MC_MoveAdditive`
Add a relative distance onto the current commanded target.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `Distance : REAL`: Relative distance value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when the next target should be relative to the current commanded path rather than the actual position.

### `MC_MoveVelocity`
Run one axis in velocity mode.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `Direction : MC_DIRECTION`: Requested travel direction policy.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `InVelocity : BOOL`: TRUE when the commanded velocity is reached.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use `InVelocity` plus readback FBs when your state machine needs confirmation that the commanded velocity has been reached.

### `MC_MoveContinuousAbsolute`
Run an absolute move that aims to leave the segment with a requested end velocity.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `Position : REAL`: Target or reference position value.
- `EndVelocity : REAL`: Requested velocity at the end of the move segment.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `Direction : MC_DIRECTION`: Requested travel direction policy.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `InEndVelocity : BOOL`: TRUE when the commanded end-velocity condition is reached.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when segment-to-segment transitions need a non-zero end velocity.

### `MC_MoveContinuousRelative`
Run a relative move that aims to leave the segment with a requested end velocity.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `Distance : REAL`: Relative distance value.
- `EndVelocity : REAL`: Requested velocity at the end of the move segment.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `InEndVelocity : BOOL`: TRUE when the commanded end-velocity condition is reached.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use for relative segments that should leave the segment with a specific end velocity.

### `MC_SetPosition`
Overwrite the current position reference without commanding a move.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Position : REAL`: Target or reference position value.
- `Relative : BOOL`: If TRUE, interpret the position as a relative offset.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use carefully; this changes the reference position rather than commanding a travel path.

### `MC_SetOverride`
Apply velocity, acceleration, and jerk scaling factors.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `VelFactor : REAL`: Velocity override factor.
- `AccFactor : REAL`: Acceleration override factor.
- `JerkFactor : REAL`: Jerk override factor.
`VAR_OUTPUT`:

- `Enabled : BOOL`: TRUE while the override function is active.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Keep the block enabled while you want the override factors applied.

### `MC_ReadActualPosition`
Read the actual position from the library axis state.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Position : REAL`: Target or reference position value.

### `MC_ReadActualVelocity`
Read the actual velocity from the library axis state.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Velocity : REAL`: Requested velocity.

### `MC_ReadActualTorque`
Read the actual torque from the library axis state.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Torque : REAL`: Reported torque value.

### `MC_ReadStatus`
Read the high-level PLCopen axis-state bits.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `ErrorStop : BOOL`: TRUE when the axis is in error stop.
- `Disabled : BOOL`: TRUE when the axis or group is disabled.
- `Stopping : BOOL`: TRUE when the axis or group is stopping.
- `Homing : BOOL`: TRUE when the axis or group is homing.
- `Standstill : BOOL`: TRUE when the axis or group is standing still.
- `DiscreteMotion : BOOL`: TRUE during discrete point-to-point motion.
- `ContinuousMotion : BOOL`: TRUE during continuous/velocity motion.
- `SynchronizedMotion : BOOL`: TRUE during synchronized motion.

### `MC_ReadMotionState`
Read motion-phase flags such as accelerating or constant velocity.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `Source : MC_SOURCE`: Requested value source.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `ConstantVelocity : BOOL`: TRUE while velocity is constant.
- `Accelerating : BOOL`: TRUE while the object is accelerating.
- `Decelerating : BOOL`: TRUE while the object is decelerating.
- `DirectionPositive : BOOL`: TRUE when motion is in the positive direction.
- `DirectionNegative : BOOL`: TRUE when motion is in the negative direction.

### `MC_ReadAxisInfo`
Read axis readiness, switch states, homed state, and warnings.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `HomeAbsSwitch : BOOL`: Current absolute-home/reference switch state.
- `LimitSwitchPos : BOOL`: Current positive limit switch state.
- `LimitSwitchNeg : BOOL`: Current negative limit switch state.
- `Simulation : BOOL`: TRUE when the axis is in simulated mode.
- `CommunicationReady : BOOL`: TRUE when the backend communication path is ready.
- `ReadyForPowerOn : BOOL`: TRUE when the axis is ready to be powered.
- `PowerOn : BOOL`: TRUE when the axis power stage is on.
- `IsHomed : BOOL`: TRUE when the axis is homed.
- `AxisWarning : BOOL`: TRUE when the axis reports a warning state.

### `MC_ReadAxisError`
Read the current axis error code.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `AxisErrorID : WORD`: Current axis-level error code.

### `MC_Reset`
Clear axis errors and leave error stop.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_ReadParameter`
Read one numeric parameter from the axis parameter surface.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `ParameterNumber : INT`: Parameter identifier to read or write.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Value : REAL`: Value being read or written.

### `MC_ReadBoolParameter`
Read one boolean parameter from the axis parameter surface.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `ParameterNumber : INT`: Parameter identifier to read or write.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Value : BOOL`: Value being read or written.

### `MC_WriteParameter`
Write one numeric axis parameter.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ParameterNumber : INT`: Parameter identifier to read or write.
- `Value : REAL`: Value being read or written.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_WriteBoolParameter`
Write one boolean axis parameter.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ParameterNumber : INT`: Parameter identifier to read or write.
- `Value : BOOL`: Value being read or written.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_Constants`
Publish standardized parameter numbers and shipped motion error constants.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- none
`VAR_INPUT`:

- none
`VAR_OUTPUT`:

- `PN_CommandedPosition : INT`: `PN_CommandedPosition` value of type `INT`.
- `PN_SWLimitPos : INT`: `PN_SWLimitPos` value of type `INT`.
- `PN_SWLimitNeg : INT`: `PN_SWLimitNeg` value of type `INT`.
- `PN_EnableLimitPos : INT`: `PN_EnableLimitPos` value of type `INT`.
- `PN_EnableLimitNeg : INT`: `PN_EnableLimitNeg` value of type `INT`.
- `PN_EnablePosLagMonitoring : INT`: `PN_EnablePosLagMonitoring` value of type `INT`.
- `PN_MaxPositionLag : INT`: `PN_MaxPositionLag` value of type `INT`.
- `PN_MaxVelocitySystem : INT`: `PN_MaxVelocitySystem` value of type `INT`.
- `PN_MaxVelocityAppl : INT`: `PN_MaxVelocityAppl` value of type `INT`.
- `PN_ActualVelocity : INT`: `PN_ActualVelocity` value of type `INT`.
- `PN_CommandedVelocity : INT`: `PN_CommandedVelocity` value of type `INT`.
- `PN_MaxAccelerationSystem : INT`: `PN_MaxAccelerationSystem` value of type `INT`.
- `PN_MaxAccelerationAppl : INT`: `PN_MaxAccelerationAppl` value of type `INT`.
- `PN_MaxDecelerationSystem : INT`: `PN_MaxDecelerationSystem` value of type `INT`.
- `PN_MaxDecelerationAppl : INT`: `PN_MaxDecelerationAppl` value of type `INT`.
- `PN_MaxJerkSystem : INT`: `PN_MaxJerkSystem` value of type `INT`.
- `PN_MaxJerkAppl : INT`: `PN_MaxJerkAppl` value of type `INT`.
- `mcERR_None : WORD`: `mcERR_None` value of type `WORD`.
- `mcERR_InvalidParameter : WORD`: `mcERR_InvalidParameter` value of type `WORD`.
- `mcERR_InvalidState : WORD`: `mcERR_InvalidState` value of type `WORD`.
- `mcERR_AxisGrouped : WORD`: `mcERR_AxisGrouped` value of type `WORD`.
- `mcERR_GroupDisabled : WORD`: `mcERR_GroupDisabled` value of type `WORD`.
- `mcERR_GroupNotReady : WORD`: `mcERR_GroupNotReady` value of type `WORD`.
- `mcERR_NotHomed : WORD`: `mcERR_NotHomed` value of type `WORD`.
- `mcERR_NotPowered : WORD`: `mcERR_NotPowered` value of type `WORD`.
- `mcERR_BackendFault : WORD`: `mcERR_BackendFault` value of type `WORD`.
- `mcERR_KinematicNoSolution : WORD`: `mcERR_KinematicNoSolution` value of type `WORD`.
- `mcERR_KinematicSingularity : WORD`: `mcERR_KinematicSingularity` value of type `WORD`.
- `mcERR_QueueFull : WORD`: `mcERR_QueueFull` value of type `WORD`.
- `mcERR_NotSupported : WORD`: `mcERR_NotSupported` value of type `WORD`.
Usage notes: Call once during initialization and then reuse the published `PN_*` and `mcERR_*` members.

## Synchronization

Master/slave cam and gear blocks built on top of the single-axis handle types.

### Public Data Types

### `MC_START_MODE`
Defines how a synchronization command starts relative to the master axis.
Values:

- `mcAbsolute`: Synchronize using absolute master positioning.
- `mcRelative`: Synchronize using relative positioning.
- `mcRampIn`: Ramp into synchronization.

### `MC_SYNC_MODE`
Defines how `MC_GearInPos` reaches synchronization.
Values:

- `mcShortest`: Reach sync with the shortest path policy.
- `mcCatchUp`: Catch up to the master.
- `mcSlowDown`: Slow down to reach synchronization.

### `MC_CAM_ID`
Identifier for a selected cam table.
Underlying type: `UINT`

### `MC_CAM_REF`
Inline cam-table payload containing master/slave point pairs and table metadata.
Fields:

- `CamId : MC_CAM_ID`: Identifier of the cam table.
- `NumberOfPairs : UINT`: Number of valid master/slave point pairs in the table.
- `IsAbsolute : BOOL`: Whether the cam positions are absolute values.
- `MasterPosition0 : REAL`: Master-axis position for pair 0.
- `MasterPosition1 : REAL`: Master-axis position for pair 1.
- `MasterPosition2 : REAL`: Master-axis position for pair 2.
- `MasterPosition3 : REAL`: Master-axis position for pair 3.
- `MasterPosition4 : REAL`: Master-axis position for pair 4.
- `MasterPosition5 : REAL`: Master-axis position for pair 5.
- `MasterPosition6 : REAL`: Master-axis position for pair 6.
- `MasterPosition7 : REAL`: Master-axis position for pair 7.
- `SlavePosition0 : REAL`: Slave-axis position for pair 0.
- `SlavePosition1 : REAL`: Slave-axis position for pair 1.
- `SlavePosition2 : REAL`: Slave-axis position for pair 2.
- `SlavePosition3 : REAL`: Slave-axis position for pair 3.
- `SlavePosition4 : REAL`: Slave-axis position for pair 4.
- `SlavePosition5 : REAL`: Slave-axis position for pair 5.
- `SlavePosition6 : REAL`: Slave-axis position for pair 6.
- `SlavePosition7 : REAL`: Slave-axis position for pair 7.

### Function Block Reference

### `MC_CamTableSelect`
Select the cam profile a slave axis will use.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Master : AXIS_REF`: Master axis handle.
- `Slave : AXIS_REF`: Slave axis handle.
- `CamTable : MC_CAM_REF`: Cam table payload passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Periodic : BOOL`: TRUE when the cam table should wrap periodically.
- `MasterAbsolute : BOOL`: Treat master positions as absolute values.
- `SlaveAbsolute : BOOL`: Treat slave positions as absolute values.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CamTableID : MC_CAM_ID`: Identifier of the selected cam table.
Usage notes: Select the cam table before triggering `MC_CamIn`.

### `MC_CamIn`
Enter cam synchronization between master and slave.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Master : AXIS_REF`: Master axis handle.
- `Slave : AXIS_REF`: Slave axis handle.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `MasterOffset : REAL`: Offset applied to the master value.
- `SlaveOffset : REAL`: Offset applied to the slave value.
- `MasterScaling : REAL`: Scaling factor applied to the master value.
- `SlaveScaling : REAL`: Scaling factor applied to the slave value.
- `MasterStartDistance : REAL`: Distance before the sync point where synchronization begins.
- `MasterSyncPosition : REAL`: Master position used as the sync reference.
- `StartMode : MC_START_MODE`: Synchronization start mode.
- `MasterValueSource : MC_SOURCE`: Source used for the master axis value.
- `CamTableID : MC_CAM_ID`: Identifier of the selected cam table.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `InSync : BOOL`: TRUE when synchronization is reached.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `EndOfProfile : BOOL`: TRUE when the selected cam profile reached its end.
Usage notes: A typical flow is `MC_CamTableSelect` -> `MC_CamIn` -> wait for `InSync`.

### `MC_CamOut`
Leave cam synchronization.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Slave : AXIS_REF`: Slave axis handle.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GearIn`
Start geared motion between master and slave.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Master : AXIS_REF`: Master axis handle.
- `Slave : AXIS_REF`: Slave axis handle.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ContinuousUpdate : BOOL`: Allows the command parameters to be refreshed while the command is active where the profile supports it.
- `RatioNumerator : INT`: Gear ratio numerator.
- `RatioDenominator : UINT`: Gear ratio denominator.
- `MasterValueSource : MC_SOURCE`: Source used for the master axis value.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `InGear : BOOL`: TRUE when gearing is established.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use with one master and one slave that already share valid axis handles.

### `MC_GearOut`
Leave geared motion.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Slave : AXIS_REF`: Slave axis handle.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GearInPos`
Synchronize a slave to a master at specified sync positions.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Master : AXIS_REF`: Master axis handle.
- `Slave : AXIS_REF`: Slave axis handle.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `RatioNumerator : INT`: Gear ratio numerator.
- `RatioDenominator : UINT`: Gear ratio denominator.
- `MasterValueSource : MC_SOURCE`: Source used for the master axis value.
- `MasterSyncPosition : REAL`: Master position used as the sync reference.
- `SlaveSyncPosition : REAL`: Slave position used as the sync reference.
- `SyncMode : MC_SYNC_MODE`: Synchronization approach policy.
- `MasterStartDistance : REAL`: Distance before the sync point where synchronization begins.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `StartSync : BOOL`: TRUE when the sync approach has started.
- `InSync : BOOL`: TRUE when synchronization is reached.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when synchronization must happen at explicit master/slave positions rather than immediately.

## Coordinated motion

Group-level handles, position payloads, transforms, and coordinated move blocks.

### Public Data Types

### `AXIS_REF`
Public handle for one motion axis. You keep one of these per axis and pass it by `VAR_IN_OUT` into the axis-related function blocks.
Fields:

- `AxisId : UDINT`: Stable public identifier for the axis in your application.
- `InternalIndex : UINT`: Runtime registry slot used by the library implementation.

### `MC_BUFFER_MODE`
Controls whether a command aborts the current motion, waits in a queue, or blends with adjacent motion commands.
Values:

- `mcAborting`: Abort the currently active command and start the new command immediately.
- `mcBuffered`: Queue the new command behind the active command.
- `mcBlendingLow`: Blend using the lower transition velocity policy.
- `mcBlendingPrevious`: Blend using the previous command velocity.
- `mcBlendingNext`: Blend using the next command velocity.
- `mcBlendingHigh`: Blend using the higher transition velocity policy.

### `MC_EXECUTION_MODE`
Controls how parameter, transform, and set-position style commands are applied.
Values:

- `mcImmediately`: Apply immediately when accepted.
- `mcDelayed`: Delayed execution mode. The current shipped profile rejects unsupported delayed paths.
- `mcQueued`: Queue behind already active/accepted work where supported.

### `MC_SOURCE`
Chooses whether a readback or synchronization calculation uses commanded, set, or actual values.
Values:

- `mcCommandedValue`: Use the commanded/planned value.
- `mcSetValue`: Use the setpoint value.
- `mcActualValue`: Use the measured/actual value.

### `AXES_GROUP_REF`
Public handle for one coordinated-motion group.
Fields:

- `GroupId : UDINT`: Stable public identifier for the group.
- `InternalIndex : UINT`: Runtime registry slot used by the library implementation.

### `AXIS_ID`
Public axis identifier used by group readback blocks.
Underlying type: `UDINT`

### `AXES_GROUP_ID`
Public group identifier used by group readback blocks.
Underlying type: `UDINT`

### `IDENT_IN_GROUP_REF`
Stable member name used when adding an axis to a group and reading group configuration.
Underlying type: `STRING[63]`

### `MC_COMMAND_ID`
Identifier for a queued or active coordinated-motion command.
Underlying type: `UINT`

### `MC_GROUP_PARAMETER`
Supported group parameter names for group parameter read/write blocks.
Values:

- `mcDynamicsMode`: Group dynamics mode parameter.
- `mcTransitionReferencePoint`: Transition reference-point parameter.

### `MC_TRANSITION_PARAMETER`
Additional numeric parameter used by transition/blending modes.
Underlying type: `REAL`

### `MC_KIN_REF`
Handle for a kinematic transform profile.
Underlying type: `UINT`

### `MC_COORD_SYSTEM`
Coordinate-space selector for group readback, transforms, and moves.
Values:

- `mcACS`: Axis coordinate system.
- `mcMCS`: Machine coordinate system.
- `mcWCS`: World coordinate system.
- `mcPCS`: Product coordinate system.
- `mcFCS`: Fixture coordinate system.
- `mcTCS`: Tool coordinate system.

### `MC_DYNAMICS_MODE`
Defines whether dynamics values are interpreted as absolute values or percentages.
Values:

- `mcAbsolute`: Interpret dynamics as absolute values.
- `mcPercentage`: Interpret dynamics as percentages of reference/default dynamics.

### `MC_TRANSITION_MODE`
Defines the transition/blending model between coordinated moves.
Values:

- `mcTMNone`: No transition blending.
- `mcTMStartVelocity`: Blend using start velocity.
- `mcTMConstantVelocity`: Blend using constant velocity.
- `mcTMCornerDistance`: Blend using a corner-distance rule.
- `mcTMMaxCornerDeviation`: Blend using a max-corner-deviation rule.

### `MC_TRANSITION_VELOCITY`
Defines the velocity policy at a coordinated-motion transition.
Values:

- `mcTVZero`: Use zero transition velocity.
- `mcTVLow`: Use the lower velocity.
- `mcTVPrevious`: Use the previous segment velocity.
- `mcTVNext`: Use the next segment velocity.
- `mcTVHigh`: Use the higher velocity.

### `MC_TRANSITION_REFERENCE`
Defines whether a transition references the start or end of a segment.
Values:

- `mcStartPoint`: Reference the segment start point.
- `mcEndPoint`: Reference the segment end point.

### `MC_ORIENTATION_MODE`
Defines how orientation is handled during coordinated motion.
Values:

- `mcLinear`: Linearly interpolate orientation.
- `mcJointInterpolated`: Interpolate in joint space.
- `mcFixed`: Hold orientation fixed.
- `mcPathBased`: Derive orientation from the path.

### `MC_COMMAND_STATE`
Readback state for one coordinated-motion command.
Values:

- `mcAccepted`: Command has been accepted but may not be active yet.
- `mcActive`: Command is the active motion command.

### `MC_GROUP_STATUS`
Public group-state enum used by coordinated-motion readback blocks.
Values:

- `mcGroupErrorStop`: Group is in error stop.
- `mcGroupDisabled`: Group is disabled.
- `mcGroupStandby`: Group is enabled and waiting.
- `mcGroupHoming`: Group is homing.
- `mcGroupStopping`: Group is stopping.
- `mcGroupMoving`: Group is moving.

### `MC_CART_REF`
Cartesian position/orientation payload.
Fields:

- `X : REAL`: Translational X component.
- `Y : REAL`: Translational Y component.
- `Z : REAL`: Translational Z component.
- `RX : REAL`: Rotational component around X.
- `RY : REAL`: Rotational component around Y.
- `RZ : REAL`: Rotational component around Z.

### `MC_COORD_REF`
Coordinate-transform payload with the same field shape as `MC_CART_REF`.
Fields:

- `X : REAL`: Translational X component.
- `Y : REAL`: Translational Y component.
- `Z : REAL`: Translational Z component.
- `RX : REAL`: Rotational component around X.
- `RY : REAL`: Rotational component around Y.
- `RZ : REAL`: Rotational component around Z.

### `MC_CONFIG_DATA`
Configuration flags that accompany Cartesian positions.
Fields:

- `ConfigValid : BOOL`: TRUE when the configuration flags are valid.
- `Shoulder : BOOL`: Configuration flag for the shoulder branch.
- `Elbow : BOOL`: Configuration flag for the elbow branch.
- `Wrist : BOOL`: Configuration flag for the wrist branch.

### `MC_TURN_INFO`
Auxiliary turn information for articulated systems.
Fields:

- `ATurns : ARRAY[1..4] OF SINT`: Auxiliary turn counters for additional axes.

### `MC_CART_POS_REF`
Cartesian position payload including pose, configuration, turn info, and auxiliary axes.
Fields:

- `Tcp : MC_CART_REF`: Cartesian TCP pose.
- `Cfg : MC_CONFIG_DATA`: Configuration flags.
- `TurnInfo : MC_TURN_INFO`: Turn information for articulated systems.
- `AuxiliaryAxes : ARRAY[1..4] OF REAL`: Auxiliary-axis positions.

### `MC_AXES_POS_REF`
Axis-space position payload.
Fields:

- `Axes : ARRAY[1..4] OF REAL`: Axis-space position array.

### `MC_POS_REF`
Combined Cartesian and axis-space position payload.
Fields:

- `C : MC_CART_POS_REF`: Cartesian representation.
- `A : MC_AXES_POS_REF`: Axis-space representation.

### `MC_DISTANCE_REF`
Relative-distance payload for group relative moves.
Underlying type: `MC_POS_REF`

### `MC_SWLIMIT`
Positive and negative software limits for one group member.
Fields:

- `SWLimitPos : REAL`: Positive software limit.
- `SWLimitNeg : REAL`: Negative software limit.

### `MC_GROUP_SWLIMITS`
Array of software-limit pairs for the group.
Underlying type: `ARRAY[1..4] OF MC_SWLIMIT`

### Function Block Reference

### `MC_AddAxisToGroup`
Attach an axis to a coordinated-motion group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `IdentInGroup : IDENT_IN_GROUP_REF`: Stable group-member identifier.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Call once per member during group setup before enabling or moving the group.

### `MC_RemoveAxisFromGroup`
Detach one named axis from a group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `IdentInGroup : IDENT_IN_GROUP_REF`: Stable group-member identifier.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_UngroupAllAxes`
Remove every axis from a group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupReadConfiguration`
Read one configured member of a group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `IdentInGroup : IDENT_IN_GROUP_REF`: Stable group-member identifier.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Axis : AXIS_REF`: Shared axis handle passed by reference.
- `AxisID : AXIS_ID`: Returned public axis identifier.

### `MC_ReadAxisGroupInfo`
Read the group membership information for one axis.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
- `AxesGroupID : AXES_GROUP_ID`: Returned public group identifier.
- `IdentInGroup : IDENT_IN_GROUP_REF`: Stable group-member identifier.

### `MC_GroupEnable`
Enable the coordinated-motion group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Enable the group after all required members and transforms are configured.

### `MC_GroupDisable`
Disable the coordinated-motion group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupPower`
Apply power permission to the whole group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Status : BOOL`: Current achieved power/enabled state.
- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use this like `MC_Power`, but for the whole group handle.

### `MC_GroupReadStatus`
Read the high-level coordinated-motion group state.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `GroupMoving : BOOL`: TRUE while the group is moving.
- `GroupHoming : BOOL`: TRUE while the group is homing.
- `GroupErrorStop : BOOL`: TRUE while the group is in error stop.
- `GroupStandby : BOOL`: TRUE while the group is ready and idle.
- `GroupStopping : BOOL`: TRUE while the group is stopping.
- `GroupDisabled : BOOL`: TRUE while the group is disabled.

### `MC_GroupReadError`
Read the current group error code.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `GroupErrorID : WORD`: Current group error code.

### `MC_GroupReset`
Clear group errors.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupReadPosition`
Read current group position in the requested coordinate system.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `Source : MC_SOURCE`: Requested value source.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Position : MC_POS_REF`: Target or reference position value.

### `MC_GroupReadVelocity`
Read current group velocity in the requested coordinate system.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `Source : MC_SOURCE`: Requested value source.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Velocity : ARRAY[1..4] OF REAL`: Requested velocity.
- `PathVelocity : REAL`: Resulting path velocity.

### `MC_GroupReadAcceleration`
Read current group acceleration in the requested coordinate system.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `Source : MC_SOURCE`: Requested value source.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Acceleration : ARRAY[1..4] OF REAL`: Requested acceleration.
- `PathAcceleration : REAL`: Resulting path acceleration.

### `MC_GroupReadMotionState`
Read detailed motion flags and the active command ID for the group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Tracking : BOOL`: TRUE when the group is in a tracking state.
- `InSync : BOOL`: TRUE when synchronization is reached.
- `InPosition : BOOL`: TRUE when the group is in position.
- `Standstill : BOOL`: TRUE when the axis or group is standing still.
- `ConstantVelocity : BOOL`: TRUE while velocity is constant.
- `Accelerating : BOOL`: TRUE while the object is accelerating.
- `Decelerating : BOOL`: TRUE while the object is decelerating.
- `ActiveCommandID : MC_COMMAND_ID`: Identifier of the current active command.

### `MC_GroupReadParameter`
Read one group parameter.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `ParameterNumber : MC_GROUP_PARAMETER`: Parameter identifier to read or write.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Value : REAL`: Value being read or written.

### `MC_GroupWriteParameter`
Write one group parameter.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `ParameterNumber : MC_GROUP_PARAMETER`: Parameter identifier to read or write.
- `Value : REAL`: Value being read or written.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupReadSWLimits`
Read the configured software limits for the group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `LimitValues : MC_GROUP_SWLIMITS`: Software-limit payload.

### `MC_GroupWriteSWLimits`
Write the configured software limits for the group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `LimitValues : MC_GROUP_SWLIMITS`: Software-limit payload.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_SetKinTransform`
Select a kinematic transform profile for the group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `KinTransform : MC_KIN_REF`: Kinematic transform handle.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_SetCartesianTransform`
Set a Cartesian transform by explicit translation and rotation fields.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `TransX : REAL`: Translation in X.
- `TransY : REAL`: Translation in Y.
- `TransZ : REAL`: Translation in Z.
- `RotAngle1 : REAL`: First rotation angle.
- `RotAngle2 : REAL`: Second rotation angle.
- `RotAngle3 : REAL`: Third rotation angle.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_SetCoordinateTransform`
Set a Cartesian transform by a `MC_COORD_REF` payload.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `CoordTransform : MC_COORD_REF`: Coordinate-transform payload.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_ReadKinTransform`
Read the currently selected kinematic transform.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `KinTransform : MC_KIN_REF`: Kinematic transform handle.

### `MC_ReadCartesianTransform`
Read the current Cartesian transform fields.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `TransX : REAL`: Translation in X.
- `TransY : REAL`: Translation in Y.
- `TransZ : REAL`: Translation in Z.
- `RotAngle1 : REAL`: First rotation angle.
- `RotAngle2 : REAL`: Second rotation angle.
- `RotAngle3 : REAL`: Third rotation angle.

### `MC_ReadCoordinateTransform`
Read the current coordinate transform payload.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CoordTransform : MC_COORD_REF`: Coordinate-transform payload.

### `MC_GroupSetPosition`
Overwrite the group position reference.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Position : MC_POS_REF`: Target or reference position value.
- `Relative : BOOL`: If TRUE, interpret the position as a relative offset.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `ExecutionMode : MC_EXECUTION_MODE`: Execution policy for the write/set operation.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_MoveLinearAbsolute`
Run a linear absolute coordinated move.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Position : MC_POS_REF`: Target or reference position value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
- `TransitionVelocity : MC_TRANSITION_VELOCITY`: Transition velocity policy.
- `TransitionMode : MC_TRANSITION_MODE`: Transition/blending mode.
- `TransitionParameter : MC_TRANSITION_PARAMETER`: Additional transition parameter.
- `OrientationMode : MC_ORIENTATION_MODE`: Orientation interpolation mode.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
Usage notes: Use when path following matters and the move should be linear in the selected coordinate system.

### `MC_MoveLinearRelative`
Run a linear relative coordinated move.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Distance : MC_DISTANCE_REF`: Relative distance value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
- `TransitionVelocity : MC_TRANSITION_VELOCITY`: Transition velocity policy.
- `TransitionMode : MC_TRANSITION_MODE`: Transition/blending mode.
- `TransitionParameter : MC_TRANSITION_PARAMETER`: Additional transition parameter.
- `OrientationMode : MC_ORIENTATION_MODE`: Orientation interpolation mode.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
Usage notes: Use when the target is expressed as a relative delta rather than an absolute point.

### `MC_MoveDirectAbsolute`
Run a direct absolute coordinated move.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Position : MC_POS_REF`: Target or reference position value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
- `TransitionVelocity : MC_TRANSITION_VELOCITY`: Transition velocity policy.
- `TransitionMode : MC_TRANSITION_MODE`: Transition/blending mode.
- `TransitionParameter : MC_TRANSITION_PARAMETER`: Additional transition parameter.
- `OrientationMode : MC_ORIENTATION_MODE`: Orientation interpolation mode.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
Usage notes: Use when a direct move is acceptable and path linearity is not required.

### `MC_MoveDirectRelative`
Run a direct relative coordinated move.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Distance : MC_DISTANCE_REF`: Relative distance value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
- `TransitionVelocity : MC_TRANSITION_VELOCITY`: Transition velocity policy.
- `TransitionMode : MC_TRANSITION_MODE`: Transition/blending mode.
- `TransitionParameter : MC_TRANSITION_PARAMETER`: Additional transition parameter.
- `OrientationMode : MC_ORIENTATION_MODE`: Orientation interpolation mode.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
Usage notes: Relative version of the direct coordinated move.

### `MC_GroupHome`
Home a coordinated-motion group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Position : MC_POS_REF`: Target or reference position value.
- `CoordSystem : MC_COORD_SYSTEM`: Coordinate system used by the command or readback.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupStop`
Stop the whole group immediately.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupHalt`
Halt the group while preserving buffered semantics.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAccepted : BOOL`: TRUE when the command has been accepted.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupWaitTime`
Insert a timed wait command into the group command stream.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Duration : TIME`: Requested dwell time.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
Usage notes: Useful for inserting dwell periods into a buffered group-motion sequence.

### `MC_GroupSetOverride`
Apply group-wide override scaling factors.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `VelFactor : REAL`: Velocity override factor.
- `AccFactor : REAL`: Acceleration override factor.
- `JerkFactor : REAL`: Jerk override factor.
`VAR_OUTPUT`:

- `Enabled : BOOL`: TRUE while the override function is active.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_TransformPosition`
Transform one position payload between coordinate systems.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `InputPosition : MC_POS_REF`: Position to transform.
- `InputCoordSystem : MC_COORD_SYSTEM`: Coordinate system of the input position.
- `OutputCoordSystem : MC_COORD_SYSTEM`: Coordinate system requested for the output position.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
- `OutputPosition : MC_POS_REF`: Transformed position payload.
- `SingularPosition : BOOL`: TRUE when the position is singular or near-singular.
Usage notes: Use for coordinate conversion or preflight checks before issuing a group move.

### `MC_GroupReadCommandInfo`
Read tracking/progress information for one coordinated-motion command.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
- `CommandID : MC_COMMAND_ID`: Identifier assigned to the accepted command.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `CommandState : MC_COMMAND_STATE`: Returned state of the tracked command.
- `ElapsedDuration : TIME`: Elapsed command time.
- `RemainingDuration : TIME`: Estimated remaining command time.
- `RemainingDistance : REAL`: Estimated remaining path distance.
- `Progress : REAL`: Normalized command progress value.
- `InfoID : WORD`: Additional informational code.
- `WarningID : WORD`: Additional warning code.
Usage notes: Use when you need progress or timing feedback for long-running group commands.

### `MC_GroupWriteReferenceDynamics`
Write the reference dynamics used by the group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupReadReferenceDynamics`
Read the reference dynamics used by the group.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.

### `MC_GroupWriteDefaultDynamics`
Write the default group dynamics values.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_GroupReadDefaultDynamics`
Read the default group dynamics values.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `AxesGroup : AXES_GROUP_REF`: Shared group handle passed by reference.
`VAR_INPUT`:

- `Enable : BOOL`: Level-sensitive enable input. Keep it TRUE while you want the block active or the readback valid.
`VAR_OUTPUT`:

- `Valid : BOOL`: TRUE when the readback outputs are valid this scan.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.

## Homing

Part 5 homing toolkit blocks for step-by-step homing procedures beyond the basic `MC_Home` block.

### Public Data Types

### `MC_HOME_DIRECTION`
Direction selector used by the advanced homing step blocks.
Values:

- `mcPositiveDirection`: Travel in the positive direction.
- `mcNegativeDirection`: Travel in the negative direction.
- `mcSwitchPositive`: Determine direction from a positive switch interpretation.
- `mcSwitchNegative`: Determine direction from a negative switch interpretation.

### `MC_SWITCH_MODE`
Defines how a switch or reference signal is interpreted during homing.
Values:

- `mcOn`: Trigger when the signal is ON.
- `mcOff`: Trigger when the signal is OFF.
- `mcRisingEdge`: Trigger on a rising edge.
- `mcFallingEdge`: Trigger on a falling edge.
- `mcEdgeSwitchPositive`: Trigger on the positive-direction edge rule.
- `mcEdgeSwitchNegative`: Trigger on the negative-direction edge rule.

### `MC_REF_SIGNAL_REF`
Reference-signal payload containing the signal state and optional metadata.
Fields:

- `Signal : BOOL`: Current signal state.
- `PositionStamp : REAL`: Position sampled with the signal.
- `MarkerCode : UINT`: Optional marker/reference code.

### Function Block Reference

### `MC_StepAbsoluteSwitch`
Run a homing step that finishes when an absolute reference signal matches the requested switch mode.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Direction : MC_HOME_DIRECTION`: Requested travel direction policy.
- `SwitchMode : MC_SWITCH_MODE`: Reference-switch evaluation mode.
- `ReferenceSignal : MC_REF_SIGNAL_REF`: Reference signal payload.
- `Velocity : REAL`: Requested velocity.
- `SetPosition : REAL`: Position to stamp into the axis when the homing step finishes.
- `TorqueLimit : REAL`: Torque limit used during the homing step.
- `TimeLimit : TIME`: Maximum allowed homing-step time.
- `DistanceLimit : REAL`: Maximum allowed homing-step travel distance.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Usually one part of a larger homing recipe.

### `MC_StepLimitSwitch`
Run a homing step that finishes when the selected limit switch matches the requested mode.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Direction : MC_HOME_DIRECTION`: Requested travel direction policy.
- `LimitSwitchMode : MC_SWITCH_MODE`: Limit-switch evaluation mode.
- `Velocity : REAL`: Requested velocity.
- `SetPosition : REAL`: Position to stamp into the axis when the homing step finishes.
- `TorqueLimit : REAL`: Torque limit used during the homing step.
- `TimeLimit : TIME`: Maximum allowed homing-step time.
- `DistanceLimit : REAL`: Maximum allowed homing-step travel distance.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Choose the direction and switch mode so the step matches your machine hardware.

### `MC_StepBlock`
Run a homing step that detects a block/hard-stop condition.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Direction : MC_HOME_DIRECTION`: Requested travel direction policy.
- `Velocity : REAL`: Requested velocity.
- `SetPosition : REAL`: Position to stamp into the axis when the homing step finishes.
- `DetectionVelocityLimit : REAL`: Velocity threshold used to detect a block condition.
- `DetectionVelocityTime : TIME`: Time window for the block-detection velocity threshold.
- `TorqueLimit : REAL`: Torque limit used during the homing step.
- `TimeLimit : TIME`: Maximum allowed homing-step time.
- `DistanceLimit : REAL`: Maximum allowed homing-step travel distance.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when a hard-stop or torque-detection style homing step is required.

### `MC_StepReferencePulse`
Run a homing step that finishes on a reference pulse.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Direction : MC_HOME_DIRECTION`: Requested travel direction policy.
- `ReferenceSignal : MC_REF_SIGNAL_REF`: Reference signal payload.
- `Velocity : REAL`: Requested velocity.
- `SetPosition : REAL`: Position to stamp into the axis when the homing step finishes.
- `TorqueLimit : REAL`: Torque limit used during the homing step.
- `TimeLimit : TIME`: Maximum allowed homing-step time.
- `DistanceLimit : REAL`: Maximum allowed homing-step travel distance.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when your axis hardware provides a dedicated reference pulse.

### `MC_StepDistanceCoded`
Run a homing step that relies on distance-coded movement.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Direction : MC_HOME_DIRECTION`: Requested travel direction policy.
- `Velocity : REAL`: Requested velocity.
- `TorqueLimit : REAL`: Torque limit used during the homing step.
- `TimeLimit : TIME`: Maximum allowed homing-step time.
- `DistanceLimit : REAL`: Maximum allowed homing-step travel distance.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_HomeDirect`
Directly mark an axis as homed at a chosen position.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `SetPosition : REAL`: Position to stamp into the axis when the homing step finishes.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use when an external mechanism has already established the absolute position.

### `MC_HomeAbsolute`
Finish homing by using the current absolute reference.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.

### `MC_FinishHoming`
Complete the final move after the earlier homing steps have found the reference.
Type: `FUNCTION_BLOCK`
`VAR_IN_OUT`:

- `Axis : AXIS_REF`: Shared axis handle passed by reference.
`VAR_INPUT`:

- `Execute : BOOL`: Command trigger input. Use a rising edge to request a new command, then keep calling the FB every scan.
- `Distance : REAL`: Relative distance value.
- `Velocity : REAL`: Requested velocity.
- `Acceleration : REAL`: Requested acceleration.
- `Deceleration : REAL`: Requested deceleration.
- `Jerk : REAL`: Requested jerk.
- `BufferMode : MC_BUFFER_MODE`: PLCopen buffering/blending mode for the command.
`VAR_OUTPUT`:

- `Done : BOOL`: TRUE when the requested command has completed successfully.
- `Busy : BOOL`: TRUE while the command is accepted and still in progress.
- `Active : BOOL`: TRUE while this FB owns the currently active motion command.
- `CommandAborted : BOOL`: TRUE when the command was aborted by another accepted command or stop condition.
- `Error : BOOL`: TRUE when the FB reports an error.
- `ErrorID : WORD`: Current FB error code.
Usage notes: Use as the closing step after earlier homing steps locate the reference.

## Example And Further Reading

- Example walkthrough: `examples/plcopen_motion_single_axis_demo/README.md`
- Coverage matrix: `docs/specs/coverage/plcopen-motion-coverage.md`
- Performance-only material: `examples/plcopen_motion_single_axis_benchmarks/README.md`
