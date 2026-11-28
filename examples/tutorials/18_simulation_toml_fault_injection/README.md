# Tutorial 18: Simulation with `simulation.toml`

Run tutorial 13's project as a simulation: a coupling plays the lamp's feedback contact, scheduled
disturbances press and release the start button, and a scheduled fault stops the program. You learn
how `simulation.toml` drives a run on its own clock, why the same file gives the same run every
time, and how to run that clock faster. It starts from tutorial 13's project.

## What you learn

- A `simulation.toml` beside the project makes `trust-runtime run` a simulation: a coupling copies
  an output to an input after a delay, as wiring or a sensor would, and a disturbance sets an input
  or raises a fault at a given time.
- The times count on the simulation's clock from the start, so every run of the same file passes
  through the same states, and `--time-scale` runs that clock faster without changing them.
- `[simulation] enabled = true` turns the simulation on, `--simulation` turns on a file that says
  `enabled = false`, and the runtime reports the mode as not for live hardware.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- Linux or macOS, whose shell runs the commands below, and two terminals.
- About 20 minutes.

## Step by step

1. Copy tutorial 13's project:

    ```bash
    rm -rf /tmp/trust-simulation
    cp -R /tmp/trust-tutorial-13 /tmp/trust-simulation
    cd /tmp/trust-simulation
    ```

2. Give the lamp a feedback input, `LampFeedback` at `%IX0.1`, as a real panel wires the lamp's
   auxiliary contact back to the PLC, and build:

    ```bash
    cat > src/Main.st <<'ST'
    PROGRAM FirstApp
    VAR
        StartCmd : BOOL;
        LampOut : BOOL;
        LampFeedback : BOOL;
    END_VAR

    LampOut := StartCmd;
    END_PROGRAM
    ST

    cat > src/Configuration.st <<'ST'
    CONFIGURATION FirstConfig
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : FirstApp;
    VAR_CONFIG
        P1.StartCmd AT %IX0.0 : BOOL;
        P1.LampFeedback AT %IX0.1 : BOOL;
        P1.LampOut AT %QX0.0 : BOOL;
    END_VAR
    END_CONFIGURATION
    ST

    trust-runtime build --project . --sources src
    ```

    The program still copies `StartCmd` to `LampOut`. The build lists its two sources.

3. Write `simulation.toml` and check the project:

    ```bash
    cat > simulation.toml <<'TOML'
    [simulation]
    enabled = true
    seed = 42

    [[couplings]]
    source = "%QX0.0"
    target = "%IX0.1"
    delay_ms = 3000

    [[disturbances]]
    at_ms = 5000
    target = "%IX0.0"
    value = "TRUE"

    [[disturbances]]
    at_ms = 15000
    target = "%IX0.0"
    value = "FALSE"

    [[disturbances]]
    at_ms = 25000
    kind = "fault"
    message = "lamp circuit open"
    TOML

    trust-runtime validate --project .
    ```

    The coupling feeds the lamp's output back to `LampFeedback` 3 s after it changes. The first two
    disturbances press the start button at 5 s and release it at 15 s, and the third faults the
    program at 25 s. `validate` answers `Project ok`: it checks this file too.

4. Start the runtime in the second terminal with `trust-runtime run --project .`. The file's
   `enabled = true` makes this run a simulation. In the first terminal,
   `trust-runtime ctl --project . config-get` shows it among its keys:
   `"simulation.mode":"simulation"` and
   `"simulation.warning":"Simulation mode active (time scale x1). Not for live hardware."`.

5. Read the image as the schedule passes, a few times over the next 20 seconds:

    ```bash
    trust-runtime ctl --project . io-read
    ```

    The snapshot's `scan` counts the 100 ms cycles from the start, so it tells the simulation time.
    The real run read:

    | `scan` | `%IX0.0` `StartCmd` | `%IX0.1` `LampFeedback` | `%QX0.0` `LampOut` |
    | --- | --- | --- | --- |
    | 30 | `Bool(false)` | `Bool(false)` | `Bool(false)` |
    | 61 | `Bool(true)` | `Bool(false)` | `Bool(true)` |
    | 90 | `Bool(true)` | `Bool(true)` | `Bool(true)` |
    | 165 | `Bool(false)` | `Bool(true)` | `Bool(false)` |
    | 190 | `Bool(false)` | `Bool(false)` | `Bool(false)` |

    At 5 s the button press reaches `StartCmd`, and the program turns the lamp on in the same
    cycle; `LampFeedback` follows 3 s later. At 15 s the release turns the lamp off, and
    `LampFeedback` drops at 18 s.

