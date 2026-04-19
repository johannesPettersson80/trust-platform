# PLCopen Motion Single-Axis Demo

Docs category: `docs/public/examples/libraries-and-motion.md`

This example is the reference consumer for
`libraries/plcopen_motion/single_axis_core`.

Use it when you want to understand how a truST project should wire the shipped
PLCopen motion library into a normal scan-driven application.

## What The Demo Does

The program runs one repeatable single-axis sequence:

1. power the axis
2. write a positive software limit of `90.0`
3. enable that positive software limit
4. home the axis to `0.0`
5. run `MC_MoveAbsolute` to `80.0`
6. queue a buffered `MC_MoveRelative` of `+15.0`
7. verify that the relative move clamps at the configured `90.0` limit
8. run `MC_MoveVelocity`
9. stop the axis
10. return to the move step and repeat

This is intentionally not a "hello world". It shows the normal PLCopen usage
pattern of:

- one shared `AXIS_REF`
- one FB instance per command/readback block
- one-time `MC_Constants()` loading
- every FB called every scan
- sequencing driven by `.Done`, `.Busy`, `.Active`, and readback state

## Files To Study

- `trust-lsp.toml`: project settings and dependency on `single_axis_core`
- `runtime.toml`: runtime setup for the example
- `io.toml`: simulated I/O backend so the example runs without hardware
- `src/Globals.st`: observability globals the program updates while it runs
- `src/Main.st`: the actual scan-driven motion sequence

## How The Example Works

### 1. Project Dependency

The demo consumes the motion package exactly the same way your own project
would:

```toml
[dependencies]
PLCopenMotionSingleAxis = { path = "../../libraries/plcopen_motion/single_axis_core", version = "0.1.0" }
```

### 2. Axis Handle And FB Instances

At the top of `src/Main.st`, the program declares:

- `Axis : AXIS_REF` as the shared public handle for the motion axis
- command FBs such as `MC_Power`, `MC_Home`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveVelocity`, `MC_Stop`
- parameter FBs such as `MC_WriteParameter` and `MC_WriteBoolParameter`
- readback FBs such as `MC_ReadStatus`, `MC_ReadAxisInfo`, `MC_ReadActualPosition`, and `MC_ReadActualVelocity`

Those FB instances persist across scans. That is the normal PLCopen usage
pattern.

### 3. One-Time Initialization

The first initialization block does three things:

1. calls `MC_Constants()` once so the example can read `PN_*` parameter names and `mcERR_*` codes
2. fills the shared `Axis` handle with `AxisId` and `InternalIndex`
3. clears the demo globals so the sequence starts from a known state

### 4. Every FB Runs Every Scan

The program does not call one command FB once and then move on. Instead, every
scan it calls:

- `MC_Power`
- the active command FBs (`MC_Home`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveVelocity`, `MC_Stop`)
- the parameter-write FBs
- the readback FBs

The state machine changes the `Execute` inputs. The FB instances keep the
command state from scan to scan.

### 5. Step Machine

`g_motion_demo_current_step` drives the sequence.

| Step | Meaning |
| --- | --- |
| `10` | Write the positive software limit parameter. |
| `20` | Enable the positive software limit. |
| `30` | Wait until power is confirmed. |
| `40` / `41` | Start homing, then wait for `MC_Home.Done`. |
| `50` | Start the absolute move to `80.0`. |
| `51` | While the absolute move is active, request the buffered relative move. |
| `52` | Wait until the buffered relative move becomes active. |
| `53` | Wait for the relative move to finish, then verify the axis stopped near `90.0`. |
| `60` | Start the velocity move. |
| `61` | Request stop while the velocity command is active. |
| `62` | Wait for standstill, count one completed sequence, then loop back to `50`. |
| `900` | Fault/stop state used when an error or timeout is latched. |

### 6. Error And Timeout Handling

The demo latches the first non-zero error from the command/readback FBs into
`g_motion_demo_last_error`.

It also keeps a simple per-step scan counter. If a step takes too long without
advancing, the program sets `mcERR_BackendFault` and moves to step `900`
instead of hanging silently.

## Build, Validate, Run

From repository root:

```bash
trust-runtime build --project examples/plcopen_motion_single_axis_demo --sources src
trust-runtime validate --project examples/plcopen_motion_single_axis_demo
trust-runtime run --project examples/plcopen_motion_single_axis_demo
```

In another terminal, you can inspect the runtime:

```bash
trust-runtime ctl --project examples/plcopen_motion_single_axis_demo status
```

The project uses `io.toml` with `driver = "simulated"`, so it runs without real
motion hardware.

## What To Watch While It Runs

These globals make the sequence easy to follow:

- `g_motion_demo_completed_sequences`: increments each time the loop completes
- `g_motion_demo_last_error`: stays at `0` on the healthy path
- `g_motion_demo_current_step`: current state-machine step
- `g_motion_demo_limit_clamp_verified`: becomes `TRUE` once the buffered move clamps at `90.0`
- `g_motion_demo_last_position`: latest position from `MC_ReadActualPosition`
- `g_motion_demo_last_velocity`: latest velocity from `MC_ReadActualVelocity`
- `g_motion_demo_power_on`: mirrors `MC_Power.Status`
- `g_motion_demo_is_homed`: mirrors `MC_ReadAxisInfo.IsHomed`

## How To Reuse This Pattern In Your Own Project

1. Copy the dependency pattern from `trust-lsp.toml`.
2. Keep one shared `AXIS_REF` for each axis you control.
3. Instantiate the PLCopen FBs once and call them every scan.
4. Call `MC_Constants()` once before using parameter numbers such as `PN_SWLimitPos`.
5. Start with `MC_Power`, one move FB, and one readback FB; then add buffered moves, limits, and error handling.
6. Replace the simulated `io.toml` with your real driver and mapping when you move to hardware.

## Library Reference

For the full library reference, including the shipped motion data types and all
public FB input/output surfaces, see
`docs/guides/PLCOPEN_MOTION_LIBRARY_GUIDE.md`.
