# HMI equipment

Tank T-101 is a gravity-drained tank. Inlet pump P-101 feeds it through control valve FV-101,
level transmitter LT-101 measures it, and level controller LIC-101 sets the valve. The equipment
is Plant library blocks: `FB_Pump`, `FB_ControlValve`, `FB_LevelInput` and `FB_Tank`. The level
loop is the real `Control.PID` in ordinary code, written the way Create PLC code writes a loop,
with one addition of its own: it tracks a closed valve while the pump stands. Each symbol of the
overview opens its panel.

It runs without hardware: the simulator and `trust-runtime` both simulate the tank from
`simulation.toml`.

## What is in it

| File | Holds |
| --- | --- |
| `src/Main.st` | The pump, the valve, the transmitter, the tank and the level loop, called every 100 ms |
| `simulation.toml` | The tank: 0.2 m², an outlet of c = 0.02 m^(5/2)/s, starting at 1.5 m |
| `runtime.toml` | The runtime display's web server, its users and the loop's write rules |
| `hmi/` | Overview: the symbols, their pipes and the level trend; Alarms: the alarm list and the event log; Panels: the pump, loop and tank panels placed on a page |
| `libraries/` | Copies of the Alarm, Control, Hmi and Plant libraries, so the example stands on its own |

## Try it on a runtime

```sh
trust-runtime build --project .
trust-runtime run --project .
```

Open <http://127.0.0.1:8080/hmi> and log in as `operator` with the password `operator` when you
change something. The demo passwords equal the user names; replace them with
`trust-runtime hmi-password` before real use.

The level controller starts in Auto and holds the level at 1.5 m.

1. Select **LIC-101**. Its panel shows the mode, the measurement, the setpoint, the output and
   their trend.
2. Set **Setpoint** to 1.8 m: the controller opens the valve and brings the level to 1.8 m.
3. Switch **Mode** to Manual and set **Output** to 0 %: the valve closes and the level falls until
   LT-101 raises its Low and then its Low-low alarm. Switch back to Auto: the change is bumpless
   and the controller refills the tank.
4. Select **P-101**, choose **Manual** and press **Stop**: the loop tracks a closed valve while the
   pump stands, and returns without a bump once you press **Start**. In **Auto** the Start and
   Stop buttons are greyed with "Auto: switch to Manual".

Engineers (log in as `engineer`) open **Settings** in a panel's header to change, for example,
the loop's gains or the pump's start timeout; the values are retained across power loss. Control
Studio tunes the same gains: they are the loop's one parameter record, `LIC101_Parameters`.
