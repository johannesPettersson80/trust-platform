# PLCopen Motion: A Single-Axis Sequence

Run a program that powers a virtual axis, sets a software limit, homes the axis and repeats a sequence of moves
with the PLCopen motion blocks, and follow it from the command line. You learn how a PLCopen program calls its
blocks every scan and steps on their outputs, how a buffered move waits for the move before it, and how the
program fails closed.

## What you learn

- Every block is called every scan with the one shared `AXIS_REF`, and a step number decides which block's
  `Execute` is TRUE; the program goes to its next step on the blocks' `Done`, `Busy` and `Active` outputs.
- A move with `BufferMode := mcBuffered` waits behind the active move and starts when that one ends, and an
  enabled software limit stops a move at the limit.
- The program latches the first error a block reports, and a step that stalls, goes to its fault step and
  removes the power, which aborts the axis's moves.

## What you need

- This repository, whose `examples/plcopen_motion_single_axis_demo` and the library it uses,
  `libraries/plcopen_motion/single_axis_core`, you copy, and `trust-runtime`, from the truST extension for VS Code
  or a release archive, on the `PATH`. The program runs on simulated I/O and a virtual axis, without hardware.
- Linux or macOS, whose shell runs the commands below, and two terminals. About 15 minutes.

## Step by step

1. From the root of the repository, copy the project and its library, keeping the path between them:

    ```bash
    rm -rf /tmp/trust-motion
    mkdir -p /tmp/trust-motion/examples /tmp/trust-motion/libraries/plcopen_motion
    cp -R examples/plcopen_motion_single_axis_demo /tmp/trust-motion/examples/
    cp -R libraries/plcopen_motion/single_axis_core /tmp/trust-motion/libraries/plcopen_motion/
    cd /tmp/trust-motion/examples/plcopen_motion_single_axis_demo
    ```

    `trust-lsp.toml` names the library by that path:

    ```toml
    [dependencies]
    PLCopenMotionSingleAxis = { path = "../../libraries/plcopen_motion/single_axis_core", version = "0.1.0" }
    ```

2. Read how `src/Main.st` calls its blocks. It declares one `Axis : AXIS_REF` and one instance of each block, and
   calls every block every scan with that axis. The step `g_motion_demo_current_step` decides which `Execute` is
   TRUE, as in the two moves of the sequence:

    ```st
    MoveAbsolute(
        Axis := Axis,
        Execute :=
            (g_motion_demo_current_step = UINT#50) OR
            (g_motion_demo_current_step = UINT#51),
        ContinuousUpdate := FALSE,
        Position := REAL#80.0,
        Velocity := REAL#25.0,
        ...
        BufferMode := MC_BUFFER_MODE#mcAborting
    );
    MoveRelative(
        Axis := Axis,
        Execute :=
            (g_motion_demo_current_step = UINT#51) OR
            (g_motion_demo_current_step = UINT#52),
        ContinuousUpdate := FALSE,
        Distance := REAL#15.0,
        Velocity := REAL#20.0,
        ...
        BufferMode := MC_BUFFER_MODE#mcBuffered
    );
    ```

    A block keeps its command from scan to scan; the program changes only its inputs.

3. Read the sequence, the `CASE` at the end of the program:

    | Step | What the program does | It goes on when |
    | --- | --- | --- |
    | `10` | writes the positive software limit, `90.0` | the axis is powered and the write is done |
    | `20` | enables that limit | the write is done |
    | `30` | waits for the power | `MC_Power.Status` |
    | `40`, `41` | homes the axis at `0.0` | the homing is busy, then done |
    | `50` | starts the absolute move to `80.0` | the move is busy and active |
    | `51` | requests the buffered relative move of `+15.0` | the relative move is busy and not active yet |
    | `52` | ends the absolute move | the relative move is active |
    | `53` | ends the relative move and checks that the axis stopped at `90.0` | the relative move is done |
    | `60` | starts the velocity move | the move is active and at its velocity |
    | `61` | stops the axis | `MC_Stop.Done` |
    | `62` | counts the sequence and goes back to `50` | the axis stands still |
    | `900` | the fault step | never |

    Before the `CASE`, the program latches the first error that a block reports into
    `g_motion_demo_last_error`; after it, a step that lasts more than 32 scans sets `mcERR_BackendFault`. Either
    takes the program to step `900`, where it calls `MC_Power` with `Enable` FALSE.

