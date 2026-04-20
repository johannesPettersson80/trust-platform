# Program In VS Code

This is the primary truST engineering workflow.

Start with the shipped tutorial project in desktop VS Code.

![Desktop VS Code with the truST runtime panel](../assets/images/hero-runtime.png)

*Figure:* Desktop VS Code with Structured Text code, the runtime panel docked
on the right, live I/O, memory, and compile diagnostics in one window.

## What You Get In One Window

- IEC-aware diagnostics and semantic highlighting
- go to definition, references, rename, and formatting
- the runtime panel with live I/O and memory
- the debugger with breakpoints, stepping, locals, and call stack

## Use A Shipped Project First

Start with a shipped tutorial:

- `examples/tutorials/12_hmi_pid_process_dashboard`

Open it:

```bash
code examples/tutorials/12_hmi_pid_process_dashboard
```

## Quick Start

1. Install `truST LSP`.
2. Open the tutorial project in VS Code.
3. Open `src/main.st` and `src/config.st`.
4. Run `Structured Text: Open Runtime Panel`.
5. Start the runtime in `Local` mode.
6. Inspect `%I`, `%Q`, and memory values in the runtime panel.
7. Set a breakpoint inside the control logic and press `F5`.
8. Rename or jump to the control function block to confirm the LSP flow.
9. Open `/hmi` only after the editor-side behavior makes sense.

## IEC-Aware Diagnostics

![IEC-aware diagnostics in VS Code](../assets/images/vscode/iec-diagnostics.png)

*Figure:* The Problems panel reports the IEC rule directly, so the editor tells
you what language rule was broken instead of only showing a generic parser
failure.

## Debug Live

![Debugger paused at a breakpoint in VS Code](../assets/images/vscode/debugger-stopped-at-breakpoint.png)

*Figure:* The debugger paused at a breakpoint with locals, call stack, inline
values, and the runtime panel visible beside the code.

## Refactor Safely

![Rename across files in VS Code](../assets/images/vscode/lsp-rename-across-files.png)

*Figure:* Rename a Structured Text symbol across files from the editor and
preview the affected definition before you apply the change.

## Safe Signals In The Tutorial

The tutorial already maps safe proof signals:

- `%IX0.0` = `StartCmd`
- `%IX0.1` = `StopCmd`
- `%IX0.2` = `PressureSpikeCmd`
- `%IX0.3` = `BypassCmd`
- `%QX0.0` = `PumpRunning`
- `%QX0.1` = `HighPressureAlarm`
- `%QX0.4` = `BypassOpen`

Verify it works:

1. toggle `%IX0.0`
2. confirm the runtime panel changes
3. open `/hmi` from the running project for visual confirmation

![Browser HMI for the tutorial project](../assets/images/browser/hmi-home.png)

*Figure:* `/hmi` for the same shipped tutorial project once the runtime is
connected. Use it as the operator-side confirmation after the editor,
diagnostics, runtime panel, and debugger all look correct.

## If It Fails

- no commands in Command Palette: go to [Installation](installation.md)
- runtime panel does not connect: go to
  [Debugging And Runtime Panel](../operate/debugging-and-runtime-panel.md)
- values do not move: go to [I/O Binding](../connect/devices-and-fieldbus/io-binding.md)
- diagnostics appear after your edit: use the clickable Problems panel
- a realistic first failure is a one-letter typo such as `PumpRuning` instead of
  `PumpRunning`; click the Problems entry to jump straight to the bad line

## Next

- [Create A New Project](create-new-project.md)
- [Project Layout](../develop/project-layout.md)
- [Debugging And Runtime Panel](../operate/debugging-and-runtime-panel.md)
- [HMI And Web UI](../operate/hmi-and-web-ui.md)
