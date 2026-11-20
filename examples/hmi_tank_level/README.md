# HMI tank level

You operate the screens of tank T-101 as an operator does: log in, raise the level setpoint, and handle the
high-level alarm that it brings. It teaches an operator screen with alarms over a real `Control.PID` loop.

## What you learn

- An HMI page shows a running program's values, and a logged-in operator changes only what the runtime allows
  (specification 39, §12).
- An alarm has a limit, a deadband and a priority. It comes in unacknowledged, the operator acknowledges it,
  and it leaves the list once the value is back (specification 40).
- The HMI Builder's **Live** mode operates the runtime's screens from VS Code (specification 39, §11).

## What you need

- truST in VS Code, and the `trust-runtime` command line from the truST release archive to run the runtime.
  No hardware: the runtime simulates the tank from `simulation.toml`.
- About 15 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **HMI tank level**. Pick a folder under
   **Select destination folder** and keep the name: VS Code opens the copy. In a terminal in that folder, run
   `trust-runtime build --project .` and then `trust-runtime run --project .`, and leave it running. The first
   build needs the truST toolchain: if `build` says it is not installed, run `trust-runtime toolchain install`
   once (pressing **Start** in VS Code installs it too).
2. In the side panel, open **Target**, choose **This project's runtime**, and press **Connect**. The status bar
   says `truST: This project's runtime · debugging disabled`: this example keeps debugging off, so Live Values
   has nothing to show, and the screens show the values instead.
3. Open `hmi/pages/overview.toml`: it opens in the HMI Builder. Press **Live** in its top bar. The bar says
   **Live · This project's runtime**, and the runtime display shows the Overview with **SIMULATION** and **No
   alarms**. In the run the **Level** read 1.51 m and the **Setpoint** 1.50 m, the controller was in **Auto**,
   and the inlet pump P-101 was **Running**.
4. Press **Log in** at the top right, and log in as `operator` with the password `operator`. The bar shows
   **operator**, the role **Operator**, and **Log out**.
5. Click the **Setpoint** card, type 2.6 under **Value**, and press **Send**. The Setpoint shows 2.60 m, the
   controller opens the inlet valve FV-101, and the level climbs. The card shows **Confirmed** once the runtime
   has applied the new value. In the run the level read 1.61 m after 2.3 s, 2.03 m after 9.3 s, 2.33 m after
   20.4 s and 2.45 m after 31.5 s.
6. When the level passes 2.5 m, 38.5 s after the change in the run, **T-101 level high** comes in. The top bar
   says **1 alarm** and shows the alarm with its time, the **Level** card shows a warning mark, and the alarm
   list under the trend has its row: **Medium**, the time, **T-101 level high**, **Unacknowledged** and
   **Acknowledge**. The top bar also counts **1 suppressed**: the rising-fast alarm, which this example
   suppresses by design while the controller is in Auto.
7. Press **Acknowledge** on the row. It says **Acknowledged** and stays in the list, because the level is still
   above the limit.
8. Click the **Setpoint** card again, type 2.0 and press **Send**. The controller closes the valve and the level
   falls: in the run 2.51 m after 1 s, 2.47 m after 2 s and 2.42 m after 3 s. Below 2.45 m the alarm returns to
   normal and leaves the list, and the top bar says **No alarms**.
9. Press **Log out**, and stop the runtime with Ctrl+C in its terminal.

## What happened

**The loop.** `Control.PID` (proportional gain 50 % per m, integral gain 2 % per m and s) compares the level
with the setpoint every 100 ms and sets the opening of FV-101. The tank is a mass balance (specification 46,
`gravity_tank`):

> A · dh/dt = q_in − c · √h, with q_in = u / 100 × 0.05 m³/s

where A = 0.2 m² is the tank's area, h the level in m, q_in the inflow, c = 0.02 m^(5/2)/s the outlet's
coefficient, and u the valve opening in %. The level settles where the inflow equals the outflow.

Worked with the run: at 2.6 m the outlet lets out c · √2.6 = 0.02 × 1.612 = 0.0322 m³/s, so the valve must
open u = 0.0322 / 0.05 × 100 = 64.5 %. Near the top of the rise the run's Overview showed FV-101 at 65 %.

**The alarm.** The vessel's `T-101 level high` alarm (`src/Main.st`) has the limit 2.5 m, the deadband
0.05 m and the priority Medium. It comes in when the level rises above 2.5 m and returns to normal when it
falls below 2.5 m − 0.05 m = 2.45 m (`Alarm.LimitAlarm`, specification 40), so a level that wavers around the
limit does not raise it again and again. Its states are those of ISA-18.2 (IEC 62682; specification 40,
§3.1): it comes in as active and unacknowledged; **Acknowledge** makes it active and acknowledged, and it stays
listed while the level is high; when the level is back below 2.45 m it returns to normal and leaves the list
of active alarms. In the run it came in when the level reached 2.50 m, and it was gone when the level read
2.42 m, 3 s after the setpoint went down.

## Try it yourself

- Put the controller in **Manual** (the mode selector) and set **Manual** to 100 %. The level rises past
  2.8 m: **T-101 level high-high** comes in, trips the inlet pump P-101 and is marked first out. **Reset trip**
  clears it once the level is back below the limit.
- Open the **Alarms** page from the navigation: it holds the full alarm list and the event log.
- Change the high limit `HighLimit` in `src/Main.st` to 2.4 m, build and run again: the alarm comes in earlier,
  at 2.4 m, and returns to normal below 2.35 m.

## Reference

Files:

| File | Holds |
| --- | --- |
| `src/Main.st` | The level control with `Control.PID`, the vessel's and the pump's alarms |
| `simulation.toml` | The tank: 0.2 m², an outlet of c = 0.02 m^(5/2)/s, starting at 1.5 m |
| `runtime.toml` | The runtime display's web server, its users and what they may change |
| `hmi/` | The screens: an Overview page and an Alarms page |
| `libraries/` | Unchanged copies of the Control and Alarm libraries, so the example stands on its own |

The alarms:

| Alarm | Priority | When |
| --- | --- | --- |
| T-101 level high | Medium | above 2.5 m |
| T-101 level high-high | High | above 2.8 m; trips the pump, latches, first out |
| T-101 level low | Medium | below 0.3 m |
| T-101 level rising fast | Low | rising faster than 0.05 m/s; suppressed by design in Auto |
| P-101 motor overload | High | an overload; trips the pump, latches, first out |

The users: `operator` and `engineer`, whose demo passwords equal their names. Replace them with `trust-runtime
hmi-password` before real use. The runtime also serves the screens in a browser, at <http://127.0.0.1:8080/hmi>.

Without the runtime, press **Start** in the side panel and then **Live** in the builder: the Simulator shows
the values, but it has no HMI web server, so the page cannot be operated and shows no alarms (specification
39, §11 item 6).

To deploy changed screens from the editor, keep the runtime connected and press **Deploy** in the builder;
**Roll back** returns to the previous screens.
