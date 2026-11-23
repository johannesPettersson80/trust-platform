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

**Faults.** A cycle that faults ends `run_cycles` or `run_for` with `trust_sim::Error::Fault`,
and the project's fault policy acts as in a run.

## Try it yourself

- Raise the tank's low water through the Simulation program, with
  `plant.write("Simulation.LowWater", true)`, and check that the heating fails with
  `T-101 water low`.
- Press Stop once the heater runs, and check that the pump still runs 29 s later and has stopped
  31 s later.

## Reference

- `examples/room_heating/machine-tests/tests/heating.rs`: the example's machine tests, which CI
  runs.
- Specification 48, RU-58: what a machine test may do.
- Specification 11 §6.8.1, "Simulator for machine tests" and "Calling a method".
