# Plant Demo: Multi-File Architecture Tutorial

Docs category: `docs/public/examples/capstones.md`

This tutorial teaches how to navigate and debug a multi-file ST project in VS
Code.

## What You Learn

- Type + FB + Program + Configuration layering
- Cross-file navigation/refactor workflow
- Debugging a state machine through scan cycles
- Project config role (`trust-lsp.toml`)

## Project Structure

- `src/types.st`: shared enums/structs
- `src/fb_pump.st`: `PumpController` state machine
- `src/program.st`: orchestration logic (`PlantProgram`)
- `src/config.st`: `CONFIGURATION`, `TASK`, program binding
- `trust-lsp.toml`: indexing/profile settings for editor/runtime features

## Step 1: Open and Build

```bash
code examples/plant_demo
trust-runtime build --project examples/plant_demo --sources src
trust-runtime validate --project examples/plant_demo
```

## Step 2: Cross-File Navigation (Exact Keystrokes)

1. Open `src/program.st`.
2. Hold `Ctrl` and click `PumpController` -> lands in `src/fb_pump.st`.
3. Press `Alt+Left` to go back.
4. Place cursor on `SpeedSet`, press `F2`, enter `PumpSpeedSet`.
5. Confirm rename preview includes all impacted references.
6. Right-click `PumpState` -> `Find All References` (or `Shift+F12`).
7. Verify references appear across multiple files.

## Step 3: Debugger Walkthrough

1. Open `src/fb_pump.st`.
2. Set a breakpoint inside `CASE Status.State OF`.
3. Press `F5` (uses `.vscode/launch.json`).
4. In Runtime Panel, toggle `%IX0.0` (start signal).
5. Step through transitions (`Idle` -> `Starting` -> `Running`).
6. Inspect Variables panel and inline values for `Status.State` and `Ramp`.

## Step 4: Understand Configuration Relationship

Read `src/config.st` and map the hierarchy:

- `CONFIGURATION` defines deployment root.
- `TASK` defines scan interval/priority.
- `PROGRAM ... WITH TASK` binds logic execution.
- `VAR_CONFIG` binds symbols to `%I/%Q` addresses.

This is the runtime wiring contract for your typed logic model.

## Step 5: Guided Change Exercise

1. In `src/fb_pump.st`, change:
   - `RampTime : TIME := T#1s;` -> `T#2s`
2. Re-run debug.
3. Observe longer time spent in `Starting` before `Running`.

## Troubleshooting

- If `F5` fails:
  - confirm `trust-debug` path in `.vscode/settings.json`.
- If no cross-file symbols:
  - confirm workspace root is `examples/plant_demo`.
- If no runtime change on input toggles:
  - confirm correct `%IX/%IW` addresses from `src/config.st`.
