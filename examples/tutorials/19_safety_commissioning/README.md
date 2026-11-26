# Tutorial 19: Safety Commissioning (Safe State, Watchdog, Fault Policy, Restart)

Prove on a bench, before production start-up, that a runtime fault drives the outputs to their safe
state, and that a cold restart brings the program back. You trigger a division by zero on purpose,
read the safe state back, and restart. Run it only in a non-production environment.

## What you learn

- `[[io.safe_state]]` in `io.toml` names the value each output takes when the runtime stops or
  faults, and `[runtime.fault] policy = "safe_halt"` drives those values before it halts the
  resource. With `halt`, the outputs stay as the last scan drove them.
- After a fault, `io-read` answers with the safe values, marked `"safe_state": true`, and the event
  log names the fault and `safe_state_applied` with each output it drove.
- `restart cold` starts the program again with every variable at its initial value.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- Two terminals.
- About 20 minutes.

## Step by step

1. Copy tutorial 13's project, so that fault testing never runs against it:

    ```bash
    rm -rf /tmp/trust-safety-commissioning
    cp -R /tmp/trust-tutorial-13 /tmp/trust-safety-commissioning
    cd /tmp/trust-safety-commissioning
    ```

2. Give the program a fault trigger: a division whose divisor `FaultTrigger` sets to zero. Replace
   `src/Main.st` and `src/Configuration.st`, which puts `FaultTrigger` at `%IX0.1`:

    ```bash
    cat > src/Main.st <<'ST'
    PROGRAM FirstApp
    VAR
        StartCmd : BOOL;
        FaultTrigger : BOOL;
        LampOut : BOOL;
        Divisor : INT := 1;
        ProbeValue : INT;
    END_VAR

    IF FaultTrigger THEN
        Divisor := 0;
    ELSE
        Divisor := 1;
    END_IF;

    ProbeValue := 10 / Divisor;
    LampOut := StartCmd;
    END_PROGRAM
    ST
    cat > src/Configuration.st <<'ST'
    CONFIGURATION FirstConfig
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : FirstApp;
    VAR_CONFIG
        P1.StartCmd AT %IX0.0 : BOOL;
        P1.FaultTrigger AT %IX0.1 : BOOL;
        P1.LampOut AT %QX0.0 : BOOL;
    END_VAR
    END_CONFIGURATION
    ST
    ```

3. Set the safety policy. In `runtime.toml`, put these sections in place of its
   `[runtime.watchdog]` and `[runtime.fault]` sections:

    ```toml
    [runtime.watchdog]
    enabled = true
    timeout_ms = 2000
    action = "safe_halt"

    [runtime.fault]
    policy = "safe_halt"
    ```

    Then write `io.toml`, with the loopback driver and the safe state of the output:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "loopback"
    params = {}

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML
    ```

4. Build and check the project:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    ```

    The validation answers `Project ok`.

5. Start the runtime in this terminal:

    ```bash
    trust-runtime run --project .
    ```

    In the second terminal, in the same folder, set the input and read the image:

    ```bash
    trust-runtime ctl --project . io-write %IX0.0 true
    trust-runtime ctl --project . io-read
    ```

    `%IX0.0` and `%QX0.0` read `Bool(true)`: the program copies the input to the output.

6. Trigger the fault:

    ```bash
    trust-runtime ctl --project . io-write %IX0.1 true
    trust-runtime ctl --project . status
    ```

    The status names the fault:
    `state=faulted fault=division by zero rt_profile=disabled rt_active=false`.
    Read the image and the events:

    ```bash
    trust-runtime ctl --project . io-read
    trust-runtime ctl --project . events
    ```

    The snapshot holds `%QX0.0` at `Bool(false)` and is marked `"safe_state": true`. The events
    name the fault, `{"error":"division by zero","file":"src/Main.st","line":16,"pou":"FirstApp",
    …,"type":"fault"}`, and the safe state,
    `{"outputs":[{"address":"%QX0.0","value":"Bool(false)"}],…,"type":"safe_state_applied"}`.

7. Recover with a cold restart:

    ```bash
    trust-runtime ctl --project . restart cold
    trust-runtime ctl --project . status
    ```

    The restart answers `"status":"restart queued"`, and the status reads
    `state=running fault=none rt_profile=disabled rt_active=false`. Every input and output reads
    `Bool(false)` again, and `trust-runtime ctl --project . io-write %IX0.0 true` makes `%QX0.0`
    follow `%IX0.0` as in step 5. Stop the runtime with
    `trust-runtime ctl --project . shutdown --exit`.

## What happened

**The fault.** With `FaultTrigger` TRUE the program divides by zero, which faults the resource.
The fault policy `safe_halt` wrote the safe state of `[[io.safe_state]]` to the output image and
through the driver, then halted: `%QX0.0` went to FALSE although `StartCmd` was still TRUE. A
fault policy of `halt` would have left it TRUE.

**The reads.** While the resource is halted, `io-read` answers with the snapshot of the last scan
that completed, its inputs, and the safe values in its outputs, marked `"safe_state": true`, and
the event log keeps the fault, with the file, line and program of the division, and
`safe_state_applied`. Archive the status, the snapshot and the events as the commissioning
evidence.

**The loopback driver.** It stands in for a bench where each output is wired back to the input of
the same address. A value that you write to an input takes effect at the next scan, and it holds
where the program drives the output at that address to the same value, as `%IX0.0` does through
`%QX0.0`. The snapshot after the fault is the last scan that completed, the one before the trigger
took effect, so `%IX0.1` reads FALSE in it.

**The restart.** `restart cold` starts the program with every variable at its initial value, so
the program runs again, and the next scan's snapshot carries no safe-state mark.

## Try it yourself

- Set `policy = "halt"` in `[runtime.fault]`, build, start and fault the resource again: `%QX0.0`
  stays `Bool(true)` after the fault, and the events hold the fault without `safe_state_applied`.
- Delete `[[io.safe_state]]` from `io.toml` and fault the resource with `safe_halt`: with no safe
  values configured, the fault writes no output, so `%QX0.0` stays `Bool(true)` and the events hold
  the fault alone.

## Reference

- Specification 11 §6.6, fault, overrun and watchdog handling, and its safe state in the reads.
- `io.toml`'s `[[io.safe_state]]`, `runtime.toml`'s `[runtime.watchdog]` and `[runtime.fault]`, and
  the `trust-runtime ctl` commands `io-write`, `io-read`, `status`, `events` and `restart`, in the
  reference of the documentation.
- Tutorial 13, which builds the project this tutorial starts from.
