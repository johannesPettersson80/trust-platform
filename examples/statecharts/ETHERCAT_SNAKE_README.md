# EtherCAT Snake Pattern - UML StateChart Examples

This directory contains statechart examples for real EtherCAT hardware control, including a visual "snake" / "Knight Rider" output pattern.

## Available Files

### `ethercat-snake.statechart.json`

Full snake pattern with 17 states:

- Phase 1 (states 0-8): turn LEDs on sequentially `0 -> 7`
- Phase 2 (states 9-16): turn LEDs off sequentially `7 -> 0`
- Continuous loop back to state 1
- 16 transitions per full cycle

### `ethercat-snake-bidirectional.statechart.json` (recommended)

Bidirectional pattern:

- Forward (`0 -> 7`)
- Backward (`6 -> 0`)
- Each state turns one LED on in entry and off in exit
- Visual effect: one moving active LED

## Required Hardware

```text
[PC NIC] -> [EK1100 Coupler] -> [EL2008 DO 8ch]
```

- `EK1100`: EtherCAT bus coupler
- `EL2008`: 8-channel digital output module
- LEDs/loads connected to `DO0..DO7`

## Action Mappings

`actionMappings` connect statechart actions to physical I/O.

```json
{
  "turnOn_DO0": {
    "action": "WRITE_OUTPUT",
    "address": "%QX0.0",
    "value": true
  }
}
```

### Supported action types

| Action Type | Description | Example |
|---|---|---|
| `WRITE_OUTPUT` | Write a digital output | `%QX0.0 := TRUE` |
| `WRITE_VARIABLE` | Write an ST variable | `motorSpeed := 1500` |
| `SET_MULTIPLE` | Write multiple outputs | Turn all LEDs off |
| `LOG` | A comment in the companion; no effect when the program runs | State diagnostics |

## IEC Address Mapping

```text
%QX0.0  -> EL2008 channel 0 (DO0)
%QX0.1  -> EL2008 channel 1 (DO1)
%QX0.2  -> EL2008 channel 2 (DO2)
...
%QX0.7  -> EL2008 channel 7 (DO7)
```

Format: `%QX[byte].[bit]`

- `Q` = output
- `X` = bit/boolean
- `0` = byte index
- `.0-.7` = bit index

## Run in VS Code

A statechart runs as Structured Text, like every visual program
(`docs/specs/17-visual-editors-runtime-unification.md`); the editor has no
simulation or hardware mode of its own.

1. Open `examples/statecharts/ethercat-snake-bidirectional.statechart.json`.
   The diagram shows the forward and backward states, the `TICK` transitions
   and the entry/exit actions of each state.
2. Save it: truST writes the companion `ethercat-snake-bidirectional.st` and
   the runtime entry `ethercat-snake-bidirectional.visual.runtime.st` beside
   it.
3. Start it from the truST sidebar.
4. In the debugger's Variables view (Set Value), set
   `fb_ethercat_snake_bidirectional.EV_START` to `TRUE` and back to `FALSE`,
   then do the same with `EV_TICK`: every scan in which `EV_TICK` is `TRUE`
   moves the chart one state.
5. Watch `%QX0.0`–`%QX0.7` in Live Values.

## Run with Real Hardware

### Runtime integration

The runtime entry declares `%QX0.0`–`%QX0.7` as globals `AT` those addresses,
so a project whose `io.toml` maps them to the EL2008 drives the LEDs with the
values the companion writes (see [HARDWARE_EXECUTION.md](HARDWARE_EXECUTION.md)).

### Example project/runtime setup

Create a project with:

- the chart in the project's `src/` folder, so its companion and runtime
  entry (written when you save the chart) are among the project's sources
- `io.toml` configured for EtherCAT (`EK1100`, `EL2008`)

Example `io.toml` snippet:

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "enp111s0"
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"

[[io.params.modules]]
model = "EK1100"
slot = 0

[[io.params.modules]]
model = "EL2008"
slot = 1
channels = 8
```

Run command example:

```bash
sudo setcap cap_net_raw,cap_net_admin=eip "$(readlink -f "$(which trust-runtime)")"
sudo nmcli dev set enp111s0 managed no
sudo ip link set enp111s0 up

trust-runtime run --project <your-project>
```

## Pattern Timing

Each scan in which `EV_TICK` is `TRUE` moves the chart one state. If a one-scan `TICK` pulse is raised every 200ms:

| Phase | States | Total Time |
|---|---|---|
| Forward | 8 states | 1.6s |
| Backward | 7 states | 1.4s |
| Full cycle | 15 transitions | 3.0s |

The speed is the rate at which `EV_TICK` is raised.

## Tips

- Run the chart and check the outputs in Live Values before connecting loads.
- Keep safe-state outputs configured in `io.toml`.
- Ensure action names in states match keys in `actionMappings`.
- Give `trust-runtime` the EtherCAT interface permissions (`setcap`) before starting it.
