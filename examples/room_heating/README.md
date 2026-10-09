# Room heating

Hot water heats a room. Tank T-101 holds the water at 45 °C: heater EH-101 heats it, transmitter
TT-101 measures it and temperature controller TIC-101 sets the heater's power. Pump P-101
circulates the water through three-way valve TV-101, which divides it between fan-coil E-101 and
the bypass, and fan F-101 blows the room air through the fan-coil. Temperature controller
TIC-102 holds the room at 22 °C with the valve. Flow switch FS-101 confirms circulation and
low-water switch LSL-101 guards the tank.

The project was drawn and configured in the HMI Builder: the equipment is Plant library blocks,
**Create PLC code** wrote their declarations and calls, and both temperature loops are the real
`Control.PID` in ordinary code, written the way Create PLC code writes a loop.

It runs without hardware: `simulation.toml` simulates the tank, the room and the valve, and the
field signals that answer the outputs.

## What is in it

| File | Holds |
| --- | --- |
| `src/Main.st` | The I/O list, the protections, the start/stop logic, the fan's room thermostat, two alarms and the generated regions: the equipment's declarations and calls and the two loops |
| `src/HeatingSequence.st` | The start and stop sequence: circulation first, then the tank, then the room; on Stop the pump runs on to carry the heat away |
| `src/Simulation.st` | The window opening and the field faults the Simulation page switches; delete it, its page and `simulation.toml` on a real plant |
| `plant.toml` | The equipment design behind Create PLC code: blocks, settings, signals and which equipment each controller measures and drives |
| `simulation.toml` | The plant model: tank, room and valve as a linear model around 45 °C, 22 °C and 51 %, with measurement noise, and the couplings for the contactor, flow and fault signals |
| `runtime.toml` | The runtime display's web server, its users and the write rules of the loops and settings |
| `control-loops/tank-temperature.json` | The tank loop as Control Studio set it up from the program's code |
| `hmi/` | Pages: Overview, Trends, Alarms, Interlocks, Settings and Simulation; English, German and Swedish texts; the tank and hand valve as project symbols |
| `libraries/` | Copies of the Alarm, Control, Hmi and Plant libraries, so the example stands on its own |

## Try it

```sh
trust-runtime build --project .
trust-runtime run --project . --simulation
```

Open <http://127.0.0.1:8080/hmi> and log in as `operator` (password `operator`) to operate, or as
`engineer` (password `engineer`) to change settings. The demo passwords equal the user names;
replace them with `trust-runtime hmi-password` before real use.

1. Press **Start** on the Heating panel. P-101 starts, FS-101 confirms the flow, EH-101 heats the
   tank, and at 40 °C the sequence goes on to **Heat the room**: the fan blows and TIC-102 opens
   TV-101 to the fan-coil. The tank settles at 45 °C and the room at 22 °C.
2. Select **TIC-101**. Its panel shows the mode, the measurement, the setpoint, the output and
   their trend. Switch **Mode** to Manual and set **Output**: the heater follows. Switch back to
   Auto: the change is bumpless.
3. On the **Simulation** page, open the window: the room cools and TIC-102 opens the valve further
   to hold 22 °C.
4. Switch **Low water** on the Simulation page: the heater and pump protections trip, the heater
   and pump stop, the sequence fails with "T-101 water low" and the alarm list shows it. Switch it
   off, acknowledge, and press **Reset**, then **Start**.

Engineers open **Settings** in a panel's header (TIC-101: setpoint, gains, output limits) or the
Settings page. The loops' setpoints, modes, outputs and gains are retained across a power loss.

## The loops in Control Studio

Both loops are set up for Control Studio (`control-loops/tank-temperature.json` and
`control-loops/room-temperature.json`). The setup form took everything from the code: the
parameter record (`TIC101_Parameters`), the PID's own measurement (`TIC101.StateValue.ProcessValue`),
the output, the setpoint, the manual output, the Auto/Manual signal (`TIC101_Manual`, TRUE in
Manual) and the startup flag. The engineer typed only the names, units, ranges and the
identification step (10 % for the tank, 5 % for the room, so the room stays below the fan
thermostat's off-point). The gains in the program are the design values; tuning them end to end
in Control Studio does not succeed on this plant yet.

## What it is not

The protections hold equipment in its safe state; they are process protections, not a certified
safety function. The plant model is linear around the operating point.
