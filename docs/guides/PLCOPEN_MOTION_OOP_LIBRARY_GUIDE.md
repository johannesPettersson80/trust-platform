# PLCopen Motion OOP Library Guide

This guide documents the object-oriented PLCopen Motion package shipped with
truST. The package follows the PLCopen OOP application examples while adapting
them to truST's deterministic ST runtime and the existing classic motion
function-block kernels.

The classic function-block packages remain the primary PLCopen compliance
surface. The OOP package is a second user-facing facade for projects that want
axis and command objects.

## Package Layout

- `libraries/plcopen_motion/oop`: OOP interfaces, command objects, and the
  `MC_OopAxis` adapter.
- `libraries/plcopen_motion/single_axis_core`: classic single-axis FB package
  used internally by `MC_OopAxis`.

## Dependency Setup

```toml
[project]
vendor_profile = "codesys"
include_paths = ["src"]
stdlib = "iec"

[dependencies]
PLCopenMotionOop = { path = "../../libraries/plcopen_motion/oop", version = "0.1.0" }
```

## Usage Pattern

Declare one concrete `MC_OopAxis` per controlled axis, bind it to a truST axis
slot, then store it behind the PLCopen `itfAxis` interface when the application
should use the OOP surface.

```iecst
PROGRAM Main
VAR
    RawAxis : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    MoveCommand : itfAxisCommand;
    Result : MC_ERROR;
END_VAR

Result := RawAxis.Bind(AxisId := UDINT#1, InternalIndex := UINT#1);
AxisObject := RawAxis;

PowerCommand := AxisObject.Power(
    Enable := TRUE,
    EnablePositive := TRUE,
    EnableNegative := TRUE
);

IF PowerCommand.Done THEN
    MoveCommand := AxisObject.MoveAbsolute(
        Position := REAL#100.0,
        Velocity := REAL#25.0,
        Acceleration := REAL#10.0,
        Deceleration := REAL#10.0,
        Jerk := REAL#1.0,
        Direction := MC_DIRECTION#mcPositiveDirection,
        BufferMode := MC_BUFFER_MODE#mcAborting
    );
END_IF;
END_PROGRAM
```

`Bind` is a truST-specific adapter method. PLCopen's OOP examples intentionally
model axis objects and do not standardize how a runtime binds an object to a
physical or simulated axis handle.

## Interfaces

### `itfCommand`

Common command object interface. Implementations expose:

- `Done : BOOL`
- `Busy : BOOL`
- `Active : BOOL`
- `CommandAborted : BOOL`
- `Error : BOOL`
- `ErrorId : MC_ERROR`
- `Abort() : MC_ERROR`
- `Wait(Timeout, AbortOnTimeout) : MC_ERROR`

### `itfAxisCommand`

Extends `itfCommand` for commands that can accept updated motion parameters.

- `Update(Position, Velocity, EndVelocity, Acceleration, Deceleration, Jerk) : MC_ERROR`

The current single-axis OOP package publishes the method and returns
`mcERR_NotSupported` until continuous in-place command updates are expanded.

### `itfContinuousAxisCommand`

Extends `itfAxisCommand`.

- `InVelocity : BOOL`

### `itfSynchronizedAxisCommand`

Extends `itfAxisCommand`.

- `InSync : BOOL`

Synchronized command objects are present so PLCopen OOP method signatures are
available. The current single-axis OOP package returns deterministic
`mcERR_NotSupported` command objects for synchronization/profile methods that
are not yet implemented by the OOP facade.

### `itfAxis`

Axis object interface. It exposes readback properties such as:

- `ActualPosition`, `ActualVelocity`, `ActualTorque`
- `PowerOn`, `ReadyForPowerOn`, `CommunicationReady`
- `Status`, `MotionStatus`, `Direction`, `ErrorId`
- `IsHomed`, `AxisWarning`, limit-switch flags, and simulation flags

It also exposes control and motion methods:

- `Power`
- `Home`
- `Stop`
- `Halt`
- `MoveAbsolute`
- `MoveRelative`
- `MoveAdditive`
- `MoveVelocity`
- `MoveContinuousAbsolute`
- `MoveContinuousRelative`
- `SetPosition`
- `SetOverride`
- `Reset`
- `ReadParameter`, `WriteParameter`
- `ReadBoolParameter`, `WriteBoolParameter`
- PLCopen synchronization/profile method names with deterministic unsupported
  command objects where the current OOP package does not implement behavior.

## Concrete Objects

### `MC_OopCommand`

Concrete implementation of `itfCommand`. It stores command state and exposes it
through PLCopen properties.

### `MC_OopAxisCommand`

Concrete implementation of `itfAxisCommand`. It extends `MC_OopCommand`.

### `MC_OopContinuousAxisCommand`

Concrete implementation of `itfContinuousAxisCommand`. It adds `InVelocity`.

### `MC_OopSynchronizedAxisCommand`

Concrete implementation of `itfSynchronizedAxisCommand`. It adds `InSync`.

### `MC_OopAxis`

Concrete implementation of `itfAxis`. It owns the classic single-axis FB
instances and command objects needed to adapt method calls to existing
single-axis kernel behavior.

`MC_OopAxis` does not duplicate the classic motion queue/state kernel. It calls
the classic FBs and maps their outputs into returned command objects and cached
axis properties.

## Unsupported Methods

The package keeps required PLCopen OOP method names visible even when the
current single-axis OOP facade does not implement the physical behavior.
Unsupported methods return a command object with:

- `Error = TRUE`
- `ErrorId = WORD#16#0500` (`mcERR_NotSupported`)

This applies to profile, probe, digital-cam, torque/superimposed, cam/gear, and
multi-axis synchronization paths that are outside the current OOP facade scope.

## Examples

Real-world OOP examples live under:

- `examples/plcopen_motion_oop_warehouse_shuttle`
- `examples/plcopen_motion_oop_labeling_conveyor`
- `examples/plcopen_motion_oop_pick_place_lift`
- `examples/plcopen_motion_oop_indexing_table`
- `examples/plcopen_motion_oop_feeder_axis`

Build any example with:

```bash
trust-runtime build --project examples/plcopen_motion_oop_warehouse_shuttle --sources src
```
