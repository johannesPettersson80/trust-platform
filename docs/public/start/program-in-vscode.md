# Program In VS Code

This is the primary truST engineering workflow.

Start with the shipped tutorial project in desktop VS Code.

![The function block LevelController with semantic highlighting and the count of its references](../assets/images/vscode/vscode-st-syntax.png)

*The function block `LevelController` of the capstone tutorial `examples/filling_line`, with
semantic highlighting and, above its declaration, the count of its references in the project
(CodeLens). The pictures on this page show that project.*

## What You Get In One Window

- IEC-aware diagnostics and semantic highlighting
- go to definition, references, rename, and formatting
- **Live Values** with live I/O and memory (read + write/force)
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
4. Press **Start** on the Run card to run the program on the simulator (see [Run & Check](../operate/debugging-and-runtime-panel.md)).
5. Open **Live Values** to inspect `%I`, `%Q`, and memory while it runs (see [Live Values](../operate/live-values.md)).
6. Set a breakpoint in the control logic and step through it (see [Debugging In VS Code](../operate/debugging-in-vscode.md)).
7. Rename or jump to the control function block to confirm the LSP flow.

## IEC-Aware Diagnostics

![The Problems panel naming the IEC rule that an assignment breaks](../assets/images/vscode/iec-diagnostics.png)

*Figure:* `PumpSpeedOut := LevelPct;` assigns a `REAL` to an `INT`. The Problems
panel names the broken rule with its section of IEC 61131-3 and suggests the
explicit conversion, `REAL_TO_INT`, instead of only reporting a generic error.

## Completion And Hover

![Completion of the standard conversion functions while typing](../assets/images/vscode/vscode-st-completion.png)

*Completion from the language server as you type: after `REAL_TO_I` it offers the standard
conversion functions from `REAL`. It suggests variables, functions, function blocks and the rest
of the standard library the same way.*

![Hover on a function block showing its declaration from another file](../assets/images/vscode/vscode-st-hover.png)

*Hover on `LevelController` in `Main.st` shows the declaration of the function block from
`LevelController.st`; go to definition, references and rename work across files the same way.*

## Debug Live

Set a breakpoint and step through your logic with VS Code's debugger — see
[Debugging In VS Code](../operate/debugging-in-vscode.md) for the full flow (breakpoints, current-line
highlight, Variables, and the Call Stack).

## Refactor Safely

![The Refactor Preview of a rename across files](../assets/images/vscode/lsp-rename-across-files.png)

*Figure:* F2 on `LevelController`, a new name, then Ctrl+Enter: the Refactor
Preview shows every change before you apply it. Here it renames the function
block where `LevelController.st` declares it, the file itself, and its use in
`Main.st`.

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
2. confirm **Live Values** changes

## If It Fails

- no commands in Command Palette: go to [Installation](installation.md)
- the program won't run or connect: go to
  [Run & Check](../operate/debugging-and-runtime-panel.md)
- values do not move: go to [I/O Binding](../connect/devices-and-fieldbus/io-binding.md)
- diagnostics appear after your edit: use the clickable Problems panel
- a realistic first failure is a one-letter typo such as `PumpRuning` instead of
  `PumpRunning`; click the Problems entry to jump straight to the bad line

## Next

- [Create A New Project](create-new-project.md)
- [Project Layout](../develop/project-layout.md)
- [Run & Check](../operate/debugging-and-runtime-panel.md)
