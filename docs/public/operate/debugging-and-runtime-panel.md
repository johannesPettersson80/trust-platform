# Compile & Run

Compile your program, then run it on the simulator — both from the **Run** card in the truST
view, with honest status at every step.

## Compile your program

Press **Compile** to validate the whole project. On success, the status bar says
**Compile passed — 2 sources, no errors.**

![The Run card after Compile passed with two sources and no errors](../assets/images/vscode/vscode-run-check-passed.png)

*Compile validates the whole project before you run. The Run card keeps the
Simulator selected with one clear Start button, and the status bar reports the
result.*

### When Compile fails

If the project has errors, they appear in VS Code's **Problems** panel with their IEC references (for
example an undefined identifier on a specific line), and the Run card reflects that there are errors. Fix
them in the editor — the Problems entries are clickable and jump to the source. truST is honest here: a
broken config surfaces an error and a fallback, never a fake-green runtime.

## Choose where it runs

**Target** selects where the program runs. For a new project this is the built-in
**Simulator**, with its current state in the label (for example **Simulator ·
Stopped**). The dropdown lists only existing targets (the simulator and any
runtimes you've added in [Devices & Connections](../connect/communication-panel.md));
you add or connect targets there, not from this dropdown.

## Run on the simulator

Press **Start**. truST compiles your program and runs it on the simulator; the
target changes to **Simulator · Running** and the button becomes **Stop**. The
status bar mirrors this (`truST: Simulator running`).
Starting from the Run card keeps your editor in place and does not open Live
Values. Open Live Values yourself when you want to inspect values.

![The Run card with the simulator running: Status Running and a Stop button](../assets/images/vscode/vscode-run-simulator-running.png)

*Start runs your program on the local simulator — honest Running status, one Stop button, and the same state echoed in the status bar.*

The Run card, status bar, and Simulator node in **Devices & Connections** read
from the same accepted lifecycle state. During launch they show **Starting**;
after the simulator accepts the session they show **Running**; after Stop they
show **Stopped**. A late or failed launch is not allowed to turn one surface
green while another says Stopped.

- **Start** — compile, then run on the selected target.
- **Stop** — stop the simulator.
- **Apply changes** — appears when you've edited the source while the simulator is running, to update
  without a full restart (simulator only).

While it runs, open [Live Values](live-values.md) to watch and drive I/O, or set a breakpoint and use
[Debugging In VS Code](debugging-in-vscode.md) to step through the logic.

## Related

- [Debugging In VS Code](debugging-in-vscode.md)
- [Live Values: write, force, release](live-values.md)
- [Build, Validate, Test](build-validate-test.md)
- [Runtime UI And Control](runtime-ui-and-control.md)
