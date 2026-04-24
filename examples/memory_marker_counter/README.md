# Memory Marker Counter: Runtime I/O + Debugging Tutorial

Docs category: `docs/public/examples/test-and-debug.md`

This tutorial teaches the scan-cycle memory model and shows how to inspect it in
both the Runtime Panel and debugger.

## Learning Goal

Understand exactly when `%M` values are read and written in one cycle:

1. cycle start: `%M` -> bound ST variable
2. logic execution: program updates values
3. cycle end: updated ST variable -> `%M`

## Project Files

- `src/Main.st`
- `src/Configuration.st`
- `trust-lsp.toml`
- `.vscode/launch.json`

## Step 1: Open and Build

From repository root:

```bash
code examples/memory_marker_counter
trust-runtime build --project examples/memory_marker_counter --sources src
trust-runtime validate --project examples/memory_marker_counter
```

## Step 2: Start Runtime + Panel

1. In VS Code: `Ctrl/Cmd+Shift+P` -> `Structured Text: Open Runtime Panel`
2. Start runtime (Local mode) or connect to running runtime.
3. Open the panel sections:
   - `I/O -> Inputs` for writing `%M` values
   - `I/O -> Outputs` for reading `%Q` values
   - cycle/heartbeat area to confirm scans are progressing

## Step 3: Core Experiment (Annotated)

Bindings (`src/Configuration.st`):

- `%MW0` <-> `Counter`
- `%QW0` <-> `CounterLatched`

Procedure:

1. Write `%MW0 = Word(41)`.
2. Wait one cycle.

Expected after first cycle:

- `%QW0 = Word(41)` (latched cycle-start value)
- `%MW0 = Word(42)` (incremented writeback at cycle end)

Expected after more cycles:

- `%MW0` increments by 1 per cycle.

## Step 4: Debugger Integration

1. Open `src/Main.st`.
2. Set breakpoint on `Counter := Counter + 1;`.
3. Press `F5` using `.vscode/launch.json`.
4. In Variables panel, inspect `Counter` and `CounterLatched`.
5. Step over and observe value transitions.
6. Confirm inline values in editor match debugger state.

## Step 5: Modify and Re-Test

Change:

- `Counter := Counter + 1;` -> `Counter := Counter + 5;`

Repeat Step 3 and confirm `%MW0` jumps by 5 each cycle.

## Automated Check

```bash
./scripts/test_memory_marker_sync.sh
```

## Pitfalls and Fixes

- Writing `%Q` and expecting it to behave like input memory:
  - fix: write `%MW0`, observe `%QW0` as output evidence.
- Runtime panel not connected:
  - fix: start runtime or verify endpoint in `trust-lsp.toml`.
- Values not updating:
  - fix: confirm cycle is running and writes are applied (not staged only).