6. After 25 s the program stops. `trust-runtime ctl --project . status` answers
   `state=faulted fault=simulation fault 'lamp circuit open'`, and
   `trust-runtime ctl --project . events` lists the fault at its simulation time:
   `{"error":"simulation fault 'lamp circuit open'","time_ns":25000000000,"type":"fault"}`. Stop
   the runtime with `trust-runtime ctl --project . shutdown --exit`.

7. Run it again ten times faster, with `trust-runtime run --project . --time-scale 10` in the
   second terminal. The same states pass in two and a half seconds, and `events` lists the fault
   at `"time_ns":25000000000` again. Stop the runtime.

## What happened

**A simulation beside the program.** Before each cycle the runtime writes the disturbances that are
due and the coupling effects that are due into the input image; after the cycle the couplings read
the outputs that it committed and queue their effects for later. An effect therefore never reaches
the cycle that caused it, as with real wiring. The simulated I/O driver of tutorial 13 writes no
input of its own, so these writes are the only change to the inputs.

**The same run every time.** The schedule counts simulation time from the start, and each cycle
moves that clock by its 100 ms. The same file and program pass through the same states at the same
times in every run, which is what makes a simulation useful for finding a fault and checking its
fix. The time scale only runs that clock faster than the wall clock: the fault comes at 25 s of
simulation time at either speed. `seed` is part of a run's identity; this model has no random
element, so it changes nothing here, while the plant models of specification 46 use it.

**When a run is a simulation.** `enabled = true` makes `trust-runtime run` a simulation, as does a
file with couplings or disturbances and no `enabled`. `--simulation` turns on a file that says
`enabled = false`, and a `--time-scale` above 1 turns the simulation on and overrides the file's
`time_scale`. A simulated runtime reports it, in `config-get` and in the banner of
`trust-runtime play`: it is not for live hardware, whose inputs the simulation would contradict.

**A scheduled fault is a fault.** The fault disturbance stops the program as any fault does: the
cycles stop, `status` reports it and the event store records it. Tutorial 13's `runtime.toml`
gives `[runtime.fault] policy = "halt"`, which leaves the outputs as they were; `"safe_halt"` would
drive the outputs of `[[io.safe_state]]` too. That is how to see what the program, an HMI or a
supervisor does when the plant fails, before a plant is connected.

**Not a test of its own.** The simulation shows the program's behaviour over time; it checks
nothing by itself. Unit tests (tutorials 10 and 11) check the logic, and `trust-runtime simulate`
runs a project's simulation for a machine test on a clock that the test moves.

## Try it yourself

- Shorten the coupling's `delay_ms` to `500` and run again: `LampFeedback` follows the lamp half a
  second later, so a read near scan 60 shows it at `Bool(true)` already, and after the release it
  drops by scan 160. Put `3000` back.
- Set `enabled = false` and run `trust-runtime run --project .`: the program runs alone, so
  `StartCmd` stays `Bool(false)` after 5 s, and `config-get` answers `"simulation.enabled":false`.
  Start it with `trust-runtime run --project . --simulation` instead: the schedule runs again.
- Misspell a disturbance's kind as `kind = "dropout"`: `trust-runtime validate --project .` refuses
  the project with `Error: invalid config 'unsupported disturbance kind 'dropout''`, and so does
  `run`.

## Reference

- Specification 11 §6.9.4, simulation model execution, and §6.9.4.1, its configuration and
  conversion rules; specification 46 §6.1, simulation time and the time scale.
- The Simulation page of the documentation, and tutorials 10 and 11 for unit tests.
