# Tutorial 12: PID Process Transfer Model (Live PLC Data)

This tutorial is a complete end-to-end workflow around one small process model:

1. build a feed-tank to product-tank transfer process in Structured Text,
2. run it in `trust-runtime` with loopback I/O,
3. drive start, stop, pressure-spike, and bypass scenarios through `%IX` inputs,
4. watch setpoints, deviations, and alarms react live.

## Project Layout

```text
12_hmi_pid_process_dashboard/
  README.md
  trust-lsp.toml
  runtime.toml
  io.toml
  src/
    main.st
    config.st
```

## What Is Included (Alarms + Setpoints)

The PLC model already contains:

- flow and pressure setpoints:
  - `P1.FlowSetpointM3h`
  - `P1.PressureSetpointBar`
- live process values:
  - `P1.TotalFlowM3h`
  - `P1.MainFlowM3h`
  - `P1.BypassFlowM3h`
  - `P1.PressureBar`
- alarm booleans + summary:
  - `P1.HighPressureAlarm`
  - `P1.LowFeedAlarm`
  - `P1.HighProductAlarm`
  - `P1.AnyAlarmActive`
  - `P1.AlarmActiveCount`
  - `P1.AlarmMessage`

## Prerequisites

From repository root:

```bash
cargo build -p trust-runtime -p trust-lsp -p trust-debug
```

Optional (VS Code extension):

```bash
code --install-extension trust-platform.trust-lsp
```

## Step 1: Open the tutorial in VS Code

```bash
code examples/tutorials/12_hmi_pid_process_dashboard
```

Inspect these files first:

- `src/main.st` (process model + setpoint/alarm logic)
- `src/config.st` (task, program instance, and `%IX`/`%QX` mapping)
- `io.toml` (loopback I/O driver)

## Step 2: Build the PLC program

```bash
cargo run -p trust-runtime --bin trust-runtime -- \
  build --project examples/tutorials/12_hmi_pid_process_dashboard --sources src
```

Expected outcome:

- `program.tpi` generated in tutorial folder
- no compile errors

## Step 3: Run the runtime

In a new terminal:

```bash
cargo run -p trust-runtime --bin trust-runtime -- \
  run --project examples/tutorials/12_hmi_pid_process_dashboard
```

This project ships `runtime.toml` with:

- web UI on `127.0.0.1:18082`
- control endpoint `unix:///tmp/trust-runtime-tutorial-pid.sock`

Confirm the runtime is alive:

```bash
cargo run -p trust-runtime --bin trust-runtime -- \
  ctl --project examples/tutorials/12_hmi_pid_process_dashboard status
```

## Step 4: Drive scenarios from runtime I/O

Use VS Code **Live Values** (or your control client) to write the inputs:

- `%IX0.0` -> `StartCmd`
- `%IX0.1` -> `StopCmd`
- `%IX0.2` -> `PressureSpikeCmd`
- `%IX0.3` -> `BypassCmd`

Outputs to observe:

- `%QX0.0` -> `PumpRunning`
- `%QX0.1` -> `HighPressureAlarm`
- `%QX0.2` -> `LowFeedAlarm`
- `%QX0.3` -> `HighProductAlarm`
- `%QX0.4` -> `BypassOpen`

Scenario checks:

1. Set `%IX0.0 = TRUE` (start): flow rises, tank levels move, `ModeText` switches to running.
2. Set `%IX0.3 = TRUE` (bypass): `BypassOpen` and `BypassFlowM3h` activate, setpoints change.
3. Pulse `%IX0.2 = TRUE` (pressure spike): `HighPressureAlarm` and `AlarmMessage` trigger.
4. Set `%IX0.1 = TRUE` (stop): process transitions to stopped mode.

Read the output image from a terminal at any time:

```bash
cargo run -p trust-runtime --bin trust-runtime -- \
  ctl --project examples/tutorials/12_hmi_pid_process_dashboard io-read
```

## Troubleshooting

### Values do not update

- confirm the runtime is running (`ctl ... status`)
- confirm `src/config.st` task/program mapping is active
- confirm the `%IX` writes target the addresses listed in Step 4

## Next Exercises

1. Add a fourth alarm (for example a low-flow alarm while the pump runs) and map it to `%QX0.5`.
2. Write a unit test for the alarm thresholds (see Tutorials 10 and 11).

## Screen

`hmi/pages/overview.toml` shows this example's values. It opens in the HMI Builder in VS Code; press **Live** while the simulator runs to see the values move. The runtime also serves it at <http://127.0.0.1:18082/hmi>.
