# HMI equipment

You operate a small transfer line through its equipment panels: the inlet pump, the level controller and the
level transmitter of tank T-101. It teaches Plant library equipment on an HMI page: each symbol opens its panel,
and what a panel lets you do follows the equipment's mode.

## What you learn

- Each equipment symbol opens its panel. In **Auto** the program commands the pump, so its **Start** and **Stop**
  are greyed, with the reason "Auto: switch to Manual" (specification 39, §8.4).
- A level loop's panel shows its mode, measurement, setpoint and output. **Manual** hands the output to the
  operator, and the return to **Auto** is bumpless: the controller starts from the output as it is.
- A level transmitter raises its own alarms: LT-101's **Low** comes in below 0.3 m.

## What you need

- truST in VS Code, and the `trust-runtime` command line from the truST release archive to run the runtime.
  No hardware: the runtime simulates the tank from `simulation.toml`.
- About 20 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **HMI equipment**. Pick a folder under **Select
   destination folder** and keep the name: VS Code opens the copy. In a terminal in that folder, run
   `trust-runtime build --project .` and then `trust-runtime run --project .`, and leave it running. The first
   build needs the truST toolchain: if `build` says it is not installed, run `trust-runtime toolchain install`
   once (pressing **Start** in VS Code installs it too).
2. In the side panel, open **Target**, choose **This project's runtime**, and press **Connect**. The status bar
   says `truST: This project's runtime · debugging disabled`.
3. Open `hmi/pages/overview.toml`: it opens in the HMI Builder. Press **Live** in its top bar. The runtime display
   shows the transfer line: P-101, FV-101, T-101, LT-101 and LIC-101, with the level trend and the tank gauge.
   Right after the runtime starts, the level dips: the controller starts from a closed valve, and the tank drains
   until it opens. In the run the gauge read 0.94 m when Live opened, with the pump running and the valve at
   39.7 %.
4. Press **Log in** at the top right, and log in as `operator` with the password `operator`.
5. Click **P-101**. Its panel says **Running** and **Auto**; **Start** and **Stop** are greyed, with "Auto: switch
   to Manual" under them, because the program runs the pump in Auto. **Starts** counts 1. Close the panel with ✕.
6. Click **LIC-101**. Its panel shows **Mode** Auto, the **Measurement**, the **Setpoint** 1.50 m, the
   **Output** and their history. Watch the level come back to the setpoint: in the run it rose from 0.97 m to
   1.45 m in 65 s, while the output went from 43.5 % to 48.9 %.
7. Click **Setpoint**, type 1.8 and press **Send**. The setpoint shows 1.80 m, the output rises, and the level
   follows: from 1.45 m to 1.78 m in 28 s in the run, where the output settled at 54.0 %.
8. Press **Mode**: it switches to **Manual**. Click **Output**, type 0 and press **Send**. The valve closes and
   the tank drains: in the run the level fell from 1.78 m to 0.33 m in 15 s, and below 0.3 m the top bar said
   **1 alarm**, the newest **Low**, LT-101's alarm.
9. Press **Mode** again: it switches back to **Auto**. The output starts where it was, at 0 % (1.1 % at the first
   reading), so the valve does not jump, and the controller refills the tank: from 0.22 m to 1.00 m in 34 s in the
   run, with the output at 50.6 %. The Low alarm stays listed until someone acknowledges it on the **Alarms**
   page.
10. Press **Log out**, and stop the runtime with Ctrl+C in its terminal.

## What happened

**Equipment and modes.** P-101 is a Plant library `FB_Pump` (trust-plant-library §6.1). In **Auto** it follows
the program's run demand (`AutoRun`, TRUE here unless LT-101 reads high-high); in **Manual** it follows the
operator's Start and Stop. The panel shows only what the mode allows, and says why the rest is greyed. LIC-101
is the real `Control.PID` in ordinary code: its panel writes the loop's mode, setpoint and manual output, which
`runtime.toml` lets an operator write within limits.

**The loop and the tank.** The tank is a mass balance (specification 46, `gravity_tank`):

> A · dh/dt = q_in − c · √h, with q_in = u / 100 × 0.05 m³/s

where A = 0.2 m² is the tank's area, h the level in m, q_in the inflow, c = 0.02 m^(5/2)/s the outlet's
coefficient, and u the valve opening in %. The level settles where the inflow equals the outflow, and with the
valve closed it drains at dh/dt = −c · √h / A, so from h₁ to h₂ it takes t = 2 · A / c · (√h₁ − √h₂).

Worked with the run: to hold 1.8 m the outlet lets out c · √1.8 = 0.02 × 1.342 = 0.0268 m³/s, so the valve must
open u = 0.0268 / 0.05 × 100 = 53.7 %; the run's output settled at 54.0 %. With the valve closed, from 1.78 m to the
Low limit of 0.3 m takes t = 2 × 0.2 / 0.02 × (√1.78 − √0.3) = 20 × (1.334 − 0.548) = 15.7 s; the run's level
passed 0.3 m between 15 s and 16 s after the output went to 0.

**The start-up dip.** At a cold start the loop's output is 0, and the loop starts bumpless from it, as Create PLC
code writes a loop, so the valve opens only as the level falls below the setpoint. A loop that should start at
its holding output needs that output as its starting value.

## Try it yourself

- Click **P-101**, press **Manual**, then **Stop**. The pump stops, and LIC-101 tracks a closed valve while it
  stands, so the level falls without the output winding up. Press **Start**: the loop takes over again without a
  bump. In Manual, Start and Stop are no longer greyed.
- Keep LIC-101 in Manual at 0 % longer: below 0.1 m LT-101's **Low-low** comes in too, with priority High.
- Open the **Alarms** page and acknowledge the Low alarm: once the level is back above 0.3 m + 0.05 m, it leaves
  the list.

## Reference

| File | Holds |
| --- | --- |
| `src/Main.st` | The pump, the valve, the transmitter, the tank and the level loop, called every 100 ms |
| `simulation.toml` | The tank: 0.2 m², an outlet of c = 0.02 m^(5/2)/s, starting at 1.5 m |
| `runtime.toml` | The runtime display's web server, its users and the loop's write rules |
| `hmi/` | Overview: the symbols, their pipes and the level trend; Alarms: the alarm list and the event log; Panels: the pump, loop and tank panels placed on a page |
| `libraries/` | Copies of the Alarm, Control, Hmi and Plant libraries, so the example stands on its own |

The users: `operator` and `engineer`, whose demo passwords equal their names. Replace them with `trust-runtime
hmi-password` before real use. Engineers open **Settings** in a panel's header to change, for example, the loop's
gains or the pump's start timeout; the values are retained across power loss. Control Studio tunes the same
gains: they are the loop's one parameter record, `LIC101_Parameters`. The runtime also serves the screens in a
browser, at <http://127.0.0.1:8080/hmi>.

Without the runtime, **Start** and **Live** show the values on the Simulator, but it has no HMI web server, so the
panels cannot be operated (specification 39, §11 item 6).
