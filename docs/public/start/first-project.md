# First Project

Start with a shipped project before you create one from an empty folder.

## Recommended Project

Use:

```text
examples/tutorials/12_hmi_pid_process_dashboard
```

Open it:

```bash
code examples/tutorials/12_hmi_pid_process_dashboard
```

## Walkthrough

1. Install truST from the [Marketplace](installation.md).
2. Open the shipped tutorial in VS Code.
3. Open `src/main.st` and `src/config.st`.
4. Run `Structured Text: Open Runtime Panel`.
5. Start the runtime in `Local` mode.
6. Toggle `%IX0.0` and confirm `PumpRunning` changes in the runtime panel.
7. Open `/hmi` from the same running project and confirm the dashboard updates.
8. Set one breakpoint and press `F5` to verify the debugger path.

The shipped tutorial gives you a working runtime, browser HMI, and debug path
without guessing about project layout or missing config files.

## What Success Looks Like

- VS Code shows truST commands and diagnostics.
- The runtime panel connects to the local runtime.
- `%I` and `%Q` values move when you toggle tutorial inputs.
- `/hmi` opens for the same project.
- A breakpoint stops in the tutorial code.

## When To Create A New Project Instead

Go straight to [Create A New Project](create-new-project.md) when you already
know the target layout, runtime config, and deployment path you need.

## Related

- [Program In VS Code](program-in-vscode.md)
- [Create A New Project](create-new-project.md)
- [Maintain An Existing Project](maintain-an-existing-project.md)
