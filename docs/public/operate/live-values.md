# Live Values: Write, Force, Release

**Live Values** shows your runtime's I/O and memory live, and lets you **write**, **force**, and
**release** values safely — without a debugger. Use it to watch I/O while the program runs and to drive
inputs during commissioning.

> Live Values is for *values*. To pause execution and step line by line, use
> [Debugging In VS Code](debugging-in-vscode.md).

## Open it

Click **Live Values** in the truST view. It opens as a **Live Values** editor tab
on the right. Starting the simulator does not open or focus this tab.

## Before you start

With nothing running, Live Values shows that the Simulator is stopped and that
there is no live data. It does not contain a second Start/Stop control. Start the
selected target from the Run card, then return to this tab.

![Live Values stopped with no duplicate runtime controls](../assets/images/vscode/vscode-live-values-stopped.png)

*Before Start, Live Values reports the stopped Simulator and tells you where to
start it; the values pane stays focused on values.*

## Live I/O while it runs

Once the simulator is running, the **I/O** tree fills in, grouped into **Inputs**, **Outputs**, and
**Memory**. Each row shows the symbol, its address (e.g. `%IX0.0`), its current value, and per-row
actions. **Write** applies a one-time value and **Force** pins a value. While a
force is active, that row's **Force** action becomes **Release**.

![Live Values running with Inputs and Outputs plus Write and Force actions](../assets/images/vscode/vscode-live-values-running.png)

*Live I/O once the simulator is running — each signal shows its value with
clear Write and Force actions.*

## ADS values

When the project contains imported ADS variables, expand **Connected variables
→ ADS**. Each row shows the generated name, current value, IEC type, and runtime
quality (**Good**, **Stale**, or **Error**) together with the source connection
and remote symbol details.

ADS rows are read-only in Live Values. Configure ADS writes explicitly in the
project rather than treating a commissioning value table as an unguarded write
surface. If an entry is stale, failed, or does not match the supported snapshot
contract, Live Values shows that problem instead of displaying it as healthy.

## Write a value

Click **Write** on a row and enter a value. The write is applied and the row updates. A write sets the value
once; the program can change it again on the next scan.

## Force a value

Click **Force** to pin a value regardless of what the program does. A forced
value is unmistakable: it carries a yellow **FORCED** badge, its row action
becomes **Release**, and a **Release all forces** button
appears in the header with a count.

![An input forced TRUE, badged FORCED, with "Release all forces (1)" in the header](../assets/images/vscode/vscode-live-values-force.png)

*Force an input — it is clearly badged **FORCED**, and the header shows how many forces are active.*

## Release forces

- **Release** on a forced row releases that one force.
- **Release all forces (N)** in the header clears every override at once — the safety action to get back
  to live behaviour quickly.

![Two values forced, with "Release all forces (2)" in the header](../assets/images/vscode/vscode-live-values-release-all.png)

*"Release all forces" clears every override at once — note the count tracks how many are active.*

After releasing, the **FORCED** badges and the **Release all forces** button disappear, and values return
to whatever the program is driving.

![Live Values after release: no FORCED badges, values back to live](../assets/images/vscode/vscode-live-values-released.png)

*After release, the overrides are gone and values are live again.*

## Honest by design

Write and force are deliberate, gated actions. If the target is disconnected, the runtime doesn't allow
the operation, or your role lacks permission, the control is disabled with a reason — Live Values never
fakes a successful write or a force that didn't take. Forced values always show their state, so an
override is never silently left in place.

## Next

- [Debugging In VS Code](debugging-in-vscode.md)
- [Devices & Connections](../connect/communication-panel.md)
