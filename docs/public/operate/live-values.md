# Live Values: Write, Force, Release

**Live Values** shows your runtime's I/O and memory live, and lets you **write**, **force**, and
**release** values safely — without a debugger. Use it to watch I/O while the program runs and to drive
inputs during commissioning.

> Live Values is for *values*. To pause execution and step line by line, use
> [Debugging In VS Code](debugging-in-vscode.md).

## Open it

Click **Live Values** in the truST view (it also opens automatically when you Start the simulator). It
opens as a **Live Values** editor tab. When you launch it from Devices & Connections it reuses that
editor group; other launch routes open it in the second editor group.

## Before you start

With nothing running, Live Values clears stale rows and explains that there is no live data. Start a
simulator or connect a remote from the truST sidebar or Devices & Connections; lifecycle controls do not
live inside the value-safety surface.

## Live I/O while it runs

Once the simulator is running, the **I/O** tree fills in, grouped into **Inputs**, **Outputs**, and
**Memory**. Each row shows the symbol, its address (e.g. `%IX0.0`), its current value, and per-row
controls — **Write**, **Force**, and **Release**.

## Write a value

Click **Write** on a row and enter a value. The write is applied and the row updates after a newer runtime
scan. A write sets the value
once; the program can change it again on the next scan.

## Force a value

On the simulator, click **Force** to pin a value. On a connected managed or remote target, the first
click explicitly arms force and the second performs it. A forced value is unmistakable: it carries a
warning-role **FORCED** badge, and a **Release all forces** button appears in the header with a count.

## Release forces

- **Release** on a row releases that one force.
- **Release all forces (N)** in the header clears every override at once — the safety action to get back
  to live behaviour quickly.

After releasing, the **FORCED** badges and the **Release all forces** button disappear, and values return
to whatever the program is driving.

## Honest by design

Write and force are deliberate, gated actions. If the target is disconnected, the runtime doesn't allow
the operation, or your role lacks permission, the control is disabled with a reason — Live Values never
fakes a successful write or a force that didn't take. Forced values always show their state, so an
override is never silently left in place.

## Next

- [Debugging In VS Code](debugging-in-vscode.md)
- [Devices & Connections](../connect/communication-panel.md)
