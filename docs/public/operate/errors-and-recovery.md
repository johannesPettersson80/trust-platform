# Errors And Recovery

truST is built to be **honest**: it never shows a fabricated "connected" or green state, and every failure
tells you the cause and the next action. This page lists the common ones and how to recover.

## A program error

**What you see:** Check fails, and the errors appear in VS Code's **Problems** panel with their IEC
references; the Run card reflects that there are errors.

**Recover:** Click a Problems entry to jump to the line. A frequent first error is a one-letter typo (e.g.
`PumpRuning` instead of `PumpRunning`). Fix it and the diagnostic clears as you type. See
[Run & Check](debugging-and-runtime-panel.md).

## A broken or hand-edited config

**What you see:** After editing a `*.toml` by hand and breaking it, Check fails ("Project check failed"),
and Devices & Connections still renders a fallback node — never a fake-green runtime.

**Recover:** Prefer the UI over hand-editing TOML — add devices from [Add A Device](../connect/add-a-device.md),
which writes valid config. If you did edit by hand, fix the reported syntax/section error and re-check.

## A runtime that isn't reachable

**What you see:** A remote target shows **Not reachable** and its Connect action is disabled, with a hint:
*"Open Devices & Connections to start or diagnose this runtime."* The node stays grey — not green.

**Recover:** Open [Devices & Connections](../connect/communication-panel.md), confirm the host is reachable
and the runtime is running, then Connect. *Discovered* means *seen*, not connected.

## Sign-in or permission needed

**What you see:** A remote shows **Sign-in required** (Connect disabled) when a token is missing or invalid,
or **Not permitted** when your role can't perform the action. In Live Values, a denied write/force control
is disabled with the reason — it never silently fails or fakes a force.

**Recover:** Set the runtime's credentials in Devices & Connections (truST stores them securely via VS Code,
not in plain settings). The default local simulator and a managed local runtime need no manual token to
write/force/release.

## The simulator won't start

**What you see:** The Run card shows **Failed to start** with a reason and a "Show output" action.

**Recover:** Open the output to read the cause (a compile error, a port already in use, or a missing
runtime). Fix it and press **Start** again. If a control port is already in use, stop the other process or
change the port in the runtime config.

## HMI shows nothing

**What you see:** The HMI Preview says **"Start the runtime to see live HMI data."** rather than a blank
screen or stale values; Trends/Alarms tabs show a designed note explaining what they render.

**Recover:** Start the simulator (or connect a runtime). Once connected, the preview reads
*"Values refreshed (connected)."* See [HMI (Preview & Browser)](hmi-and-web-ui.md).

## The honest-status promise

Across every surface: grey means *not running/connected*; green appears **only** when a runtime is genuinely
running or a connection is proven; pending states are bounded, never an endless spinner; and you are never
left with a bare "failed" — there is always a cause and a next step.

## Related

- [Run & Check](debugging-and-runtime-panel.md)
- [Live Values](live-values.md)
- [Devices & Connections](../connect/communication-panel.md)
