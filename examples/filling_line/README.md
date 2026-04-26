# Filling Line: Capstone Process-Control Tutorial

Docs category: `docs/public/examples/capstones.md`

This is the capstone tutorial for process control in truST.

You will run a full multi-file project, inspect interface implementations,
exercise I/O scenarios, and apply hot reload.

## What You Learn

- Interface-driven FB design (`IValve`, `IPump`)
- Hysteresis-based control logic
- Task + I/O binding via `CONFIGURATION`
- Live tuning with `Structured Text: Hot Reload`

## Files to Study

- `src/Types.st`
- `src/Interfaces.st`
- `src/ValveActuator.st`
- `src/PumpDrive.st`
- `src/LevelController.st`
- `src/Main.st`
- `src/Configuration.st`

## Step 1: Build and Open

```bash
code examples/filling_line
trust-runtime build --project examples/filling_line --sources src
trust-runtime validate --project examples/filling_line
```

## Step 2: Interface and Type Hierarchy Exploration

1. Open `src/Interfaces.st`.
2. Right-click `IValve` -> `Show Type Hierarchy`.
3. Confirm implementation path includes `ValveActuator`.
4. Ctrl+Click `IValve` usages in `src/ValveActuator.st` and `src/Main.st`.
5. Repeat for `IPump` and `PumpDrive`.

## Step 3: Formatting Profile Check

1. Open `src/Main.st`.
2. Run `Shift+Alt+F` (`Format Document`).
3. Confirm formatting is stable and consistent with codesys-oriented project style.

## Step 4: Runtime Panel + Scenario Table

1. `Ctrl/Cmd+Shift+P` -> `Structured Text: Open Runtime Panel`.
2. Start runtime (Local) or connect external runtime.
3. Drive inputs according to table below.

| Input Writes | Expected Outputs | Troubleshooting |
|---|---|---|
| `%IX0.0=TRUE`, `%IX0.1=FALSE`, `%IW0=500` | `%QX0.0=TRUE`, `%QX0.1=FALSE`, `%QW0=800` | If no fill output, verify StartCmd mapping in `src/Configuration.st`. |
| `%IX0.0=TRUE`, `%IX0.1=FALSE`, `%IW0=700` | `%QX0.0=FALSE`, `%QX0.1=FALSE`, `%QW0=0` | If valves still active, check hysteresis math in `LevelController`. |
| `%IX0.0=TRUE`, `%IX0.1=FALSE`, `%IW0=900` | `%QX0.0=FALSE`, `%QX0.1=TRUE`, `%QW0=600` | If drain never opens, confirm `LevelPct` scaling from raw input. |
| `%IX0.1=TRUE` (stop override) | stopped outputs (`FALSE/FALSE/0`) | If not stopping, check stop condition precedence in controller logic. |

## Step 5: Hot Reload Tuning Exercise

1. Open `src/LevelController.st`.
2. Change `Target : REAL := 70.0;` to `60.0;`.
3. Run `Ctrl/Cmd+Shift+P` -> `Structured Text: Hot Reload`.
4. Re-run scenario `%IW0=700` and observe neutral band shift.

## Step 6: Deep-Dive Debug

1. Set breakpoint in `src/Main.st` output selection block.
2. Press `F5`.
3. Toggle `%IW0` across thresholds while stepping.
4. Observe `Ctrl.Mode`, `FillValveOut`, `DrainValveOut`, `PumpSpeedOut`.

## Troubleshooting

- Runtime panel not updating:
  - verify runtime started and control endpoint reachable.
- Unexpected mode transitions:
  - inspect scaled `LevelPct` and hysteresis values.
- Hot reload no effect:
  - ensure command executed successfully and file was saved before reload.
