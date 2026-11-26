# Machine Tests

A machine test runs your project's program and its simulated plant under `cargo test` and checks
what the machine does: press Start, let a minute of plant time pass, and read the outputs. The test
drives a simulator of its own, `trust-runtime simulate`, on a clock that only the test moves, so
every run gives the same result, and an hour of plant time takes only the time its scans take
(specification 48 RU-58).

The steps below build the machine tests of the example `room_heating`, which CI runs:
`examples/room_heating/machine-tests/tests/heating.rs`.

## What you need

- truST's `trust-runtime`, of the same version as `trust-sim`. trust-sim takes the runtime that
  `TRUST_RUNTIME` names, else `trust-runtime` on the `PATH`, else the runtime of the newest truST
  VS Code extension.
- Rust with `cargo`.
- A project with a simulated plant (`simulation.toml`), such as `room_heating`.

## Step by step

1. Make a crate in the project's folder, here `machine-tests/`, with trust-sim at your truST
   version:

    ```toml
    [package]
    name = "room-heating-machine-tests"
    version = "0.1.0"
    edition = "2024"
    publish = false

    [dev-dependencies]
    trust-sim = "=0.44.1"
    ```

    Give it an empty `src/lib.rs`: a crate needs a target beside its tests.

2. Write a test in `tests/heating.rs`. It opens the project, the folder above the crate, starts a
   simulator, presses the heating's Start as the Heating panel's button does, and checks the pump
   two runs of the program later:

    ```rust
    use std::time::Duration;
    use trust_sim::{Inputs, Result};

    /// The interval of the task that runs the program (`src/config.st`).
    const TASK: Duration = Duration::from_millis(100);

    #[test]
    fn a_start_runs_the_pump() -> Result<()> {
        let mut plant = trust_sim::Project::open("..")?.simulator()?;
        plant.run_cycles(1)?;
        let started: bool =
            plant.call("Main.Heating", "Plant.ISequence", "Start", Inputs::new())?;
        assert!(started);
        plant.run_for(2 * TASK)?;
        assert!(plant.read::<bool>("Main.P101_Run")?);
        Ok(())
    }
    ```

3. Run it from the crate's folder:

    ```text
    cargo test
    ```

    The first test builds the project with `trust-runtime build`, once for all the tests, and
    each test starts a simulator of its own. You see `test a_start_runs_the_pump ... ok`.

4. Read a failure. Run the program once instead of twice, `plant.run_for(TASK)`, and run the test
   again: it fails at `assertion failed: plant.read::<bool>("Main.P101_Run")?`. The program hands
   the sequence's request to the pump before it runs the sequence, so the pump's output follows one
   run later: the test found the program's real timing. Put it back.

5. Let plant time pass. `plant.run_for(Duration::from_secs(30))` runs 30 s of plant time in the
   time its scans take: `room_heating`'s test of the pump's 30 s run-on after a stop takes seconds.

## What happened

**Cycles and runs.** `run_cycles(n)` runs `n` cycles of the resource, one every
`cycle_interval_ms` of `runtime.toml`, 10 ms in `room_heating`. A task runs at its own interval,
`MainTask` every 100 ms, so the program runs at every tenth cycle: a test that waits for the
program uses `run_for` with the task's interval.

**Values.** `read` reads what the last cycle left, by the path Live Values shows
(`Main.P101_Run`, `Main.Heating.State`); an enumeration reads as its value's name, `"Running"`.
`write` writes before the next cycle, as Live Values' set does, and `force` holds a value until
`release`. `call` calls a method of an instance through an interface its type implements, as a
screen's command does; a method without a result answers `()`.

**Faults.** A cycle that faults ends `run_cycles` or `run_for` there, with
`trust_sim::Error::Fault`; its `time`, and `now()`, are that cycle's time, and its text names the
fault and, in a debug build, where the program raised it: `division by zero in Main at
src/Main.st:17`. The project's fault policy acts as in a run. A halting fault leaves the resource faulted until the test restarts it: a
run in the meantime runs no cycle, while `run_for`'s time passes.

## Replay a trace from the machine

A trace that the runtime records while you commission the machine becomes a test of every later
change. A simulator replays its cycles with the recorded inputs, changes from outside and clock
readings, and compares each cycle's outputs with the trace's (specification 48 RU-60).
`room_heating` keeps one, `traces/commissioning.trusttrace`, and CI replays it.

1. Record while you commission. With the program running on the computer, run this in the
   project's folder, then press Start on the Heating panel in the first seconds:

    ```text
    trust-runtime ctl --project . record --duration 30s --out traces/commissioning.trusttrace
    ```

    Once 30 s of logical time have run, it prints
    `recorded 3000 cycles (complete) to traces/commissioning.trusttrace, 995984 bytes`. In the
    example's recording, Start came 0.91 s in. The pump ran from 1.06 s, FS-101 saw flow at
    5.06 s, the heater ran from 5.26 s, and the fan from 18.66 s, once the tank reached 40 °C.

