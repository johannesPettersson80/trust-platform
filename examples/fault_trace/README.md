# Fault trace

A dosing pump's controller faults when its stroke sensor breaks. You save the flight recorder's trace of
the fault, replay it on your computer to see the fault again, and check a fix against it.

## What you learn

- The flight recorder always keeps the last cycles of the running program, and after a fault it keeps the
  cycles up to the faulted one: the fault's trace.
- `trust-runtime ctl fault-trace` saves that trace as a file, and the runtime has already written it into
  the project's `traces/` folder.
- `trust-dev replay` runs the trace's cycles again in a simulator of the project and shows the same fault.
  A changed program shows the first cycle where it acts otherwise.
- A machine test keeps a fault's trace with trust-sim and replays it, so that CI runs it.

## What you need

- truST in VS Code, which brings `trust-runtime` and `trust-dev`. No hardware: `simulation.toml` stands in
  for the stroke sensor.
- Two terminals in the project's folder.
- For the machine test only: Rust from [rustup](https://rustup.rs).
- About 15 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **Fault trace**. Pick a folder under
   **Select destination folder** and keep the name under **New project folder name**: VS Code opens the
   copy. Open two terminals in its folder.
2. Open `src/Main.st`. `DosePerStroke := DosePerMinute / StrokeRate;` divides the 1,200 units of chlorine
   the water needs a minute by the strokes a minute that the stroke sensor counts at `%IW0`. Nothing
   guards the division against 0.
3. Open `simulation.toml`: the stroke sensor counts 60 strokes a minute from the start and 0 from 5 s, as
   when its wire breaks. The last section of `runtime.toml`, `[runtime.trace] flight_recorder = 300`,
   makes the runtime keep the last 300 to 600 cycles, 3 to 6 s at the task's 10 ms.
4. In the first terminal, build the project and run it with its simulation:

    ```text
    trust-runtime build --project .
    trust-runtime run --project . --simulation
    ```

    Five seconds after the start, the runtime logs the fault, a `runtime_fault` line with
    `"error":"division by zero"` and `"time_ms":5000`, and where the program raised it:
    `"file":"src/Main.st"`, `"line":17` and `"pou":"Main"`.

5. In the second terminal, ask the runtime's state and its faults, then save the fault's trace:

    ```text
    trust-runtime ctl --project . status
    trust-runtime ctl --project . faults
    trust-runtime ctl --project . fault-trace --out dosing-fault.trusttrace
    ```

    The first prints `state=faulted fault=division by zero rt_profile=disabled rt_active=false`. The
    second lists the fault with where the program raised it: `"pou":"Main"`, `"file":"src/Main.st"`,
    `"line":17`, the division in `src/Main.st`. The third prints
    `saved the fault's trace of 501 cycles (faulted) to dosing-fault.trusttrace, 53584 bytes`. The
    runtime had already written the same trace into `traces/`, named after the fault's UTC date and time:
    in the run, `traces/fault-20261125T160340Z.trusttrace`, also 53584 bytes.

6. Press Ctrl+C in the first terminal to stop the runtime. It ends with
   `Error: runtime resource faulted: division by zero`.
7. Replay the trace:

    ```text
    trust-dev replay dosing-fault.trusttrace --project .
    ```

    It builds the project as it is now, replays the 501 cycles and prints
    `Replayed 501 cycles of the same program: every cycle's outputs match the trace.`

## What happened

**The flight recorder.** `flight_recorder = 300` makes the runtime record every cycle into one of two
recordings of 300 cycles (specification 48 RU-59). A recording starts with the program's state, and each
cycle adds the tasks that ran, the inputs, the changes from outside, the clock readings and the outputs.

In words: when the recording in use is full, the runtime starts the other one afresh, so it always holds
the last 300 to 600 cycles. When a cycle faults, it keeps the full recording and the one in use, up to the
faulted cycle, as the fault's trace, which it keeps until the next fault.

As a formula, for a fault in cycle k, counting the first cycle as 0, with n cycles a recording and k ≥ n:

> the trace holds the cycles n × (⌊k / n⌋ − 1) to k, that is n + (k mod n) + 1 cycles

Worked with the run: the task runs every 10 ms and the sensor counted 0 from 5,000 ms, so cycle k = 500
faulted. With n = 300, ⌊500 / 300⌋ = 1: the trace holds the cycles 0 to 500, 300 + 200 + 1 = 501 cycles,
from 0 to 5,000 ms, as `ctl fault-trace` printed.

**The replay.** `trust-dev replay` builds the project as it is now and runs it in a simulator, on a clock
that only the replay moves (specification 48 RU-60). It starts from the program's state at the trace's
first cycle, gives each cycle the recorded inputs, changes and clock readings, and compares the cycle's
outputs, and its fault, with the trace's. The program was the same, so all 501 cycles matched, and cycle
500 faulted as it had on the machine. The trace shows the fault on any computer, without the machine and
without a broken sensor.

## Try it yourself

- Guard the division in `src/Main.st`, so that the pump doses nothing when the sensor counts no strokes:

    ```st
    IF StrokeRate > 0 THEN
        DosePerStroke := DosePerMinute / StrokeRate;
    ELSE
        DosePerStroke := 0;
    END_IF;
    ```

    Replay the same trace. `trust-dev replay` ends with 1 and prints:

    ```text
    Replayed 501 cycles of a changed program.
    The first difference is at cycle 500, at the logical time 5000 ms:
      the trace's cycle ended with division by zero in Main, the build's with no fault
    No later cycle differs.
    ```

    The first 500 cycles match, so the fix changes nothing before the fault, and at the faulted cycle the
    fixed program runs on.

- Set `flight_recorder = 100` in `runtime.toml`, run the project again and save the fault's trace. In the
  run it held 101 cycles, 11984 bytes: with n = 100, ⌊500 / 100⌋ = 5, so the cycles 400 to 500,
  100 + 0 + 1 = 101.
- Run the machine test, which does the same on a clock that only the test moves: `cargo test` in
  `machine-tests/`. `tests/fault.rs` runs 4 s and reads `DosePerStroke` 20, runs on into the fault at 5 s,
  keeps the fault's trace with `sim.fault_trace()?` and replays it in a new simulator of the project.

## Reference

| File | What it holds |
|---|---|
| `src/Main.st` | the program `Main`, which divides the dose by the stroke rate, and the two located variables |
| `src/Configuration.st` | the configuration: the task `Dosing` runs `Main` every 10 ms |
| `simulation.toml` | the stroke sensor's two readings: 60 from the start, 0 from 5 s |
| `runtime.toml` | the runtime's settings; `[runtime.trace] flight_recorder = 300` at the end |
| `io.toml` | the simulated I/O driver |
| `.vscode/launch.json` | the **truST Simulator** launch configuration that **Start** uses in VS Code |
| `machine-tests/` | the machine test `tests/fault.rs`, with trust-sim |

| Variable | Address | Type | Meaning |
| --- | --- | --- | --- |
| `StrokeRate` | `%IW0` | INT | The pump's strokes a minute, from its stroke sensor. |
| `DosePerStroke` | `%QW0` | INT | The dose each stroke gives, to the pump's dosing setpoint. |

- [Machine Tests](https://johannespettersson80.github.io/trust-platform/develop/machine-tests/): a fault in
  a machine test, and replaying a trace from the machine.
- [`trust-runtime ctl fault-trace`](https://johannespettersson80.github.io/trust-platform/reference/cli/trust-runtime/)
  and [`trust-dev replay`](https://johannespettersson80.github.io/trust-platform/reference/cli/trust-dev/).
- [`[runtime.trace] flight_recorder`](https://johannespettersson80.github.io/trust-platform/reference/config/runtime-toml/).
- Specification 48, "Rust in truST projects", RU-59 and RU-60 (`docs/specs/48-rust-pous.md`).
