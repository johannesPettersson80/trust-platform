# Debugging And Runtime Panel

truST puts the debugger and runtime panel beside your Structured Text code in
VS Code.

![Debugger paused at a breakpoint in VS Code](../assets/images/vscode/debugger-stopped-at-breakpoint.png)

*Figure:* The debugger paused at a breakpoint with locals, call stack, inline
values, and the runtime panel visible beside the code.

## What this surface gives you

- breakpoints and stepping at ST statement boundaries
- locals, call stack, and inline values in the editor
- live I/O, memory, and compile diagnostics in the runtime panel
- one project and control endpoint shared by edit, run, and debug

## Fast path

1. Open a truST project in VS Code.
2. Run `Structured Text: Open Runtime Panel`.
3. Choose local or external mode.
4. Start the runtime.
5. Use `F5` when you need breakpoints and stepping.

## Runtime panel

![Desktop VS Code with the truST runtime panel](../assets/images/hero-runtime.png)

*Figure:* Desktop VS Code with Structured Text code, the runtime panel docked
on the right, live I/O, memory, and compile diagnostics in one window.

Use the runtime panel for:

- live I/O read and quick state checks
- quick local iteration without leaving the editor
- viewing runtime state while editing code

### Good panel workflows

| Task | Best surface |
| --- | --- |
| flip a simulated bit and watch the result | runtime panel |
| confirm `%I/%Q/%M` addresses are mapped as expected | runtime panel |
| inspect faults, restart, or control connection state | runtime panel |
| debug program flow with breakpoints | debugger |

### Common debug scenarios

#### Output never changes

1. Confirm the runtime is actually running.
2. Check whether the source variable is mapped in `Configuration.st`.
3. Inspect the runtime panel I/O tree for `%I` and `%Q` changes.
4. If the input changes but output does not, set a breakpoint in the ST logic.

#### Timer never fires

1. Confirm the task is scheduled in `CONFIGURATION`.
2. Confirm the runtime scan is running and not faulted.
3. Inspect the timer inputs or elapsed state in debugger or runtime panel.

#### Type mismatch or impossible write

1. Check diagnostics first.
2. Confirm the target address class matches the value you are writing.
3. Use [Build, Validate, Test](build-validate-test.md) before assuming the runtime is wrong.

## Debugger

Use the debugger when you need:

- breakpoints
- step in / step over / step out
- variable inspection
- inline values

The adapter is `trust-debug`, and VS Code drives it through the same runtime
control endpoint the rest of truST uses.

### Typical debugger flow

1. Build and validate the project first.
2. Start or attach to the runtime.
3. Set a breakpoint in the ST file you care about.
4. Press `F5`.
5. Inspect variables, step, and resume until the failure condition is understood.

## Browser Runtime Overview

The runtime web UI gives a browser-hosted runtime summary outside VS Code.

![Runtime overview with live inputs and outputs](../assets/images/runtime/ui-overview.png)

*Figure:* The browser runtime overview shows health, cycle timing, tasks, and
the live input/output summary when you need the same runtime state outside the
editor.

### When not to use the debugger

- do not start with the debugger when simple diagnostics or a forced I/O check will answer the question faster
- do not treat debugger success as proof that hardware mappings are correct; verify through the runtime panel too

## Related

- [Runtime UI And Control](runtime-ui-and-control.md)
- [Agent Quickstart](../start/agent-quickstart.md)
- [trust-debug](../reference/cli/trust-debug.md)