2. Replay it from the command line:

    ```text
    trust-dev replay traces/commissioning.trusttrace --project .
    ```

    It builds the project as it is now, replays the 3000 cycles and prints
    `Replayed 3000 cycles of the same program: every cycle's outputs match the trace.`

3. Make it a test. `tests/replay.rs` in the crate:

    ```rust
    #[test]
    fn the_commissioning_trace_replays_with_the_same_outputs() -> trust_sim::Result<()> {
        let project = trust_sim::Project::open("..")?;
        trust_sim::Replay::open("../traces/commissioning.trusttrace")?.run(&project)
    }
    ```

    `cargo test` runs it with the other tests.

4. Read a difference. Make the heating wait 5 s in the flow step before it heats: in
   `src/HeatingSequence.st`, `IF Flow THEN` becomes `IF Flow AND StepTime >= T#5s THEN`. Replay
   again:

    ```text
    Replayed 3000 cycles of a changed program.
    The first difference is at cycle 553, at the logical time 6100 ms:
      Main.EH101_Contactor (%QX0.0): the trace has true, the build false
    2446 later cycles differ too.
    ```

    `trust-dev replay` ends with 1. `cargo test` fails the test with the same report:

    ```text
    ---- the_commissioning_trace_replays_with_the_same_outputs stdout ----
    Error: the replay of ../traces/commissioning.trusttrace differs from the trace:
    Replayed 3000 cycles of a changed program.
    The first difference is at cycle 553, at the logical time 6100 ms:
      Main.EH101_Contactor (%QX0.0): the trace has true, the build false
    2446 later cycles differ too.
    ```

    Put the line back.

**After a change meant to change the outputs,** record the trace again, as in step 1, and keep the
new file with the change: the test then replays the program as it now is.

**What a replay keeps.** The trace holds the program's memory at its start and each cycle's
tasks, changes from outside, clock readings and both process images (specification 48 RU-59).
Here the changes from outside are the Start and the simulated plant's sensor values. The replay
gives the program the inputs the machine saw, so it shows where the program's decisions differ.

**Later cycles.** After the first difference the program's state departs from the recording.
The heater's controller starts from another state, so most later cycles differ too.

**A changed program.** It takes the trace's values as an online change carries them. A changed
initial value does not reach the replay, just as it would not reach a running PLC.

**Another computer.** A program computes the same results on every platform (specification 44
NX-04). CI replays the trace, recorded on Linux, on macOS and Windows too.

**A fault in a machine test.** With `[runtime.trace] flight_recorder = <n>` in `runtime.toml`,
the simulation always keeps the last `n` to `2n` cycles. A test that ends at a fault reads them
with `sim.fault_trace()?`, which gives the bytes of the fault's `.trusttrace` file, or `None`
before a fault. Save them to look at the fault later, or replay them at once:

```rust
let file = sim.fault_trace()?.expect("the fault's trace");
let replayed = project.simulator()?.replay(&file)?;
assert!(replayed.matched, "{replayed}");
```

## Try it yourself

- Raise the tank's low water through the Simulation program, with
  `plant.write("Simulation.LowWater", true)`, and check that the heating fails with
  `T-101 water low`.
- Press Stop once the heater runs, and check that the pump still runs 29 s later and has stopped
  31 s later.
- Record a stop: with `--duration 60s`, press Start, and Stop once the heater runs. Then shorten
  the pump's run-on in `src/HeatingSequence.st`, `PumpRun := StepTime < T#15s;` for
  `PumpRun := StepTime < PumpOverrun;`, and replay: the first difference is the pump's output,
  where it now stops 15 s early.

## Reference

- `examples/room_heating/machine-tests/tests/heating.rs`: the example's machine tests, which CI
  runs.
- `examples/room_heating/machine-tests/tests/replay.rs` and `examples/room_heating/traces/`: the
  commissioning trace and its replay, which CI runs too.
- `examples/rust_first_cell/tests/cell.rs`: the machine tests of a Rust-first project, in the
  project's own crate, as specification 48 §14.1 lays it out: they open the crate's folder,
  `Project::open(".")`, and test its press cell's sequence step by step.
- Specification 48, RU-59 and RU-60: what a trace keeps and how a replay runs it.
- [`trust-dev replay`](../reference/cli/trust-dev.md) and
  [`trust-runtime ctl record`](../reference/cli/trust-runtime.md).
- Specification 48, RU-58: what a machine test may do.
- Specification 11 §6.8.1, "Simulator for machine tests" and "Calling a method".