4. Build the program and check the project:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    ```

    `build` lists `Sources: 5 file(s)`: the project's `src/Globals.st` and `src/Main.st` and the library's three
    files. `validate` answers `Project ok`.

5. Start the runtime in the second terminal with `trust-runtime run --project .`. In the first, check it:

    ```bash
    trust-runtime ctl --project . status
    ```

    It answers `state=running fault=none rt_profile=disabled rt_active=false`.

6. Read the sequence's globals:

    ```bash
    trust-runtime ctl --project . read g_motion_demo_current_step g_motion_demo_completed_sequences g_motion_demo_limit_clamp_verified g_motion_demo_last_position g_motion_demo_last_error g_motion_demo_power_on g_motion_demo_is_homed
    ```

    ```text
    scan=228 time_ns=2280132485 state=running quality=good
    g_motion_demo_current_step : UINT = 50
    g_motion_demo_completed_sequences : UDINT = 32
    g_motion_demo_limit_clamp_verified : BOOL = true
    g_motion_demo_last_position : REAL = 90.0
    g_motion_demo_last_error : WORD = 0
    g_motion_demo_power_on : BOOL = true
    g_motion_demo_is_homed : BOOL = true
    ```

    The values are those that one completed scan left, scan 228 here. The axis is powered and homed, no block has
    reported an error, and the relative move stopped at the limit, `90.0`. A read that shows step `53` finds the axis
    at `80.0`, between the two moves. Read again two seconds later: in the run, scan 432 showed 61 sequences, 29
    more in 204 scans.

7. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`.

## What happened

**Blocks called every scan.** A PLCopen block does its work while it is called: a command starts on the rising
edge of `Execute` and reports its progress in later calls, through `Busy`, `Active`, `Done` and `Error`. So the
program calls every block in every scan, whatever the step, and the step only chooses which `Execute` inputs are
TRUE. The readback blocks, `MC_ReadStatus`, `MC_ReadAxisInfo`, `MC_ReadActualPosition` and the others, run with
`Enable` TRUE and give the values the globals show.

**A buffered move.** In step `51` the relative move's `Execute` rises while the absolute move is still active.
With `mcBuffered` the library queues it: `Busy` TRUE, `Active` FALSE. When the absolute move ends in step `52`, the
queued move becomes active, and when it ends in step `53`, the axis would stand at 80 + 15 = 95. The software limit
that steps `10` and `20` set and enabled stops it at `90.0`, and the program records that in
`g_motion_demo_limit_clamp_verified`.

**The virtual axis.** The library's virtual axis has no motion profile yet: a move ends at once, at its target, in
the scan whose call takes `Execute` back to FALSE (the library guide, usage rule 6). So each step from `50` to `62`
lasts one 10 ms scan, a sequence takes seven, and the counter climbs by about 14 every second. A real drive would
take the time that the velocity and the accelerations give.

**Failing closed.** `MC_Power` is called with `Enable := g_motion_demo_current_step <> UINT#900`. In step `900` the
power goes off: the axis goes to Disabled, the library aborts its moves, and a motion block called on it is refused
with `mcERR_NotPowered`, as PLCopen Part 1 §2.1 requires. The program stays in step `900` until it is restarted, and
`g_motion_demo_last_error` keeps the first error, the one that names the cause.

## Try it yourself

Stop the runtime before each change, then build and start it again, and read the globals as in step 6.

- Make the buffered move fit: in the call of `MoveRelative`, change `Distance := REAL#15.0` to `REAL#5.0`. The
  relative move ends at 85.0, inside the limit: in the run, a read in step `60` showed
  `g_motion_demo_last_position` 85.0 and `g_motion_demo_limit_clamp_verified` FALSE, as no clamp was needed.
- Turn the limit off: in the call of `WriteEnableLimitPos`, change `Value := TRUE` to `Value := FALSE`.
  Nothing stops the relative move: a read in step `60` showed the position 95.0 and the check FALSE.
- Give the absolute move an acceleration it refuses: in the call of `MoveAbsolute`, change
  `Acceleration := REAL#0.0` to `REAL#-1.0`. `MC_MoveAbsolute` refuses it with `ErrorID` 1
  (`mcERR_InvalidParameter`), the program latches that error and goes to step `900`, and the power goes off. The
  read showed step `900`, `g_motion_demo_last_error` 1, `g_motion_demo_power_on` FALSE and
  `g_motion_demo_is_homed` TRUE, with the axis at 0.0, where homing left it, and no sequence completed.

## Reference

- `docs/guides/PLCOPEN_MOTION_LIBRARY_GUIDE.md`: the library's blocks, their inputs and outputs, and its usage
  rules.
- `docs/specs/coverage/plcopen-motion-coverage.md`: what the library implements of PLCopen Motion Control, and
  where the virtual axis differs.
- `examples/plcopen-motion-single-axis/README.md`: the starter, which teaches the rising edge of `Execute` in
  VS Code.
- To use the pattern in your own project: copy the dependency line of `trust-lsp.toml`, keep one `AXIS_REF` per
  axis, declare each block once and call it every scan, call `MC_Constants` once before you use its `PN_*` and
  `mcERR_*` numbers, and replace the simulated `io.toml` with your driver and mapping when you move to hardware.
