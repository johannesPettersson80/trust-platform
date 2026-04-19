# Ladder Logic Examples

Docs category: `docs/public/examples/visual-editors.md`

This folder contains Ladder Diagram (LD) examples for the VS Code Ladder editor and the
generated ST companion workflow.

IEC-aligned LD specification references:

- Normative LD language spec: `docs/specs/11-ladder-diagram.md`
- truST LD implementation profile: `docs/specs/12-ladder-profile-trust.md`

## Files

- `simple-start-stop.ladder.json`: Basic two-rung start/stop example.
- `ethercat-snake.ladder.json`: LED chase pattern using multiple outputs.

Both examples use LD schema v2 (`schemaVersion: 2`).
`simple-start-stop.ladder.json` demonstrates symbolic node operands mapped to declared
global variables (`scope: "global"`), with an additional local declaration example.

## Open an Example in the Ladder Editor

From repository root:

```bash
code --new-window --extensionDevelopmentPath=editors/vscode . examples/ladder/simple-start-stop.ladder.json
```

If VS Code opens text view, use `Reopen Editor With...` and select `Ladder Logic Editor`.

## Right Pane Workflow (Current UI)

The Ladder editor uses a right-side pane with three views:

- `I/O` (default): ST-style runtime controls and runtime tree.
- `Settings`: ST runtime settings view.
- `Tools`: Ladder element/rung/edit operations (contact, coil, timer, counter, compare, math, rung add/remove, undo/redo, copy/paste, replace, auto-route, save).

Runtime controls in `I/O`/`Settings` match ST panel behavior:

- Mode toggle: `Local` / `External`
- Start/Stop button
- Runtime status text/pill
- Filtered I/O tree and force/write/release actions

Symbolic runtime control notes:

- Input force/write supports declared symbols as well as direct `%IX*` addresses.
- When local and global variables share the same name, use scoped references
  (`LOCAL::Name` / `GLOBAL::Name`) to disambiguate.

## Execution Modes

### Local

- In-extension simulation.
- No external runtime endpoint required.

### External

- Connects to runtime endpoint (for example `/tmp/trust-debug.sock`).
- Useful for real hardware backends like `examples/hardware_8do`.

## Hardware Run (External Mode)

1. Start hardware backend:

```bash
cd examples/hardware_8do
sudo ./start.sh
```

2. Open a ladder example in the Ladder editor.
3. In right pane `I/O`, switch to `External`, then click `Start`.

## Notes

- Saving a `.ladder.json` updates deterministic ST companion output (`<name>.st`) used by
  ST-first runtime/project workflows.
- Custom editor registration is optional; text editing of `.ladder.json` remains available.
